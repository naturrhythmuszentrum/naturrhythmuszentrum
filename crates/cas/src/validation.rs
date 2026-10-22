// SPDX-License-Identifier: AGPL-3.0-only
//! Independent validation of flat graph drafts before publication.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{
    Draft, Expression, ExpressionDraft, LogicalId, Occurrence, Reference, State, Vertex, encode_id,
    size,
};
use crate::model::{AtomicSort, Binding, Environment, Field, Operator, PredicateKind, Scope, Sort};

/// A single location for mathematical admission errors.
const AT: Location = Location::new("draft validation");

/// Return a small structured sort error.
const fn sort_error() -> Fault {
    Fault::invalid(InvalidCode::Sort, AT)
}

/// Check scope size before any copy or traversal of frames.
pub(crate) fn scope_limit(scope: &Scope, budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::Depth, size(scope.len(), AT)?, AT)?;
    for frame in scope {
        budget.check(BudgetKind::Edges, size(frame.len(), AT)?, AT)?;
        for slot in frame {
            atomic_sort(slot, budget)?;
        }
    }
    Ok(())
}

/// A finite field characteristic must be prime, even when algorithms remain unsupported.
fn field(value: &Field, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match value {
        Field::PositiveCharacteristic(prime) => {
            if *prime < 2 {
                return Err(Fault::invalid(InvalidCode::Context, AT));
            }
            let mut divisor = 2_u32;
            while divisor <= *prime / divisor {
                budget.work(1, AT)?;
                if prime.is_multiple_of(divisor) {
                    return Err(Fault::invalid(InvalidCode::Context, AT));
                }
                divisor = divisor
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
            }
        }
        Field::Named(name) if name.is_empty() => {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
        _ => {}
    }
    Ok(())
}

/// Validate nonrecursive coefficient fields without interpreting named domains.
fn atomic_sort(value: &AtomicSort, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match value {
        AtomicSort::Scalar(value) | AtomicSort::Matrix { field: value, .. } => field(value, budget),
        AtomicSort::Integer | AtomicSort::Boolean => Ok(()),
    }
}

/// Validate every coefficient field in one flat mathematical sort.
fn declared_sort(value: &Sort, budget: &mut Budget) -> Result<(), Fault> {
    match value {
        Sort::Test | Sort::Distribution => Ok(()),
        Sort::Value(value) | Sort::Set(value) => atomic_sort(value, budget),
        Sort::Function { inputs, output } => {
            for input in inputs {
                atomic_sort(input, budget)?;
            }
            atomic_sort(output, budget)
        }
    }
}

/// Check that every symbolic matrix extent names a declared integer identity.
fn extent_parameters(
    value: &AtomicSort,
    environment: &Environment,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let AtomicSort::Matrix { rows, columns, .. } = value else {
        return Ok(());
    };
    for extent in [rows, columns] {
        if let crate::model::Extent::Parameter(key) = extent {
            let cost = size(environment.declarations.len(), AT)?
                .checked_mul(budget.limits().name_bytes)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
            budget.work(cost, AT)?;
            if !environment.declarations.iter().any(|declaration| {
                declaration.key == *key && declaration.sort == Sort::Value(AtomicSort::Integer)
            }) {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
        }
    }
    Ok(())
}

/// Validate symbolic extents in each position of a flat sort.
fn sort_extents(value: &Sort, environment: &Environment, budget: &mut Budget) -> Result<(), Fault> {
    match value {
        Sort::Test | Sort::Distribution => Ok(()),
        Sort::Value(value) | Sort::Set(value) => extent_parameters(value, environment, budget),
        Sort::Function { inputs, output } => {
            for input in inputs {
                extent_parameters(input, environment, budget)?;
            }
            extent_parameters(output, environment, budget)
        }
    }
}

/// Encode a draft reference without accepting it as a valid graph endpoint.
fn reference(value: &Reference, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Reference::Existing(handle) => {
            out.byte(0)?;
            encode_id(handle.logical_id(), out)
        }
        Reference::Local(index) => {
            out.byte(1)?;
            out.unsigned(u64::from(*index))
        }
    }
}

/// Account for all caller-owned drafts before copying the published graph.
pub(crate) fn draft_payload(drafts: &[Draft], budget: &mut Budget) -> Result<u64, Fault> {
    budget.check(BudgetKind::StagedVertices, size(drafts.len(), AT)?, AT)?;
    let mut total = 0_u64;
    for draft in drafts {
        match draft {
            Draft::Source { text, .. } => {
                budget.check(BudgetKind::SourceBytes, size(text.len(), AT)?, AT)?;
            }
            Draft::Expression(value) => scope_limit(&value.scope, budget)?,
            Draft::Condition { scope, .. } => scope_limit(scope, budget)?,
            _ => {}
        }
        let mut out = Encoder::new(budget, AT);
        encode_draft(draft, &mut out)?;
        let bytes = out.finish();
        let amount = size(bytes.len(), AT)?
            .checked_mul(3)
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        total = total
            .checked_add(amount)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
    }
    Ok(total)
}

/// Measure a draft's complete nonrecursive payload through a bounded key sink.
fn encode_draft(draft: &Draft, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match draft {
        Draft::Solve(value) => {
            out.byte(value.tag())?;
            value.encode(out, &mut crate::domain_encoding::reference)?;
        }
        Draft::RootValue {
            header,
            selection,
            slot,
        } => {
            reference(header, out)?;
            reference(selection, out)?;
            crate::wire::index(*slot, out)?;
        }
        Draft::ExactCoefficient { header, value, map } => {
            reference(header, out)?;
            reference(value, out)?;
            reference(map, out)?;
        }
        Draft::Source { key, text } => {
            out.name(key)?;
            out.count(text.len())?;
            out.raw(text.as_bytes())?;
        }
        Draft::Cases(_)
        | Draft::Header(_)
        | Draft::Context(_)
        | Draft::Region(_)
        | Draft::FamilyRegion { .. }
        | Draft::Space(_)
        | Draft::CaseArm(_)
        | Draft::Boundary(_)
        | Draft::SourceForm(_) => {
            crate::domain_encoding::draft(draft, out)?;
        }
        Draft::Condition {
            kind,
            header,
            scope,
            arguments,
        } => encode_condition(kind, header, scope, arguments, out)?,
        Draft::Expression(value) => encode_expression(value, out)?,
        Draft::Occurrence(value) => encode_occurrence(value, out)?,
        Draft::Request {
            task,
            premises,
            context,
            inputs,
            limits,
        } => encode_request(task, premises, context, inputs, limits, out)?,
    }
    Ok(())
}

/// Retain a condition's predicate, header, scope and ordered arguments in the measured key.
fn encode_condition(
    kind: &crate::model::PredicateKind,
    header: &Reference,
    scope: &Scope,
    arguments: &[Reference],
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    encoding::predicate(kind, out)?;
    reference(header, out)?;
    encoding::scope(scope, out)?;
    out.count(arguments.len())?;
    for argument in arguments {
        reference(argument, out)?;
    }
    Ok(())
}

/// Measure ordinary expression metadata before its ordered operand references.
fn encode_expression(value: &ExpressionDraft, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(u8::from(value.space.is_some()))?;
    if let Some(space) = &value.space {
        reference(space, out)?;
    }
    encoding::operator(&value.operator, out)?;
    encoding::sort(&value.sort, out)?;
    reference(&value.header, out)?;
    encoding::scope(&value.scope, out)?;
    encoding::binding(value.binding.as_ref(), out)?;
    out.count(value.operands.len())?;
    for operand in &value.operands {
        reference(operand, out)?;
    }
    Ok(())
}

/// Measure source ownership, optional provenance and the complete occurrence child inventory.
fn encode_occurrence(
    value: &crate::graph::OccurrenceDraft,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    reference(&value.expression, out)?;
    for optional in [&value.source, &value.provenance] {
        out.byte(u8::from(optional.is_some()))?;
        if let Some(value) = optional {
            reference(value, out)?;
        }
    }
    if let Some(span) = value.span {
        out.unsigned(span.start)?;
        out.unsigned(span.end)?;
    }
    out.count(value.children.len())?;
    for child in &value.children {
        reference(child, out)?;
    }
    out.count(value.binder_names.len())?;
    for name in &value.binder_names {
        out.name(name)?;
    }
    Ok(())
}

/// Measure the exact request task, premise/input order and resolved limit table.
fn encode_request(
    task: &crate::task::Task,
    premises: &[Reference],
    context: &Reference,
    inputs: &[Reference],
    limits: &crate::budget::Limits,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    crate::task::encode(task, out)?;
    out.count(premises.len())?;
    for premise in premises {
        reference(premise, out)?;
    }
    reference(context, out)?;
    out.count(inputs.len())?;
    for input in inputs {
        reference(input, out)?;
    }
    for kind in crate::budget::ALL_KINDS {
        out.unsigned(limits.get(kind))?;
    }
    Ok(())
}

/// Canonicalize declaration order after prepaying a conservative sorting bound.
pub(crate) fn environment(value: &Environment, budget: &mut Budget) -> Result<Environment, Fault> {
    field(&value.field, budget)?;
    let count = size(value.declarations.len(), AT)?;
    budget.check(BudgetKind::Vertices, count, AT)?;
    let cost = count
        .checked_mul(count)
        .and_then(|n| n.checked_mul(budget.limits().name_bytes))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
    budget.work(cost, AT)?;
    let mut out = Encoder::new(budget, AT);
    encoding::environment(value, &mut out)?;
    let encoded = out.finish();
    budget.work(size(encoded.len(), AT)?, AT)?;
    let mut canonical = value.clone();
    canonical
        .declarations
        .sort_by(|left, right| left.key.cmp(&right.key));
    let mut keys = BTreeSet::new();
    for declaration in &canonical.declarations {
        declared_sort(&declaration.sort, budget)?;
        sort_extents(&declaration.sort, &canonical, budget)?;
        if declaration.key.is_empty() || !keys.insert(&declaration.key) {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
    }
    if matches!(canonical.field, Field::PositiveCharacteristic(0 | 1)) {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    Ok(canonical)
}

/// Check predicate arity/sorts and the semantic header of every argument.
pub(crate) fn condition(
    state: &State,
    kind: &PredicateKind,
    context: LogicalId,
    scope: &Scope,
    arguments: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    scope_limit(scope, budget)?;
    let header = &state.header(context)?.bytes;
    let mut values = Vec::new();
    values
        .try_reserve_exact(arguments.len())
        .map_err(|_| Fault::allocation(AT))?;
    for argument in arguments {
        budget.work(
            size(header.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let value = state.expression(*argument)?;
        if !crate::header::embeds(state, value.header, context, budget)? || value.scope != *scope {
            return Err(Fault::invalid(InvalidCode::Scope, AT));
        }
        values.push(&value.sort);
    }
    predicate_sorts(kind, &values)
}

/// Check only predicate shape; it provides no truth status.
fn predicate_sorts(kind: &PredicateKind, sorts: &[&Sort]) -> Result<(), Fault> {
    let boolean = Sort::Value(AtomicSort::Boolean);
    let integer = Sort::Value(AtomicSort::Integer);
    let valid = match kind {
        PredicateKind::Defined | PredicateKind::Differentiable => !sorts.is_empty(),
        PredicateKind::NonZero => {
            sorts.len() == 1 && sorts.first().is_some_and(|sort| numeric(sort))
        }
        PredicateKind::Positive | PredicateKind::NonNegative => {
            sorts.len() == 1 && sorts.first().is_some_and(|sort| ordered(sort))
        }
        PredicateKind::IntegerRange => {
            sorts.len() == 3 && sorts.iter().all(|sort| **sort == integer)
        }
        PredicateKind::Equal => sorts.len() == 2 && sorts.first() == sorts.get(1),
        PredicateKind::Symmetric => {
            sorts.len() == 1 && sorts.first().is_some_and(|sort| square(sort))
        }
        PredicateKind::And | PredicateKind::Or => sorts.iter().all(|sort| **sort == boolean),
        PredicateKind::Not | PredicateKind::ForAll => {
            sorts.len() == 1 && sorts.first() == Some(&&boolean)
        }
        PredicateKind::Named(name) => !name.is_empty(),
    };
    if valid { Ok(()) } else { Err(sort_error()) }
}

/// Whether a flat sort supports scalar or integer arithmetic.
const fn numeric(sort: &Sort) -> bool {
    matches!(
        sort,
        Sort::Value(AtomicSort::Integer | AtomicSort::Scalar(_))
    )
}

/// Whether a flat sort has the admitted real/integer ordering.
const fn ordered(sort: &Sort) -> bool {
    matches!(
        sort,
        Sort::Value(AtomicSort::Integer | AtomicSort::Scalar(Field::Rational | Field::Real)),
    )
}

/// Whether a matrix has syntactically equal row and column extents.
fn square(sort: &Sort) -> bool {
    matches!(sort, Sort::Value(AtomicSort::Matrix { rows, columns, .. }) if rows == columns)
}

/// Validate the complete scoped expression and return exact finite shape statistics.
pub(crate) fn expression(
    state: &State,
    draft: &ExpressionDraft,
    context: LogicalId,
    operands: &[LogicalId],
    budget: &mut Budget,
) -> Result<(u64, u64), Fault> {
    if let Operator::SymbolRelation(kind) = draft.operator {
        return symbol_relation(state, draft, context, operands, kind, budget);
    }
    scope_limit(&draft.scope, budget)?;
    declared_sort(&draft.sort, budget)?;
    binding_shape(&draft.operator, draft.binding.as_ref(), operands.len())?;
    let environment = &state.header(context)?.environment;
    sort_extents(&draft.sort, environment, budget)?;
    for frame in &draft.scope {
        for slot in frame {
            extent_parameters(slot, environment, budget)?;
        }
    }
    let header = &state.header(context)?.bytes;
    let mut scope_key = Encoder::new(budget, AT);
    encoding::scope(&draft.scope, &mut scope_key)?;
    encoding::binding(draft.binding.as_ref(), &mut scope_key)?;
    encoding::sort(&draft.sort, &mut scope_key)?;
    let comparison_bytes = size(scope_key.finish().len(), AT)?;
    let charge = comparison_bytes
        .checked_add(size(header.len(), AT)?)
        .and_then(|n| n.checked_mul(size(operands.len(), AT).ok()?.checked_add(1)?))
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
    budget.work(charge, AT)?;
    let mut children = Vec::new();
    children
        .try_reserve_exact(operands.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut depth = 1_u64;
    let mut unfolded = 1_u64;
    for (position, operand) in operands.iter().enumerate() {
        budget.work(
            size(header.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let child = state.expression(*operand)?;
        if !crate::header::embeds(state, child.header, context, budget)? {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
        check_child_scope(&draft.scope, draft.binding.as_ref(), position, &child.scope)?;
        depth = depth.max(
            child
                .depth
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?,
        );
        unfolded = unfolded
            .checked_add(child.unfolded)
            .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, AT))?;
        children.push(child);
    }
    budget.check(BudgetKind::Depth, depth, AT)?;
    budget.check(BudgetKind::PeakExpressionSize, unfolded, AT)?;
    binding_sorts(draft, &children)?;
    operator_sorts(
        &draft.operator,
        &draft.sort,
        &draft.scope,
        environment,
        &children,
        budget,
    )?;
    if let Operator::AlgebraicRoot { variable, .. } = &draft.operator {
        let polynomial = operands.first().copied().ok_or_else(sort_error)?;
        root_polynomial(state, polynomial, variable, budget)?;
    }
    Ok((depth, unfolded))
}

/// Check a fixed association edge without expanding either endpoint's declaration context.
fn symbol_relation(
    state: &State,
    draft: &ExpressionDraft,
    context: LogicalId,
    operands: &[LogicalId],
    kind: crate::model::SymbolRelation,
    budget: &mut Budget,
) -> Result<(u64, u64), Fault> {
    budget.work(1, AT)?;
    if !draft.scope.is_empty() || draft.binding.is_some() {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    if draft.sort != Sort::Value(AtomicSort::Boolean) || draft.space.is_some() {
        return Err(sort_error());
    }
    let [result_id, target_id] = operands else {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    };
    if result_id == target_id {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    }
    let result = state.expression(*result_id)?;
    let target = state.expression(*target_id)?;
    let result_shape = relation_endpoint(result, budget)?;
    let target_shape = relation_endpoint(target, budget)?;
    if result.header != context {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    if kind != crate::model::SymbolRelation::Copy && result_shape != target_shape {
        return Err(sort_error());
    }
    budget.check(BudgetKind::Depth, 2, AT)?;
    budget.check(BudgetKind::PeakExpressionSize, 3, AT)?;
    Ok((2, 3))
}

/// A prior admitted closed Real matrix symbol is a sealed constant-size endpoint.
fn relation_endpoint(value: &Expression, budget: &mut Budget) -> Result<[u32; 2], Fault> {
    budget.work(1, AT)?;
    if !matches!(value.operator, Operator::Free(_)) || !value.operands.is_empty() {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    }
    if !value.scope.is_empty() || value.binding.is_some() {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    let Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: crate::model::Extent::Literal(rows),
        columns: crate::model::Extent::Literal(columns),
    }) = value.sort
    else {
        return Err(sort_error());
    };
    Ok([rows, columns])
}

/// Recognize rational polynomial syntax without expanding coefficients or proving roots.
fn root_polynomial(
    state: &State,
    root: LogicalId,
    variable: &str,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(
            size(visited.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if !visited.insert(id) {
            continue;
        }
        let value = state.expression(id)?;
        match &value.operator {
            Operator::Rational(number) => number.check(budget, AT)?,
            Operator::Free(key) if key == variable => {}
            Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::IntegerPower(0..) => {}
            Operator::Div => {
                let denominator = value.operands.get(1).copied().ok_or_else(sort_error)?;
                let Operator::Rational(number) = &state.expression(denominator)?.operator else {
                    return Err(sort_error());
                };
                if number.is_zero() {
                    return Err(sort_error());
                }
            }
            _ => return Err(sort_error()),
        }
        pending
            .try_reserve(value.operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend(value.operands.iter().copied());
    }
    Ok(())
}

/// Check endpoint and set-element sorts against the declared binder frame.
fn binding_sorts(draft: &ExpressionDraft, children: &[&Expression]) -> Result<(), Fault> {
    let Some(binding) = &draft.binding else {
        return Ok(());
    };
    let first = binding.sorts.first().ok_or_else(sort_error)?;
    if matches!(
        draft.operator,
        Operator::Integral { .. } | Operator::Series { .. } | Operator::Limit(_),
    ) && children
        .iter()
        .skip(1)
        .any(|child| child.sort != Sort::Value(first.clone()))
    {
        return Err(sort_error());
    }
    if matches!(draft.operator, Operator::Set { .. })
        && (binding.sorts.len() != 1 || draft.sort != Sort::Set(first.clone()))
    {
        return Err(sort_error());
    }
    Ok(())
}

/// Compare scope frames without an implicit name-based binding operation.
fn check_child_scope(
    outer: &Scope,
    binding: Option<&Binding>,
    position: usize,
    child: &Scope,
) -> Result<(), Fault> {
    let position = u32::try_from(position).map_err(|_| Fault::invalid(InvalidCode::Scope, AT))?;
    if let Some(binding) = binding
        && binding.body_operands.contains(&position)
    {
        if child.len().checked_sub(1) != Some(outer.len())
            || child.last() != Some(&binding.sorts)
            || child.get(..outer.len()) != Some(outer.as_slice())
        {
            return Err(Fault::invalid(InvalidCode::Scope, AT));
        }
        return Ok(());
    }
    if child != outer {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    Ok(())
}

/// Require the exact body position and frame cardinality of each binder operator.
fn binding_shape(
    operator: &Operator,
    binding: Option<&Binding>,
    arity: usize,
) -> Result<(), Fault> {
    let expected = match operator {
        Operator::Integral { .. }
        | Operator::Sum
        | Operator::Series { .. }
        | Operator::Limit(_) => Some(1),
        Operator::Comprehension { .. } => Some(2),
        Operator::Predicate(PredicateKind::ForAll) | Operator::Set { .. } => Some(0),
        _ => None,
    };
    match (expected, binding) {
        (None, None) => Ok(()),
        (Some(count), Some(binding)) => {
            if binding.body_operands != [0]
                || arity == 0
                || binding.sorts.is_empty()
                || (count != 0 && binding.sorts.len() != count)
            {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            if matches!(operator, Operator::Sum | Operator::Comprehension { .. })
                && binding
                    .sorts
                    .iter()
                    .any(|sort| *sort != AtomicSort::Integer)
            {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            if matches!(
                operator,
                Operator::Integral { .. } | Operator::Series { .. } | Operator::Limit(_),
            ) && binding
                .sorts
                .iter()
                .any(|sort| !matches!(sort, AtomicSort::Scalar(_)))
            {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            Ok(())
        }
        _ => Err(Fault::invalid(InvalidCode::Scope, AT)),
    }
}

/// Validate operator sorts after scope and arity accounting.
fn operator_sorts(
    operator: &Operator,
    result: &Sort,
    scope: &Scope,
    environment: &Environment,
    children: &[&Expression],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let same = children.iter().all(|child| child.sort == *result);
    let first = children.first().copied();
    let valid = match operator {
        Operator::Select(_) | Operator::AffineReading(_) => {
            if !scope.is_empty() {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            crate::readings::syntax::validate(operator, result, children, budget)?;
            true
        }
        Operator::RegularEmbed
        | Operator::DistributionZero
        | Operator::Delta { .. }
        | Operator::DistributionAdd
        | Operator::DistributionScale
        | Operator::DistributionDerivative { .. }
        | Operator::SmoothMultiply
        | Operator::Pair
        | Operator::PairNearSupport
        | Operator::TestDerivative { .. }
        | Operator::TestMultiply
        | Operator::TestApply
        | Operator::RegularAction => {
            crate::distribution_syntax::sorts(operator, result, children, budget)?
        }
        Operator::SymbolRelation(_)
        | Operator::Cases
        | Operator::RootValue { .. }
        | Operator::ExactCoefficient => false,
        Operator::Conjugate | Operator::RealPart | Operator::ImagPart => {
            component_sorts(operator, result, children)
        }
        Operator::NamedCase(value) => {
            children.len() == 1 && same && named_case_sort(value, result, scope, environment)
        }
        Operator::Rational(_) | Operator::Free(_) | Operator::Bound { .. } => {
            leaf_sort(operator, result, scope, environment, budget)? && children.is_empty()
        }
        Operator::Constant(_) => {
            children.is_empty()
                && matches!(
                    result,
                    Sort::Value(AtomicSort::Scalar(Field::Real | Field::Complex))
                )
        }
        Operator::Neg
        | Operator::Add
        | Operator::Sub
        | Operator::Mul
        | Operator::Div
        | Operator::IntegerPower(_)
        | Operator::GeneralPower(_) => arithmetic_sorts(operator, result, children),
        Operator::Call(key) => call_sorts(key, result, environment, children),
        Operator::Derivative(key) => {
            children.len() == 1
                && same
                && matches!(result, Sort::Value(AtomicSort::Scalar(_)))
                && environment
                    .declarations
                    .iter()
                    .any(|declaration| declaration.key == *key && declaration.sort == *result)
        }
        Operator::IntegerEmbed => {
            children.len() == 1
                && first.is_some_and(|child| child.sort == Sort::Value(AtomicSort::Integer))
                && matches!(result, Sort::Value(AtomicSort::Scalar(_)))
        }
        Operator::Predicate(kind) => {
            let sorts: Vec<_> = children.iter().map(|child| &child.sort).collect();
            *result == Sort::Value(AtomicSort::Boolean) && predicate_sorts(kind, &sorts).is_ok()
        }
        Operator::Integral { .. }
        | Operator::Sum
        | Operator::Series { .. }
        | Operator::Limit(_)
        | Operator::Set { .. } => calculus_sorts(operator, result, children),
        Operator::AlgebraicRoot {
            variable,
            multiplicity,
            ..
        } => {
            *multiplicity != Some(0)
                && environment.declarations.iter().any(|declaration| {
                    declaration.key == *variable
                        && matches!(declaration.sort, Sort::Value(AtomicSort::Scalar(_)))
                })
                && calculus_sorts(operator, result, children)
        }
        Operator::Matrix { .. }
        | Operator::Comprehension { .. }
        | Operator::Transpose
        | Operator::MatrixMultiply
        | Operator::Spectral(_)
        | Operator::MatrixMap(_)
        | Operator::Entrywise(_) => matrix_sorts(operator, result, children, budget)?,
    };
    if valid { Ok(()) } else { Err(sort_error()) }
}

/// Total Complex component syntax keeps the original scalar arity and field checks.
fn component_sorts(operator: &Operator, result: &Sort, children: &[&Expression]) -> bool {
    let result_field = if *operator == Operator::Conjugate {
        Field::Complex
    } else {
        Field::Real
    };
    children.len() == 1
        && result == &Sort::Value(AtomicSort::Scalar(result_field))
        && children
            .first()
            .is_some_and(|child| child.sort == Sort::Value(AtomicSort::Scalar(Field::Complex)))
}

/// Arithmetic retains its existing pointwise sort restrictions for every operand.
fn arithmetic_sorts(operator: &Operator, result: &Sort, children: &[&Expression]) -> bool {
    let same = children.iter().all(|child| child.sort == *result);
    match operator {
        Operator::Neg => children.len() == 1 && same && numeric(result),
        Operator::Add | Operator::Sub | Operator::Mul => {
            children.len() == 2 && same && numeric(result)
        }
        Operator::Div => {
            children.len() == 2 && same && matches!(result, Sort::Value(AtomicSort::Scalar(_)))
        }
        Operator::IntegerPower(exponent) => {
            children.len() == 1
                && same
                && numeric(result)
                && (*exponent >= 0 || matches!(result, Sort::Value(AtomicSort::Scalar(_))))
        }
        Operator::GeneralPower(_) => {
            children.len() == 2
                && children.first().is_some_and(|child| child.sort == *result)
                && matches!(result, Sort::Value(AtomicSort::Scalar(_)))
                && children.get(1).is_some_and(|child| numeric(&child.sort))
        }
        _ => false,
    }
}

/// Named scalar wrappers retain their declared axis and family-specific parameter shape.
fn named_case_sort(
    value: &crate::boundary_model::NamedCase,
    result: &Sort,
    scope: &Scope,
    environment: &Environment,
) -> bool {
    scope.is_empty()
        && *result == Sort::Value(AtomicSort::Scalar(Field::Real))
        && (value.parameter.is_some() == (value.name == crate::boundary_model::CaseName::Step))
        && environment
            .declarations
            .iter()
            .any(|item| item.key == value.coordinate && item.sort == *result)
}

/// Check the value carried by an operand-free literal or variable.
fn leaf_sort(
    operator: &Operator,
    result: &Sort,
    scope: &Scope,
    environment: &Environment,
    budget: &Budget,
) -> Result<bool, Fault> {
    Ok(match operator {
        Operator::Rational(value) => {
            value.check(budget, AT)?;
            match result {
                Sort::Value(AtomicSort::Integer) => value.is_integer(),
                Sort::Value(AtomicSort::Scalar(Field::Rational | Field::Real | Field::Complex)) => {
                    true
                }
                _ => false,
            }
        }
        Operator::Free(key) => {
            let declaration = environment
                .declarations
                .iter()
                .find(|declaration| declaration.key == *key);
            if declaration.is_some_and(|declaration| declaration.sort != *result) {
                return Err(Fault::invalid(
                    InvalidCode::SymbolSortMismatch,
                    Location::new("declared symbol sort"),
                ));
            }
            declaration.is_some()
        }
        Operator::Bound { depth, slot } => {
            let depth = usize::try_from(*depth).map_err(|_| sort_error())?;
            let slot = usize::try_from(*slot).map_err(|_| sort_error())?;
            let frame = scope
                .len()
                .checked_sub(depth)
                .and_then(|n| n.checked_sub(1))
                .and_then(|index| scope.get(index));
            let actual = frame.and_then(|frame| frame.get(slot));
            if actual.is_some_and(|sort| *result != Sort::Value(sort.clone())) {
                return Err(Fault::invalid(
                    InvalidCode::SymbolSortMismatch,
                    Location::new("declared binder sort"),
                ));
            }
            actual.is_some()
        }
        _ => false,
    })
}

/// Check a declared function signature without supplying a derivative or value.
fn call_sorts(
    key: &str,
    result: &Sort,
    environment: &Environment,
    children: &[&Expression],
) -> bool {
    environment.declarations.iter().any(|declaration| {
        if declaration.key != key {
            return false;
        }
        let Sort::Function { inputs, output } = &declaration.sort else {
            return false;
        };
        inputs.len() == children.len()
            && *result == Sort::Value(output.clone())
            && inputs
                .iter()
                .zip(children)
                .all(|(input, child)| child.sort == Sort::Value(input.clone()))
    })
}

/// Check the shape of a represented calculus request without executing it.
fn calculus_sorts(operator: &Operator, result: &Sort, children: &[&Expression]) -> bool {
    use crate::model::Approach;

    let first = children.first().copied();
    match operator {
        Operator::Integral { definite } => {
            children.len() == (if *definite { 3 } else { 1 })
                && first.is_some_and(|child| child.sort == *result && numeric(result))
                && children.iter().skip(1).all(|child| numeric(&child.sort))
        }
        Operator::Sum => {
            children.len() == 3
                && first.is_some_and(|child| child.sort == *result && numeric(result))
                && children
                    .iter()
                    .skip(1)
                    .all(|child| child.sort == Sort::Value(AtomicSort::Integer))
        }
        Operator::Series { .. } => {
            children.len() == 2
                && first.is_some_and(|child| child.sort == *result && numeric(result))
                && children.get(1).is_some_and(|child| numeric(&child.sort))
        }
        Operator::Limit(approach) => {
            let arity = match approach {
                Approach::PositiveInfinity | Approach::NegativeInfinity => 1,
                Approach::Path => 3,
                _ => 2,
            };
            children.len() == arity
                && first.is_some_and(|child| child.sort == *result)
                && children.iter().skip(1).all(|child| numeric(&child.sort))
        }
        Operator::Set { .. } => {
            children.len() == 1
                && first.is_some_and(|child| child.sort == Sort::Value(AtomicSort::Boolean))
                && matches!(result, Sort::Set(_))
        }
        Operator::AlgebraicRoot {
            field, selection, ..
        } => {
            let selected = match selection {
                crate::model::RootSelection::All => Sort::Set(AtomicSort::Scalar(field.clone())),
                _ => Sort::Value(AtomicSort::Scalar(field.clone())),
            };
            children.len() == 1
                && first.is_some_and(|child| numeric(&child.sort))
                && *result == selected
        }
        _ => false,
    }
}

/// Borrow field and extents from a matrix sort.
const fn matrix(sort: &Sort) -> Option<(&Field, &crate::model::Extent, &crate::model::Extent)> {
    match sort {
        Sort::Value(AtomicSort::Matrix {
            field,
            rows,
            columns,
        }) => Some((field, rows, columns)),
        _ => None,
    }
}

/// Check ordered matrix shapes without scalar commutativity or numerical work.
fn matrix_sorts(
    operator: &Operator,
    result: &Sort,
    children: &[&Expression],
    budget: &Budget,
) -> Result<bool, Fault> {
    use crate::model::{Extent, MatrixOperation};

    let first = children.first().and_then(|child| matrix(&child.sort));
    let valid = match operator {
        Operator::Matrix { rows, columns } => {
            let count = u64::from(*rows)
                .checked_mul(u64::from(*columns))
                .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
            budget.check(BudgetKind::MatrixEntries, count, AT)?;
            matrix(result).is_some_and(|(field, actual_rows, actual_columns)| {
                *actual_rows == Extent::Literal(*rows)
                    && *actual_columns == Extent::Literal(*columns)
                    && size(children.len(), AT).ok() == Some(count)
                    && children
                        .iter()
                        .all(|child| child.sort == Sort::Value(AtomicSort::Scalar(field.clone())))
            })
        }
        Operator::Comprehension { rows, columns } => {
            children.len() == 1
                && matrix(result).is_some_and(|(field, actual_rows, actual_columns)| {
                    actual_rows == rows
                        && actual_columns == columns
                        && children.first().is_some_and(|child| {
                            child.sort == Sort::Value(AtomicSort::Scalar(field.clone()))
                        })
                })
        }
        Operator::Transpose => {
            children.len() == 1
                && first.is_some_and(|(field, rows, columns)| {
                    matrix(result) == Some((field, columns, rows))
                })
        }
        Operator::MatrixMultiply => {
            children.len() == 2
                && first.is_some_and(|(field, rows, columns)| {
                    children
                        .get(1)
                        .and_then(|child| matrix(&child.sort))
                        .is_some_and(|(right_field, right_rows, right_columns)| {
                            field == right_field
                                && columns == right_rows
                                && matrix(result) == Some((field, rows, right_columns))
                        })
                })
        }
        Operator::Entrywise(_) => {
            children.len() == 2
                && matrix(result).is_some()
                && children.iter().all(|child| child.sort == *result)
        }
        Operator::MatrixMap(operation) => {
            children.len() == 1
                && first.is_some_and(|(field, rows, columns)| {
                    rows == columns
                        && match operation {
                            MatrixOperation::Determinant | MatrixOperation::Trace => {
                                *result == Sort::Value(AtomicSort::Scalar(field.clone()))
                            }
                            MatrixOperation::Adjugate | MatrixOperation::Inverse => {
                                matrix(result) == first
                            }
                        }
                })
        }
        Operator::Spectral(selection) => spectral_sorts(*selection, result, children),
        _ => false,
    };
    Ok(valid)
}

/// Check the eigenvalue operand as well as the requested spectral result shape.
fn spectral_sorts(
    selection: crate::model::SpectralKind,
    result: &Sort,
    children: &[&Expression],
) -> bool {
    use crate::model::{Extent, SpectralKind};

    let Some((field, rows, columns)) = children.first().and_then(|child| matrix(&child.sort))
    else {
        return false;
    };
    if rows != columns {
        return false;
    }
    let eigenvalue = children
        .get(1)
        .is_some_and(|child| child.sort == Sort::Value(AtomicSort::Scalar(field.clone())));
    match selection {
        SpectralKind::Least | SpectralKind::Greatest => {
            children.len() == 1
                && *field == Field::Real
                && *result == Sort::Value(AtomicSort::Scalar(Field::Real))
        }
        SpectralKind::AlgebraicMultiplicity | SpectralKind::GeometricMultiplicity => {
            children.len() == 2 && eigenvalue && *result == Sort::Value(AtomicSort::Integer)
        }
        SpectralKind::Eigenvectors => {
            children.len() == 2
                && eigenvalue
                && *result
                    == Sort::Set(AtomicSort::Matrix {
                        field: field.clone(),
                        rows: rows.clone(),
                        columns: Extent::Literal(1),
                    })
        }
    }
}

/// Validate occurrence arity, identity, source boundaries and provenance before insertion.
pub(crate) fn occurrence(
    state: &State,
    value: &Occurrence,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let dependencies = crate::descriptor::occurrence_children(state, value.expression, budget)?;
    budget.work(size(value.children.len(), AT)?, AT)?;
    if value.children.len() != dependencies.len() {
        return Err(sort_error());
    }
    for (child, (_, expected)) in value.children.iter().zip(&dependencies) {
        if state.occurrence(*child)?.expression != *expected {
            return Err(sort_error());
        }
    }
    match (value.source, value.provenance) {
        (Some(source), None) => {
            let Vertex::Source(source) = state.node(source)? else {
                return Err(Fault::invalid(InvalidCode::Reference, AT));
            };
            if let Some(span) = value.span {
                let start = usize::try_from(span.start)
                    .map_err(|_| Fault::invalid(InvalidCode::Span, AT))?;
                let end =
                    usize::try_from(span.end).map_err(|_| Fault::invalid(InvalidCode::Span, AT))?;
                if start > end
                    || end > source.text.len()
                    || !source.text.is_char_boundary(start)
                    || !source.text.is_char_boundary(end)
                {
                    return Err(Fault::invalid(InvalidCode::Span, AT));
                }
            }
        }
        (None, Some(provenance)) if value.span.is_none() => {
            if !matches!(
                state.node(provenance)?,
                Vertex::Expression(_)
                    | Vertex::Occurrence(_)
                    | Vertex::SourceForm(_)
                    | Vertex::Application(_)
                    | Vertex::ScopedApplication(_)
            ) {
                return Err(Fault::invalid(InvalidCode::Reference, AT));
            }
        }
        _ => return Err(Fault::invalid(InvalidCode::Span, AT)),
    }
    let names = crate::descriptor::binding(state, value.expression)?
        .map_or(0, |binding| binding.sorts.len());
    if !value.binder_names.is_empty() && value.binder_names.len() != names {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Limits;
    use crate::domain_model::HeaderDraft;
    use crate::error::FaultKind;
    use crate::graph::{Cas, Handle};
    use crate::model::{Declaration, Extent, SymbolRelation};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// Use the unchanged admission policy for independent setup and checked calls.
    fn ledger() -> TestResult<Budget> {
        Ok(Budget::new(Limits::default())?)
    }

    /// A literal rectangle has no unresolved extent declaration dependencies.
    const fn rectangle(rows: u32, field: Field) -> Sort {
        Sort::Value(AtomicSort::Matrix {
            field,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(1),
        })
    }

    /// Each endpoint has an independent admitted header and an actual Free expression.
    fn symbol(cas: &mut Cas, key: &str, sort: Sort, scope: Scope) -> TestResult<(Handle, Handle)> {
        let drafts = [
            Draft::Header(HeaderDraft::Core(Environment {
                declarations: vec![Declaration {
                    key: key.into(),
                    display_name: key.into(),
                    sort: sort.clone(),
                }],
                ..Environment::default()
            })),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Free(key.into()),
                sort,
                header: Reference::Local(0),
                scope,
                binding: None,
                operands: Vec::new(),
            }),
        ];
        let handles = cas.admit(&drafts, &mut ledger()?)?;
        Ok((
            handles.first().ok_or("header")?.clone(),
            handles.get(1).ok_or("symbol")?.clone(),
        ))
    }

    /// A standard pair makes all mutation tests start from an admitted association shape.
    fn fixture() -> TestResult<(Cas, ExpressionDraft)> {
        let mut cas = Cas::new(Limits::default())?;
        let (header, result) = symbol(&mut cas, "result", rectangle(2, Field::Real), Vec::new())?;
        let (_, target) = symbol(&mut cas, "target", rectangle(2, Field::Real), Vec::new())?;
        let draft = ExpressionDraft {
            space: None,
            operator: Operator::SymbolRelation(SymbolRelation::Copy),
            sort: Sort::Value(AtomicSort::Boolean),
            header: Reference::Existing(header),
            scope: Vec::new(),
            binding: None,
            operands: vec![Reference::Existing(result), Reference::Existing(target)],
        };
        Ok((cas, draft))
    }

    /// Require the named structural refusal and prove the failed batch changed nothing.
    fn refuses(cas: &mut Cas, draft: ExpressionDraft, code: InvalidCode) -> TestResult {
        let before = cas.counts();
        let result = cas.admit(&[Draft::Expression(draft)], &mut ledger()?);
        assert!(
            matches!(result, Err(Fault { kind: FaultKind::Invalid(actual), .. }) if actual == code)
        );
        assert_eq!(cas.counts(), before);
        Ok(())
    }

    /// Scope, Boolean result and exact owner header are independent admission obligations.
    /// ´test:unit:symbol-relation-refuses-scope-sort-and-header´
    #[test]
    fn symbol_relation_refuses_scope_sort_and_header() -> TestResult {
        let (mut cas, original) = fixture()?;
        let mut scoped = original.clone();
        scoped.scope = vec![vec![AtomicSort::Integer]];
        refuses(&mut cas, scoped, InvalidCode::Scope)?;
        let mut result_sort = original.clone();
        result_sort.sort = Sort::Value(AtomicSort::Integer);
        refuses(&mut cas, result_sort, InvalidCode::Sort)?;
        let (other_header, scoped_symbol) = symbol(
            &mut cas,
            "scoped",
            rectangle(2, Field::Real),
            vec![vec![AtomicSort::Integer]],
        )?;
        let mut header = original.clone();
        header.header = Reference::Existing(other_header);
        refuses(&mut cas, header, InvalidCode::Context)?;
        let mut endpoint = original;
        endpoint.operands[1] = Reference::Existing(scoped_symbol);
        refuses(&mut cas, endpoint, InvalidCode::Scope)
    }

    /// Endpoints must be distinct prior Free symbols from this exact graph.
    /// ´test:unit:symbol-relation-refuses-invalid-endpoint-identities´
    #[test]
    fn symbol_relation_refuses_invalid_endpoint_identities() -> TestResult {
        let (mut cas, original) = fixture()?;
        let mut repeated = original.clone();
        repeated.operands[1] = repeated.operands[0].clone();
        refuses(&mut cas, repeated, InvalidCode::Reference)?;
        let mut forward = original.clone();
        forward.operands[1] = Reference::Local(0);
        refuses(&mut cas, forward, InvalidCode::Reference)?;
        let (_, foreign) = fixture()?;
        let mut external = original.clone();
        external.operands[1] = foreign.operands[1].clone();
        refuses(&mut cas, external, InvalidCode::ForeignHandle)?;
        let admitted = cas.admit(&[Draft::Expression(original.clone())], &mut ledger()?)?;
        let mut non_symbol = original.clone();
        non_symbol.operands[1] = Reference::Existing(admitted.first().ok_or("relation")?.clone());
        refuses(&mut cas, non_symbol, InvalidCode::Reference)?;
        let mut arity = original;
        arity.operands.pop();
        refuses(&mut cas, arity, InvalidCode::Reference)
    }

    /// Only literal Real matrices participate, and companions preserve the target rectangle.
    /// ´test:unit:symbol-relation-refuses-sorts-and-companion-extents´
    #[test]
    fn symbol_relation_refuses_sorts_and_companion_extents() -> TestResult {
        let (mut cas, original) = fixture()?;
        for (key, sort) in [
            ("scalar", Sort::Value(AtomicSort::Scalar(Field::Real))),
            ("complex", rectangle(2, Field::Complex)),
        ] {
            let (_, target) = symbol(&mut cas, key, sort, Vec::new())?;
            let mut draft = original.clone();
            draft.operands[1] = Reference::Existing(target);
            refuses(&mut cas, draft, InvalidCode::Sort)?;
        }
        let (_, target) = symbol(&mut cas, "selected", rectangle(3, Field::Real), Vec::new())?;
        let mut selected = original;
        selected.operands[1] = Reference::Existing(target);
        cas.admit(&[Draft::Expression(selected.clone())], &mut ledger()?)?;
        for kind in [
            SymbolRelation::DeterministicBound,
            SymbolRelation::NumericalError,
        ] {
            selected.operator = Operator::SymbolRelation(kind);
            refuses(&mut cas, selected.clone(), InvalidCode::Sort)?;
        }
        Ok(())
    }

    /// All three association tags round-trip and unknown kinds refuse canonical decoding.
    /// ´test:unit:symbol-relation-wire-kinds´
    #[test]
    fn symbol_relation_wire_kinds() -> TestResult {
        let (mut cas, original) = fixture()?;
        for kind in [
            SymbolRelation::Copy,
            SymbolRelation::DeterministicBound,
            SymbolRelation::NumericalError,
        ] {
            let operator = Operator::SymbolRelation(kind);
            let mut draft = original.clone();
            draft.operator = operator.clone();
            cas.admit(&[Draft::Expression(draft)], &mut ledger()?)?;
            let mut budget = ledger()?;
            let mut out = Encoder::new(&mut budget, AT);
            encoding::operator(&operator, &mut out)?;
            let bytes = out.finish();
            let decoded = crate::wire_read::Reader::new(&bytes, &mut budget)?.operator()?;
            assert_eq!(decoded, operator);
        }
        assert!(matches!(
            crate::wire_read::Reader::new(&[48, 3], &mut ledger()?)?.operator(),
            Err(Fault {
                kind: FaultKind::Invalid(InvalidCode::Encoding),
                ..
            })
        ));
        Ok(())
    }

    /// Association syntax supplies neither coefficient equality, normalization nor solving authority.
    /// ´test:unit:symbol-relation-has-no-equation-authority´
    #[test]
    fn symbol_relation_has_no_equation_authority() -> TestResult {
        let (mut cas, original) = fixture()?;
        let before = cas.state.proof_count();
        let handles = cas.admit(&[Draft::Expression(original)], &mut ledger()?)?;
        let relation = handles.first().ok_or("relation")?;
        let id = cas.checked(relation)?;
        assert!(crate::polynomial::equal(&cas.state, id, id, false, &mut ledger()?).is_err());
        assert!(crate::polynomial::normalize(&mut cas, id, &mut ledger()?).is_err());
        assert!(cas.solve_view(relation).is_err());
        assert_eq!(cas.state.proof_count(), before);
        Ok(())
    }

    /// Relation admission has exact Work and payload boundaries and rolls back below either.
    /// ´test:unit:symbol-relation-work-and-payload-boundaries´
    #[test]
    fn symbol_relation_work_and_payload_boundaries() -> TestResult {
        let (mut measured_cas, draft) = fixture()?;
        let mut measured = ledger()?;
        measured_cas.admit(&[Draft::Expression(draft)], &mut measured)?;
        let work = measured.usage().work;
        let payload = measured_cas.counts().payload_bytes;
        for (kind, needed) in [
            (BudgetKind::Work, work),
            (BudgetKind::PayloadBytes, payload),
        ] {
            for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
                let (mut cas, draft) = fixture()?;
                let before = cas.counts();
                let limits = if kind == BudgetKind::Work {
                    Limits {
                        work: limit,
                        ..Limits::default()
                    }
                } else {
                    Limits {
                        payload_bytes: limit,
                        ..Limits::default()
                    }
                };
                let result = cas.admit(&[Draft::Expression(draft)], &mut Budget::new(limits)?);
                assert_eq!(result.is_ok(), succeeds);
                if !succeeds {
                    assert_eq!(cas.counts(), before);
                }
            }
        }
        Ok(())
    }
}
