// SPDX-License-Identifier: AGPL-3.0-only
//! Execute native scalar laws and convert their exact records without rounded endpoint arithmetic.

use super::{AT, LiveDecisionError};
use crate::FrozenDecisionInputs;
use nrz_bindings::readings::algebra::{
    ExactConstant, Rational,
    decision::{ScalarKernelInput, ScalarKernelInputValue, ScalarKernelLaw, ScalarKernelRecord},
    pi_bounds::PiBoundCertificate,
    syntax::Constant,
};
use nrz_bindings::{Budget, Declaration, Role};
use nrz_scalar::{
    ArithmeticContext, Dyadic, Enclosure, Finite, KernelBudget, KernelCertificate,
    KernelEvaluation, KernelLaw, KernelUsage, Scalar,
};

/// Prepay the bounded four-operation schedule, including every fixed-size export.
pub(super) fn execute<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    order: [usize; 2],
    arithmetic: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<([KernelCertificate; 4], KernelUsage), LiveDecisionError> {
    // Two sums (116), reciprocal (44), product (60), four exports (512).
    budget.work(732, AT)?;
    let mut scalar = KernelBudget::new(732);
    let first = input_box(
        frozen.operands()[order[0]].coordinates(),
        arithmetic,
        &mut scalar,
    )?;
    let second = input_box(
        frozen.operands()[order[1]].coordinates(),
        arithmetic,
        &mut scalar,
    )?;
    let reciprocal = KernelEvaluation::reciprocal(second.enclosure(), arithmetic, &mut scalar)?;
    let product = KernelEvaluation::product(
        first.enclosure(),
        reciprocal.enclosure(),
        arithmetic,
        &mut scalar,
    )?;
    let records = [
        first.export(&mut scalar)?,
        second.export(&mut scalar)?,
        reciprocal.export(&mut scalar)?,
        product.export(&mut scalar)?,
    ];
    Ok((records, scalar.usage()))
}

/// The total already contains its numerical companion, so use it exactly once.
fn input_box<P: Scalar>(
    coordinates: [P; 3],
    arithmetic: &ArithmeticContext,
    scalar: &mut KernelBudget,
) -> Result<KernelEvaluation<P>, LiveDecisionError> {
    let [center, total, _] = coordinates;
    let radius = Finite::new(total)?;
    Ok(KernelEvaluation::sum(
        Enclosure::singleton(Finite::new(center)?),
        Enclosure::new(radius.negated().get(), total)?,
        arithmetic,
        scalar,
    )?)
}

/// Only integer coefficient and exponent enter the independent exact checker.
fn exact(value: Dyadic, budget: &mut Budget) -> Result<Rational, LiveDecisionError> {
    let coefficient = Rational::from_ratio(value.significand, 1, budget)?;
    let power = Rational::from_ratio(2, 1, budget)?.pow(i64::from(value.exponent), budget)?;
    Ok(coefficient.mul(&power, budget)?)
}

/// Convert an executed output with an independently supplied exact law.
fn record(
    value: &KernelCertificate,
    law: ScalarKernelLaw,
    budget: &mut Budget,
) -> Result<ScalarKernelRecord, LiveDecisionError> {
    Ok(ScalarKernelRecord {
        law,
        lower: exact(value.output.lower, budget)?,
        upper: exact(value.output.upper, budget)?,
    })
}

/// Preserve full input-set coverage and all ordered reciprocal/product operand references.
pub(super) fn convert<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    order: [usize; 2],
    records: &[KernelCertificate; 4],
    pi: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Vec<ScalarKernelRecord>, LiveDecisionError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(4)
        .map_err(|_| LiveDecisionError::Recipe)?;
    for (slot, value) in order.into_iter().zip(&records[..2]) {
        let KernelLaw::Sum { left, right } = value.law else {
            return Err(LiveDecisionError::Recipe);
        };
        let source = source(frozen, slot, pi, budget)?;
        let input = ScalarKernelInput {
            center: exact(left.lower, budget)?,
            radius: exact(right.upper, budget)?,
            value: source,
        };
        result.push(record(
            value,
            ScalarKernelLaw::Input(Box::new(input)),
            budget,
        )?);
    }
    result.push(record(
        &records[2],
        ScalarKernelLaw::Reciprocal { input: 1 },
        budget,
    )?);
    result.push(record(
        &records[3],
        ScalarKernelLaw::Product { left: 0, right: 2 },
        budget,
    )?);
    Ok(result)
}

/// Independently use the original checked coherent constant, never a frozen center or name alone.
fn source<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    slot: usize,
    pi: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<ScalarKernelInputValue, LiveDecisionError> {
    let association = frozen.association();
    let operand = &association.operands()[slot];
    let declarations = association.case().source().owner().declarations();
    let Some(Declaration::Constant {
        descriptor, value, ..
    }) = declarations.get(operand.name())
    else {
        return Err(LiveDecisionError::Recipe);
    };
    let difference = descriptor.role() == Role::Difference;
    let zero_origin = descriptor.measure().offset().is_zero();
    if !(difference && zero_origin) {
        return Err(LiveDecisionError::Recipe);
    }
    let coherent = descriptor
        .measure()
        .scale()
        .mul(&value.clone().into(), budget)?;
    if let Some(value) = coherent.as_rational() {
        return Ok(ScalarKernelInputValue::Rational(value.clone()));
    }
    let circle = ExactConstant::named(Constant::Pi, budget)?;
    let quotient = coherent.div(&circle, budget)?;
    let factor = quotient.as_rational().ok_or(LiveDecisionError::Recipe)?;
    Ok(ScalarKernelInputValue::PiMultiple {
        factor: factor.clone(),
        certificate: Box::new(pi.clone()),
    })
}

/// Conservative Machin payload ownership, prepaid before cloning any untrusted rational.
pub(super) fn pi_bytes(pi: &PiBoundCertificate) -> u64 {
    [&pi.lower, &pi.upper, &pi.sums[0], &pi.sums[1]]
        .into_iter()
        .fold(128_u64, |total, value| {
            let (first, second) = value.component_bits();
            total.saturating_add(
                first
                    .div_ceil(8)
                    .saturating_add(second.div_ceil(8))
                    .saturating_mul(4)
                    .saturating_add(128),
            )
        })
}
