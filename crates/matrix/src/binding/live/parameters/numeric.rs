// SPDX-License-Identifier: AGPL-3.0-only
//! Outward affine parameter contraction preserves source error and both covariance factors.

use super::{AT, CompanionExecution, CompanionExecutionError as Error, Heap, HeapError};
use nrz_bindings::readings::algebra::{
    Rational,
    limits::{BudgetKind, BudgetUsage},
};
use nrz_bindings::{Budget, readings::CompanionReadingKind};
use nrz_scalar::{
    ArithmeticContext, DecodedLiteral, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};

/// Complete private output parameter arrays, exposed together after successful evaluation.
#[derive(Clone, Debug)]
pub(super) struct Values<P: Scalar> {
    /// Finite represented output parameter values.
    pub(super) centers: Vec<P>,
    /// Outward numerical-error radii for the original exact parameter target.
    pub(super) errors: Vec<P>,
}

/// Coefficient import precedes every numerical source access and has its own ledger snapshot.
pub(super) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    execution: &CompanionExecution<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(Values<P>, BudgetUsage), Error> {
    let map = execution.exact().target().map();
    let inputs = usize::try_from(map.inputs()).map_err(|_| HeapError::ExtentOverflow)?;
    let outputs = usize::try_from(map.outputs()).map_err(|_| HeapError::ExtentOverflow)?;
    let bytes = storage::<P>(inputs, outputs, map.coefficients().len())?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        let decoded = coefficients::<P>(map.coefficients(), execution.literals(), context, budget)?;
        let before_evaluation = budget.usage();
        let values = heap.reading_entries(&execution.sources()[0])?;
        let errors = heap.reading_entries(&execution.sources()[1])?;
        check(
            &values,
            &errors,
            inputs,
            execution.exact().target().kind(),
            budget,
        )?;
        let mut result = Values {
            centers: crate::binding::buffer(outputs)?,
            errors: crate::binding::buffer(outputs)?,
        };
        for row in 0..outputs {
            let start = row.checked_mul(inputs).ok_or(HeapError::ExtentOverflow)?;
            let (center, error) = contraction(
                &decoded[start..start + inputs],
                &values,
                &errors,
                context,
                budget,
            )?;
            result.centers.push(center);
            result.errors.push(error);
        }
        let output = storage::<P>(0, outputs, 0)?
            .checked_add(execution.bytes())
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(BudgetKind::OutputBytes, output, AT)?;
        Ok((result, before_evaluation))
    })
}

/// All source entries remain checked even when the exact map multiplies them by zero.
fn check<P: Scalar>(
    values: &[P],
    errors: &[P],
    count: usize,
    kind: CompanionReadingKind,
    budget: &mut Budget,
) -> Result<(), Error> {
    if values.len() != count || errors.len() != count {
        return Err(Error::Identity);
    }
    for (value, error) in values.iter().zip(errors) {
        budget.work(1, AT)?;
        let negative_radius = matches!(kind, CompanionReadingKind::Radius(_)) && *value < P::ZERO;
        if *error < P::ZERO || negative_radius {
            return Err(Error::Identity);
        }
    }
    Ok(())
}

/// Rational spelling is prepaid under the same finite caller ledger as the contraction.
pub(in crate::binding::live) fn coefficients<P: Scalar>(
    values: &[Rational],
    policy: LiteralLimits,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Vec<DecodedLiteral<P>>, Error> {
    let mut result = crate::binding::buffer(values.len())?;
    for value in values {
        let text = value.exact_string(budget)?;
        let work = u64::try_from(text.len())
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(12)
            .and_then(|n| n.checked_add(8))
            .ok_or(HeapError::ExtentOverflow)?;
        budget.work(work, AT)?;
        let limits = LiteralLimits::new(
            policy.max_digits(),
            policy.max_exponent(),
            policy.max_work().min(work),
        );
        result.push(NumericLiteral::new(&text, limits)?.decode(context)?);
    }
    Ok(result)
}

/// Enclose each coefficient import and source numerical error before summing in fixed order.
pub(in crate::binding::live) fn contraction<P: Scalar>(
    coefficients: &[DecodedLiteral<P>],
    values: &[P],
    errors: &[P],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(P, P), Error> {
    let mut candidate = P::ZERO;
    let mut enclosure = Enclosure::singleton(Finite::ZERO);
    for ((coefficient, value), error) in coefficients.iter().zip(values).zip(errors) {
        budget.work(12, AT)?;
        let input = Enclosure::singleton(Finite::new(*value)?)
            .add(Enclosure::new(-*error, *error)?, context)?;
        let product = coefficient.enclosure().mul(input, context)?;
        enclosure = enclosure.add(product, context)?;
        candidate = Finite::new(candidate + coefficient.center().get() * *value)?.get();
    }
    let error = enclosure
        .displacement_from(Finite::new(candidate)?, context)?
        .get();
    Ok((candidate, error))
}

/// Charge simultaneous complete arrays, without traversing an unused empty matrix axis.
fn storage<P: Scalar>(inputs: usize, outputs: usize, coefficients: usize) -> Result<u64, Error> {
    let values = inputs
        .checked_add(outputs)
        .and_then(|n| n.checked_mul(2 * std::mem::size_of::<P>()))
        .ok_or(HeapError::ExtentOverflow)?;
    let literals = coefficients
        .checked_mul(std::mem::size_of::<DecodedLiteral<P>>())
        .ok_or(HeapError::ExtentOverflow)?;
    u64::try_from(
        values
            .checked_add(literals)
            .and_then(|n| n.checked_add(256))
            .ok_or(HeapError::ExtentOverflow)?,
    )
    .map_err(|_| HeapError::ExtentOverflow.into())
}
