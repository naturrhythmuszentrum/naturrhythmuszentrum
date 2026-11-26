// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original-syntax and simultaneous-substitution checker.

use super::{AT, Request, Tag};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::lc_root;
use crate::graph::{Expression, LogicalId};
use crate::model::{AtomicSort, Branch, Field, Operator, PredicateKind, Sort};
use crate::{Cas, Fault, Handle, InvalidCode};

/// A structurally unsupported original cannot receive this theorem.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// The expected request is constructed by the caller, never from proposal bytes.
pub(super) fn verify(
    cas: &Cas,
    expected: &Request,
    root: &lc_root::Receipt,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if root.request(cas, budget)? != &expected.root {
        return Err(invalid());
    }
    verify_originals(cas, expected, budget)
}

/// Validate the receiving original statement before replay can publish a root judgment.
pub(super) fn verify_originals(
    cas: &Cas,
    expected: &Request,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if expected.revision != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    budget.check(BudgetKind::MatrixEntries, expected.sources.len() as u64, AT)?;
    budget.check(
        BudgetKind::MatrixEntries,
        expected.constraints.len() as u64,
        AT,
    )?;
    budget.check(
        BudgetKind::MatrixEntries,
        expected.substitution.len() as u64,
        AT,
    )?;
    if expected.substitution.len() != expected.root.inputs.len()
        || expected.root.inputs.is_empty()
        || expected
            .root
            .positions
            .iter()
            .any(|position| *position >= expected.root.inputs.len())
    {
        return Err(invalid());
    }
    if expected.context != expected.root.context
        || !expected.constraints.starts_with(&expected.root.constraints)
    {
        return Err(invalid());
    }
    let context_id = cas.checked(&expected.context, budget)?;
    let context = cas.state.context(context_id, budget)?;
    if !cas
        .state
        .region(context.admissible, budget)?
        .scope
        .is_empty()
    {
        return Err(invalid());
    }
    let header = context.header_id;
    for source in &expected.sources {
        if expected.root.endpoints.contains(source) {
            return Err(invalid());
        }
        let expression = real(cas, source, header, budget)?;
        if !matches!(expression.operator, Operator::Free(_)) {
            return Err(invalid());
        }
    }
    for constraint in &expected.constraints {
        budget.work(1, AT)?;
        let expression = cas
            .state
            .expression(cas.checked(constraint, budget)?, budget)?;
        if expression.header != header
            || !expression.scope.is_empty()
            || expression.sort != Sort::Value(AtomicSort::Boolean)
        {
            return Err(invalid());
        }
    }
    if expected.tag == Tag::Box {
        let l = &expected.substitution[expected.root.positions[0]];
        let c = &expected.substitution[expected.root.positions[1]];
        if l == c
            || !matches!(
                real(cas, l, context.header_id, budget)?.operator,
                Operator::Free(_)
            )
            || !matches!(
                real(cas, c, context.header_id, budget)?.operator,
                Operator::Free(_)
            )
        {
            return Err(invalid());
        }
    }
    for (index, source) in expected.substitution.iter().enumerate() {
        budget.work(1, AT)?;
        for (earlier, prior) in expected.root.inputs[..index].iter().enumerate() {
            budget.work(1, AT)?;
            if prior == &expected.root.inputs[index] && source != &expected.substitution[earlier] {
                return Err(invalid());
            }
        }
        source_expression(cas, source, expected, header, budget, 0)?;
    }
    targets(cas, expected, header, budget)
}

/// Read one original Real expression under the declared source header.
fn real<'a>(
    cas: &'a Cas,
    handle: &Handle,
    header: LogicalId,
    budget: &mut Budget,
) -> Result<&'a Expression, Fault> {
    budget.work(1, AT)?;
    let expression = cas.state.expression(cas.checked(handle, budget)?, budget)?;
    if expression.header != header
        || !expression.scope.is_empty()
        || expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
    {
        return Err(invalid());
    }
    Ok(expression)
}

/// Classification of one admitted source map.
#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceKind {
    /// Rational affine expression of the original declarations.
    Affine,
    /// Positive monomial or positive root with explicit domain predicates.
    PositiveMap,
}

