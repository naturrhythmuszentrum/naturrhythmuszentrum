// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite arithmetic and exact ordered-premise joins.

use super::{SeriesCertificate as C, SeriesObject, SeriesStatement as S};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, Vertex, size};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use std::cmp::Ordering;

/// One attribution for finite object checking, distinct from generic scalar conversion.
pub(crate) const AT: Location = Location::new("formal series arithmetic");

/// A false row or mismatched premise is malformed mathematical evidence.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Well-formed capabilities outside object arithmetic retain their capability classification.
pub(crate) const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::SeriesUnsupported, AT)
}

/// Precision is checked before arity or workspace sizing can misclassify an unavailable request.
pub(crate) fn precision(cas: &Cas, value: &S, budget: &mut Budget) -> Result<(), Fault> {
    if let S::Reciprocal { input, .. } = value {
        super::reciprocal::nonunit(cas, input, budget)?;
    }
    if let S::EvaluatedResidual {
        subject,
        expression,
        dependent,
        ..
    } = value
    {
        super::residual::preflight(cas, subject, expression, dependent, budget)?;
    }
    if let S::Compose {
        left, right, order, ..
    } = value
    {
        let outer = cas.series_object(left)?.order;
        let inner = cas.series_object(right)?.order;
        if *order == 0 || *order > outer.min(inner) {
            return Err(super::compose::precision(outer, inner, *order, budget)?);
        }
    }
    Ok(())
}

/// Borrow an exact admitted coefficient without any input-sized copy.
pub(crate) fn number<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a Rational, Fault> {
    let expression = cas.state.expression(cas.checked(handle)?)?;
    let Operator::Rational(value) = &expression.operator else {
        return Err(unsupported());
    };
    if expression.sort != Sort::Value(AtomicSort::Scalar(crate::model::Field::Real)) {
        return Err(invalid());
    }
    Ok(value)
}

/// The statement determines the complete ordered arity before any proof payload is copied.
pub(crate) fn premise_count(cas: &Cas, value: &S) -> Result<usize, Fault> {
    match value {
        S::Source(value) => Ok(value.clause.arity()),
        S::Reciprocal { .. } | S::Truncate { .. } | S::Coefficient { .. } | S::Derive { .. } => {
            Ok(0)
        }
        S::EvaluatedResidual { .. } | S::Scale { .. } | S::Integrate { .. } => Ok(1),
        S::Add {
            left, right, order, ..
        }
        | S::Compose {
            left, right, order, ..
        }
        | S::Mul {
            left, right, order, ..
        } => Ok(usize::from(cas.series_object(left)?.order != *order)
            + usize::from(cas.series_object(right)?.order != *order)),
    }
}

/// Every metadata component participates, including the full declared binder frame.
pub(super) fn compatible(
    left: &SeriesObject<Handle>,
    right: &SeriesObject<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::encoding::scope(&left.scope, &mut out)?;
    crate::encoding::scope(&right.scope, &mut out)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.work(bytes, AT)?;
    if left.field != right.field
        || left.centre != right.centre
        || left.variable != right.variable
        || left.scope != right.scope
        || left.context != right.context
    {
        return Err(invalid());
    }
    Ok(())
}

/// Scalar interpretation shares the exact original declaration, scope and context.
fn scalar(
    cas: &Cas,
    basis: &SeriesObject<Handle>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let expression = cas.state.expression(cas.checked(handle)?)?;
    let context = cas.state.context(cas.checked(&basis.context)?)?;
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::encoding::scope(&expression.scope, &mut out)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.work(bytes, AT)?;
    let header = context.header_id;
    if expression.header != header || expression.scope != basis.scope {
        return Err(invalid());
    }
    number(cas, handle)?.check(budget, AT)
}

/// Constant independence precedes rational capability, preserving the malformed-scope split.
pub(super) fn constant(
    cas: &Cas,
    basis: &SeriesObject<Handle>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    super::independent(
        &cas.state,
        cas.checked(handle)?,
        cas.checked(&basis.variable)?,
        budget,
    )?;
    scalar(cas, basis, handle, budget)
}

