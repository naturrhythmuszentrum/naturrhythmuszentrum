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
        Draft::Source { key, text } => {
            out.name(key)?;
            out.count(text.len())?;
            out.raw(text.as_bytes())?;
        }
        Draft::Cases(_)
        | Draft::Header(_)
        | Draft::Context(_)
        | Draft::Region(_)
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
        } => {
            encoding::predicate(kind, out)?;
            reference(header, out)?;
            encoding::scope(scope, out)?;
            out.count(arguments.len())?;
            for argument in arguments {
                reference(argument, out)?;
            }
        }
        Draft::Expression(value) => {
            encoding::operator(&value.operator, out)?;
            encoding::sort(&value.sort, out)?;
            reference(&value.header, out)?;
            encoding::scope(&value.scope, out)?;
            encoding::binding(value.binding.as_ref(), out)?;
            out.count(value.operands.len())?;
            for operand in &value.operands {
                reference(operand, out)?;
            }
        }
        Draft::Occurrence(value) => {
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
        }
        Draft::Request {
            task,
            premises,
            context,
            inputs,
            limits,
        } => {
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
        }
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
    budget: &Budget,
) -> Result<(), Fault> {
    let same = children.iter().all(|child| child.sort == *result);
    let first = children.first().copied();
    let valid = match operator {
        Operator::Cases => false,
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
        Operator::Neg => children.len() == 1 && same && numeric(result),
        Operator::Add | Operator::Sub | Operator::Mul => {
            children.len() == 2 && same && numeric(result)
        }
        Operator::Div => {
            children.len() == 2 && same && matches!(result, Sort::Value(AtomicSort::Scalar(_)),)
        }
        Operator::IntegerPower(exponent) => {
            children.len() == 1
                && same
                && numeric(result)
                && (*exponent >= 0 || matches!(result, Sort::Value(AtomicSort::Scalar(_))))
        }
        Operator::GeneralPower(_) => {
            children.len() == 2
                && first.is_some_and(|child| child.sort == *result)
                && matches!(result, Sort::Value(AtomicSort::Scalar(_)))
                && children.get(1).is_some_and(|child| numeric(&child.sort))
        }
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
        Operator::Free(key) => environment
            .declarations
            .iter()
            .any(|declaration| declaration.key == *key && declaration.sort == *result),
        Operator::Bound { depth, slot } => {
            let depth = usize::try_from(*depth).map_err(|_| sort_error())?;
            let slot = usize::try_from(*slot).map_err(|_| sort_error())?;
            let frame = scope
                .len()
                .checked_sub(depth)
                .and_then(|n| n.checked_sub(1))
                .and_then(|index| scope.get(index));
            frame
                .and_then(|frame| frame.get(slot))
                .is_some_and(|sort| *result == Sort::Value(sort.clone()))
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
    let expression = state.expression(value.expression)?;
    budget.work(size(value.children.len(), AT)?, AT)?;
    if value.children.len() != expression.operands.len() {
        return Err(sort_error());
    }
    for (child, expected) in value.children.iter().zip(&expression.operands) {
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
                Vertex::Expression(_) | Vertex::Occurrence(_)
            ) {
                return Err(Fault::invalid(InvalidCode::Reference, AT));
            }
        }
        _ => return Err(Fault::invalid(InvalidCode::Span, AT)),
    }
    let names = expression
        .binding
        .as_ref()
        .map_or(0, |binding| binding.sorts.len());
    if !value.binder_names.is_empty() && value.binder_names.len() != names {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    Ok(())
}
