// SPDX-License-Identifier: AGPL-3.0-only
//! Execute native scalar laws and convert their exact records without rounded endpoint arithmetic.

use super::{AT, LiveDecisionError};
use crate::FrozenDecisionInputs;
use nrz_bindings::readings::algebra::{
    ExactConstant, Rational,
    decision::{
        ScalarKernelFamilyInput, ScalarKernelInput, ScalarKernelInputValue, ScalarKernelLaw,
        ScalarKernelReading, ScalarKernelRecord,
    },
    pi_bounds::PiBoundCertificate,
    syntax::Constant,
};
use nrz_bindings::{Budget, Declaration, Role};
use nrz_scalar::{
    ArithmeticContext, Dyadic, Enclosure, Finite, KernelBudget, KernelCertificate,
    KernelEvaluation, KernelLaw, KernelUsage, LiteralLimits, Scalar,
};

/// Prepay the bounded quotient and optional affine schedules, including every fixed-size export.
pub(super) fn execute<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    order: [usize; 2],
    arithmetic: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<(Vec<KernelCertificate>, KernelUsage), LiveDecisionError> {
    let mapped = frozen.association().case().source().input_readings().len();
    let work = 732_u64.saturating_add(
        u64::try_from(mapped)
            .map_err(|_| LiveDecisionError::Recipe)?
            .saturating_mul(1064),
    );
    budget.work(work, AT)?;
    let mut scalar = KernelBudget::new(work);
    let mut records = Vec::new();
    records
        .try_reserve_exact(4_usize.saturating_add(mapped.saturating_mul(4)))
        .map_err(|_| LiveDecisionError::Recipe)?;
    let mut values = [
        input_box(
            frozen.operands()[order[0]].coordinates(),
            arithmetic,
            &mut scalar,
        )?,
        input_box(
            frozen.operands()[order[1]].coordinates(),
            arithmetic,
            &mut scalar,
        )?,
    ];
    for value in &values {
        records.push(value.export(&mut scalar)?);
    }
    for (value, slot) in values.iter_mut().zip(order) {
        if let Some(reading) = frozen.association().operands()[slot].reading() {
            *value = mapped_input(
                *value,
                reading,
                arithmetic,
                &mut scalar,
                &mut records,
                budget,
            )?;
        }
    }
    let reciprocal = KernelEvaluation::reciprocal(values[1].enclosure(), arithmetic, &mut scalar)?;
    let product = KernelEvaluation::product(
        values[0].enclosure(),
        reciprocal.enclosure(),
        arithmetic,
        &mut scalar,
    )?;
    records.push(reciprocal.export(&mut scalar)?);
    records.push(product.export(&mut scalar)?);
    Ok((records, scalar.usage()))
}

/// Execute both exact coefficient imports and the full-box product and sum in declared order.
fn mapped_input<P: Scalar>(
    input: KernelEvaluation<P>,
    reading: &nrz_bindings::readings::PhysicalReading,
    arithmetic: &ArithmeticContext,
    scalar: &mut KernelBudget,
    records: &mut Vec<KernelCertificate>,
    budget: &mut Budget,
) -> Result<KernelEvaluation<P>, LiveDecisionError> {
    let [scale] = reading.map().coefficients() else {
        return Err(LiveDecisionError::Recipe);
    };
    let [offset] = reading.map().offsets() else {
        return Err(LiveDecisionError::Recipe);
    };
    let scale = coefficient(scale, arithmetic, scalar, budget)?;
    let offset = coefficient(offset, arithmetic, scalar, budget)?;
    let product =
        KernelEvaluation::product(input.enclosure(), scale.enclosure(), arithmetic, scalar)?;
    let sum = KernelEvaluation::sum(product.enclosure(), offset.enclosure(), arithmetic, scalar)?;
    for value in [scale, offset, product, sum] {
        records.push(value.export(scalar)?);
    }
    Ok(sum)
}