/// Validate typed roles even when the payload is only a caller's expected judgment.
pub(crate) fn roles(cas: &Cas, value: &S, budget: &mut Budget) -> Result<(), Fault> {
    precision(cas, value, budget)?;
    if let S::Source(value) = value {
        return super::source::roles(cas, value, budget);
    }
    if matches!(value, S::EvaluatedResidual { .. }) {
        return super::residual::roles(cas, value, budget);
    }
    let input = cas.series_object(value.input())?;
    input.preflight(budget)?;
    match value {
        S::Source(_) | S::EvaluatedResidual { .. } => return Err(invalid()),
        S::Reciprocal { output, .. } | S::Truncate { output, .. } | S::Derive { output, .. } => {
            compatible(input, cas.series_object(output)?, budget)?;
        }
        S::Integrate {
            constant: value,
            output,
            ..
        } => {
            constant(cas, input, value, budget)?;
            compatible(input, cas.series_object(output)?, budget)?;
        }
        S::Compose { right, output, .. }
        | S::Add { right, output, .. }
        | S::Mul { right, output, .. } => {
            compatible(input, cas.series_object(right)?, budget)?;
            compatible(input, cas.series_object(output)?, budget)?;
        }
        S::Scale {
            scalar: factor,
            output,
            ..
        } => {
            scalar(cas, input, factor, budget)?;
            compatible(input, cas.series_object(output)?, budget)?;
        }
        S::Coefficient { value, .. } => scalar(cas, input, value, budget)?,
    }
    Ok(())
}

/// Exact equality never trusts a proposed arithmetic result or an unmetered large comparison.
pub(super) fn equal(left: &Rational, right: &Rational, budget: &mut Budget) -> Result<(), Fault> {
    if left.compare(right, budget)? != Ordering::Equal {
        return Err(invalid());
    }
    Ok(())
}

/// Bounds precede the vector access, and each visited coefficient consumes `JetEntries`.
pub(super) fn coefficient<'a>(
    cas: &'a Cas,
    object: &SeriesObject<Handle>,
    index: u32,
    budget: &mut Budget,
) -> Result<&'a Rational, Fault> {
    if index >= object.order {
        return Err(unsupported());
    }
    budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
    number(
        cas,
        object
            .coefficients
            .get(usize::try_from(index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?,
    )
}

/// Higher-order inputs consume precisely their ordered checked restriction, never a lookalike.
fn restriction<'a>(
    cas: &'a Cas,
    original: &Handle,
    order: u32,
    premises: &[CheckedApplication],
    slot: &mut usize,
) -> Result<&'a SeriesObject<Handle>, Fault> {
    let object = cas.series_object(original)?;
    if object.order == order {
        return Ok(object);
    }
    let premise = premises.get(*slot).ok_or_else(invalid)?;
    *slot = slot.checked_add(1).ok_or_else(invalid)?;
    let Statement::Series(value) = &premise.judgment.statement else {
        return Err(invalid());
    };
    let S::Truncate { input, output } = value.as_ref() else {
        return Err(invalid());
    };
    if input != original || premise.rule != crate::rule::Primitive::SeriesTruncate {
        return Err(invalid());
    }
    let restricted = cas.series_object(output)?;
    if restricted.order != order {
        return Err(invalid());
    }
    Ok(restricted)
}

/// Check the ordinary scalar-definedness slot without manufacturing or dropping conditions.
pub(super) fn defined(
    cas: &Cas,
    factor: &Handle,
    basis: &SeriesObject<Handle>,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let [premise] = premises else {
        return Err(invalid());
    };
    let Statement::PredicateFact { region, predicate } = &premise.judgment.statement else {
        return Err(invalid());
    };
    let context = cas.state.context(cas.checked(&basis.context)?)?;
    let Vertex::Condition(condition) = cas.state.node(cas.checked(predicate)?)? else {
        return Err(invalid());
    };
    if region.logical_id() != context.admissible
        || condition.kind != PredicateKind::Defined
        || condition.arguments.as_slice() != [cas.checked(factor)?]
        || condition.header != context.header_id
        || condition.scope != basis.scope
    {
        return Err(invalid());
    }
    Ok(())
}

/// Rebuild every coefficient range and convolution pair independently of certificate rows.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if matches!(&draft.conclusion, Statement::Series(value) if matches!(value.as_ref(), S::EvaluatedResidual { .. }))
    {
        return super::residual::check(cas, draft, premises, budget);
    }
    if matches!(&draft.conclusion, Statement::Series(value) if matches!(value.as_ref(), S::Source(_)))
    {
        return super::source::check(cas, draft, premises, budget);
    }
    let (Statement::Series(value), Certificate::Series(certificate)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    if value.primitive() != draft.rule || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    if certificate.form() != 0 {
        return Err(unsupported());
    }
    roles(cas, value, budget)?;
    let input = cas.series_object(value.input())?;
    if draft.context != input.context
        || premises.iter().any(|p| p.judgment.context != draft.context)
    {
        return Err(invalid());
    }
    let mut out = crate::encoding::Encoder::new(budget, AT);
    super::codec::certificate(certificate, &mut out)?;
    let encoded = size(out.finish().len(), AT)?;
    let workspace = arithmetic_workspace(cas, value, budget)?;
    let bytes = encoded
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(workspace))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    let retained = cas.judgment_reservation(cas.retained_judgment_bytes(), budget, AT)?;
    let bytes = bytes
        .checked_add(retained)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.with_retained_payload(bytes, AT, |budget| {
        arithmetic(cas, value, certificate, premises, budget)
    })?;
    let mut conditions = ScopedConditions::default();
    for premise in premises {
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    Ok(conditions)
}