/// A sign premise must be an original Boolean constraint on this exact DAG node.
fn positive(
    cas: &Cas,
    request: &Request,
    operand: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for condition in &request.constraints {
        budget.work(1, AT)?;
        let node = cas
            .state
            .expression(cas.checked(condition, budget)?, budget)?;
        if node.operator == Operator::Predicate(PredicateKind::Positive)
            && node.operands == [operand]
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Admit only rational affine syntax and explicitly positive monomial/root maps.
/// The graph is acyclic; the depth bound also protects the caller ledger.
fn source_expression(
    cas: &Cas,
    handle: &Handle,
    request: &Request,
    header: LogicalId,
    budget: &mut Budget,
    depth: usize,
) -> Result<SourceKind, Fault> {
    budget.check(BudgetKind::MatrixEntries, depth as u64, AT)?;
    let expression = real(cas, handle, header, budget)?;
    let operands = &expression.operands;
    match (&request.tag, &expression.operator) {
        (_, Operator::Free(_)) => {
            if !operands.is_empty() || !request.sources.iter().any(|s| s == handle) {
                return Err(invalid());
            }
            return Ok(SourceKind::Affine);
        }
        (_, Operator::Rational(_)) if operands.is_empty() => return Ok(SourceKind::Affine),
        (Tag::MappedCover, Operator::Add | Operator::Sub | Operator::Mul | Operator::Div)
            if operands.len() == 2 => {}
        (Tag::MappedCover, Operator::GeneralPower(Branch::StrictReal)) if operands.len() == 2 => {
            let exponent = cas.state.expression(operands[1], budget)?;
            if !matches!(&exponent.operator, Operator::Rational(n) if n.is_half()) {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    let left = cas.handle(operands[0]);
    let left_kind = source_expression(cas, &left, request, header, budget, depth + 1)?;
    if matches!(expression.operator, Operator::GeneralPower(_)) {
        if !positive(cas, request, operands[0], budget)? {
            return Err(invalid());
        }
        return Ok(SourceKind::PositiveMap);
    }
    let right = cas.handle(operands[1]);
    let right_kind = source_expression(cas, &right, request, header, budget, depth + 1)?;
    match expression.operator {
        Operator::Add | Operator::Sub
            if left_kind == SourceKind::Affine && right_kind == SourceKind::Affine =>
        {
            Ok(SourceKind::Affine)
        }
        Operator::Mul
            if left_kind == SourceKind::Affine
                && right_kind == SourceKind::Affine
                && (matches!(
                    cas.state.expression(operands[0], budget)?.operator,
                    Operator::Rational(_)
                ) || matches!(
                    cas.state.expression(operands[1], budget)?.operator,
                    Operator::Rational(_)
                )) =>
        {
            Ok(SourceKind::Affine)
        }
        Operator::Mul | Operator::Div
            if positive(cas, request, operands[0], budget)?
                && positive(cas, request, operands[1], budget)? =>
        {
            Ok(SourceKind::PositiveMap)
        }
        _ => Err(invalid()),
    }
}

/// Reconstruct every substituted target from original ordered source expressions.
fn targets(
    cas: &Cas,
    request: &Request,
    header: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let product = cas.checked(&request.targets[0], budget)?;
    let root = cas.checked(&request.targets[1], budget)?;
    let l = cas.checked(&request.substitution[request.root.positions[0]], budget)?;
    let c = cas.checked(&request.substitution[request.root.positions[1]], budget)?;
    let expression = real(cas, &request.targets[0], header, budget)?;
    if expression.operator != Operator::Mul || expression.operands != [l, c] {
        return Err(invalid());
    }
    let expression = real(cas, &request.targets[1], header, budget)?;
    if expression.operator != Operator::GeneralPower(Branch::StrictReal)
        || expression.operands.len() != 2
        || expression.operands[0] != product
    {
        return Err(invalid());
    }
    let exponent = cas.state.expression(expression.operands[1], budget)?;
    if !matches!(&exponent.operator, Operator::Rational(n) if n.is_half()) {
        return Err(invalid());
    }
    let expression = real(cas, &request.targets[2], header, budget)?;
    if expression.operator != Operator::Div
        || expression.operands.len() != 2
        || expression.operands[1] != root
    {
        return Err(invalid());
    }
    let one = cas.state.expression(expression.operands[0], budget)?;
    if !matches!(&one.operator, Operator::Rational(n) if n == &crate::Rational::one()) {
        return Err(invalid());
    }
    let expression = real(cas, &request.targets[3], header, budget)?;
    if expression.operator != Operator::Sub
        || expression.operands.len() != 2
        || expression.operands[1] != product
    {
        return Err(invalid());
    }
    let square = cas.state.expression(expression.operands[0], budget)?;
    if square.operator != Operator::Mul || square.operands != [root, root] {
        return Err(invalid());
    }
    Ok(())
}