/// The native import keeps its existing explicit literal allowance and refuses larger requirements.
fn coefficient<P: Scalar>(
    value: &Rational,
    arithmetic: &ArithmeticContext,
    scalar: &mut KernelBudget,
    budget: &mut Budget,
) -> Result<KernelEvaluation<P>, LiveDecisionError> {
    let (numerator, denominator) = value
        .small_ratio(budget)?
        .ok_or(LiveDecisionError::Recipe)?;
    Ok(KernelEvaluation::rational(
        numerator,
        denominator,
        LiteralLimits::new(32, 32, 128),
        arithmetic,
        scalar,
    )?)
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
    records: &[KernelCertificate],
    pi: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Vec<ScalarKernelRecord>, LiveDecisionError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(
            4_usize.saturating_add(frozen.association().case().source().input_readings().len()),
        )
        .map_err(|_| LiveDecisionError::Recipe)?;
    for (slot, value) in order.into_iter().zip(&records[..2]) {
        let KernelLaw::Sum { left, right } = value.law else {
            return Err(LiveDecisionError::Recipe);
        };
        if let Some(family) = frozen.association().operands()[slot].family() {
            result.push(record(
                value,
                ScalarKernelLaw::Family(Box::new(ScalarKernelFamilyInput {
                    center: exact(left.lower, budget)?,
                    radius: exact(right.upper, budget)?,
                    family: family.clone(),
                })),
                budget,
            )?);
            continue;
        }
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
    let (logical, cursor) = mapped_records(frozen, order, records, &mut result, budget)?;
    if records.len() != cursor + 2 {
        return Err(LiveDecisionError::Recipe);
    }
    let reciprocal = u32::try_from(result.len()).map_err(|_| LiveDecisionError::Recipe)?;
    result.push(record(
        &records[cursor],
        ScalarKernelLaw::Reciprocal { input: logical[1] },
        budget,
    )?);
    result.push(record(
        &records[cursor + 1],
        ScalarKernelLaw::Product {
            left: logical[0],
            right: reciprocal,
        },
        budget,
    )?);
    Ok(result)
}

/// One exact map law covers the final actual sum; all four executed primitives remain in the trace.
fn mapped_records<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    order: [usize; 2],
    records: &[KernelCertificate],
    result: &mut Vec<ScalarKernelRecord>,
    budget: &mut Budget,
) -> Result<([u32; 2], usize), LiveDecisionError> {
    let mut logical = [0, 1];
    let mut cursor = 2;
    for (position, slot) in order.into_iter().enumerate() {
        if let Some(reading) = frozen.association().operands()[slot].reading() {
            let [scale] = reading.map().coefficients() else {
                return Err(LiveDecisionError::Recipe);
            };
            let [offset] = reading.map().offsets() else {
                return Err(LiveDecisionError::Recipe);
            };
            let value = records.get(cursor + 3).ok_or(LiveDecisionError::Recipe)?;
            let index = u32::try_from(result.len()).map_err(|_| LiveDecisionError::Recipe)?;
            result.push(record(
                value,
                ScalarKernelLaw::Reading(Box::new(ScalarKernelReading {
                    definition: reading.node().clone(),
                    input: logical[position],
                    scale: scale.clone(),
                    offset: offset.clone(),
                })),
                budget,
            )?);
            logical[position] = index;
            cursor += 4;
        }
    }
    Ok((logical, cursor))
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

/// Complete family boxes and qualified source handles are reserved before cloning their laws.
pub(super) fn family_bytes<P: Scalar>(frozen: &FrozenDecisionInputs<P>) -> u64 {
    frozen
        .association()
        .operands()
        .iter()
        .filter_map(|operand| operand.family())
        .fold(0_u64, |total, family| {
            [&family.lower, &family.upper].into_iter().fold(
                total.saturating_add(192),
                |total, value| {
                    let (first, second) = value.component_bits();
                    total.saturating_add(
                        first
                            .div_ceil(8)
                            .saturating_add(second.div_ceil(8))
                            .saturating_mul(4)
                            .saturating_add(128),
                    )
                },
            )
        })
}

/// Extra actual scalar records and exact map fields are prepaid before execution and cloning.
pub(super) fn reading_bytes<P: Scalar>(frozen: &FrozenDecisionInputs<P>) -> u64 {
    frozen
        .association()
        .case()
        .source()
        .input_readings()
        .iter()
        .fold(0_u64, |total, (_, reading)| {
            reading
                .map()
                .coefficients()
                .iter()
                .chain(reading.map().offsets())
                .fold(total.saturating_add(960), |total, value| {
                    let (first, second) = value.component_bits();
                    total.saturating_add(
                        first
                            .div_ceil(8)
                            .saturating_add(second.div_ceil(8))
                            .saturating_mul(4)
                            .saturating_add(128),
                    )
                })
        })
}