/// Bound unreduced products and partial sums from actual admitted coefficient extents.
fn arithmetic_workspace(cas: &Cas, value: &S, budget: &mut Budget) -> Result<u64, Fault> {
    let input = cas.series_object(value.input())?;
    if matches!(value, S::Reciprocal { .. }) {
        return super::reciprocal::workspace(input.order, budget);
    }
    let mut bits = 1_u64;
    let other = match value {
        S::Compose { right, .. } | S::Add { right, .. } | S::Mul { right, .. } => Some(right),
        _ => None,
    };
    for handle in std::iter::once(value.input()).chain(other) {
        for coefficient in &cas.series_object(handle)?.coefficients {
            budget.work(1, AT)?;
            let (a, b) = number(cas, coefficient)?.component_bits();
            bits = bits.max(a).max(b);
        }
    }
    if let S::Scale { scalar, .. } = value {
        budget.work(1, AT)?;
        let (a, b) = number(cas, scalar)?.component_bits();
        bits = bits.max(a).max(b);
    }
    if let S::Integrate { constant, .. } = value {
        budget.work(1, AT)?;
        let (a, b) = number(cas, constant)?.component_bits();
        bits = bits.max(a).max(b);
    }
    if matches!(value, S::Derive { .. } | S::Integrate { .. }) {
        bits = bits.max(u64::from(input.order.bit_width()));
    }
    if let S::Compose { order, .. } = value {
        return super::compose::workspace(*order, bits);
    }
    bits.checked_mul(u64::from(input.order))
        .and_then(|v| v.checked_mul(64))
        .and_then(|v| v.checked_add(1024))
        .ok_or_else(invalid)
}

/// The finite matcher consumes every row and every proof slot exactly once.
fn arithmetic(
    cas: &Cas,
    value: &S,
    certificate: &C,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let input = cas.series_object(value.input())?;
    match (value, certificate) {
        (S::Reciprocal { .. }, _) => {
            return super::reciprocal::check(cas, value, certificate, premises, budget);
        }
        (S::Compose { .. }, _) => {
            return super::compose::check(cas, value, certificate, premises, budget);
        }
        (S::Derive { .. } | S::Integrate { .. }, _) => {
            return super::calculus::check(cas, value, certificate, premises, budget);
        }
        (S::Truncate { output, .. }, C::Truncate { coefficients, .. }) => {
            if !premises.is_empty() {
                return Err(invalid());
            }
            let output = cas.series_object(output)?;
            if output.order > input.order {
                return Err(unsupported());
            }
            if size(coefficients.len(), AT)? != u64::from(output.order) {
                return Err(invalid());
            }
            for (index, claimed) in coefficients.iter().enumerate() {
                let index = u32::try_from(index).map_err(|_| invalid())?;
                equal(coefficient(cas, input, index, budget)?, claimed, budget)?;
                equal(claimed, coefficient(cas, output, index, budget)?, budget)?;
            }
        }
        (
            S::Coefficient { index, value, .. },
            C::Coefficient {
                index: claimed,
                value: expected,
                ..
            },
        ) => {
            if !premises.is_empty() || index != claimed {
                return Err(invalid());
            }
            equal(coefficient(cas, input, *index, budget)?, expected, budget)?;
            equal(expected, number(cas, value)?, budget)?;
        }
        (S::Scale { scalar, output, .. }, C::Scale { rows, .. }) => {
            defined(cas, scalar, input, premises)?;
            let output = cas.series_object(output)?;
            if output.order != input.order || size(rows.len(), AT)? != u64::from(input.order) {
                return Err(invalid());
            }
            for (index, row) in rows.iter().enumerate() {
                let index = u32::try_from(index).map_err(|_| invalid())?;
                if row.index != index {
                    return Err(invalid());
                }
                let a = number(cas, scalar)?;
                let b = coefficient(cas, input, index, budget)?;
                equal(&row.left, a, budget)?;
                equal(&row.right, b, budget)?;
                equal(&a.mul(b, budget)?, &row.result, budget)?;
                equal(
                    &row.result,
                    coefficient(cas, output, index, budget)?,
                    budget,
                )?;
            }
        }
        _ => return binary_arithmetic(cas, value, certificate, premises, budget),
    }
    Ok(())
}

/// Check the complete coefficientwise or convolution rows after ordered restriction joins.
fn binary_arithmetic(
    cas: &Cas,
    value: &S,
    certificate: &C,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    match (value, certificate) {
        (
            S::Add {
                left,
                right,
                output,
                order,
                default_order,
            },
            C::Add {
                order: claimed,
                default_order: default,
                rows,
                ..
            },
        ) => {
            let (left, right, output) = binary(
                cas,
                [left, right, output],
                *order,
                *default_order,
                premises,
                budget,
            )?;
            if order != claimed
                || default_order != default
                || size(rows.len(), AT)? != u64::from(*order)
            {
                return Err(invalid());
            }
            for (index, row) in rows.iter().enumerate() {
                let index = u32::try_from(index).map_err(|_| invalid())?;
                if row.index != index {
                    return Err(invalid());
                }
                let a = coefficient(cas, left, index, budget)?;
                let b = coefficient(cas, right, index, budget)?;
                equal(a, &row.left, budget)?;
                equal(b, &row.right, budget)?;
                equal(&a.add(b, budget)?, &row.result, budget)?;
                equal(
                    &row.result,
                    coefficient(cas, output, index, budget)?,
                    budget,
                )?;
            }
        }
        (
            S::Mul {
                left,
                right,
                output,
                order,
                default_order,
            },
            C::Mul {
                order: claimed,
                default_order: default,
                rows,
                ..
            },
        ) => {
            let (left, right, output) = binary(
                cas,
                [left, right, output],
                *order,
                *default_order,
                premises,
                budget,
            )?;
            let count = product_count(*order)?;
            budget.check(BudgetKind::JetEntries, count, AT)?;
            if order != claimed || default_order != default || size(rows.len(), AT)? != count {
                return Err(invalid());
            }
            let mut rows = rows.iter();
            for degree in 0..*order {
                let mut sum = Rational::zero();
                for i in 0..=degree {
                    let j = degree - i;
                    let row = rows.next().ok_or_else(invalid)?;
                    if (row.degree, row.left_index, row.right_index) != (degree, i, j) {
                        return Err(invalid());
                    }
                    let a = coefficient(cas, left, i, budget)?;
                    let b = coefficient(cas, right, j, budget)?;
                    equal(a, &row.left, budget)?;
                    equal(b, &row.right, budget)?;
                    let product = a.mul(b, budget)?;
                    equal(&product, &row.product, budget)?;
                    sum = sum.add(&product, budget)?;
                    equal(&sum, &row.sum, budget)?;
                }
                equal(&sum, coefficient(cas, output, degree, budget)?, budget)?;
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

/// A complete triangular convolution includes every zero term.
fn product_count(order: u32) -> Result<u64, Fault> {
    let n = u64::from(order);
    Ok(n.checked_mul(n.checked_add(1).ok_or_else(invalid)?)
        .ok_or_else(invalid)?
        / 2)
}

/// Three borrowed objects sharing the explicitly checked truncation order.
type BinaryObjects<'a> = (
    &'a SeriesObject<Handle>,
    &'a SeriesObject<Handle>,
    &'a SeriesObject<Handle>,
);

/// Record the min-order default and require explicit truncation proofs for each larger input.
pub(super) fn binary<'a>(
    cas: &'a Cas,
    objects: [&Handle; 3],
    order: u32,
    default: bool,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<BinaryObjects<'a>, Fault> {
    let [a, b, out] = objects;
    let left = cas.series_object(a)?;
    let right = cas.series_object(b)?;
    let output = cas.series_object(out)?;
    if order == 0 || order > left.order.min(right.order) {
        return Err(unsupported());
    }
    budget.check(BudgetKind::JetOrder, u64::from(order - 1), AT)?;
    if output.order != order || (default && order != left.order.min(right.order)) {
        return Err(invalid());
    }
    let mut slot = 0;
    let left = restriction(cas, a, order, premises, &mut slot)?;
    let right = restriction(cas, b, order, premises, &mut slot)?;
    if slot != premises.len() {
        return Err(invalid());
    }
    compatible(left, output, budget)?;
    compatible(right, output, budget)?;
    Ok((left, right, output))
}
