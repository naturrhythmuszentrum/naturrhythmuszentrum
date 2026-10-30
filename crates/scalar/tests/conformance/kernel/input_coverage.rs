// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`kernel_family_schedule_baseline`] | scalar | The complete native LC arithmetic schedule still exports four conditional records for a family. |
//! | [`kernel_affine_schedule_baseline`] | scalar | Two exact coefficients and their product and sum add four executed records before the quotient. |
//!

//! Physical coverage adds premises; affine coverage additionally executes the original map.

use super::{TestResult, point, replay};
use nrz_scalar::{
    ArithmeticContext, Enclosure, KernelBudget, KernelCertificate, KernelError, KernelEvaluation,
    LiteralLimits,
};

/// Execute the LC input boxes, optional exact affine map, reciprocal and product in order.
fn quotient(mapped: bool, budget: &mut KernelBudget) -> Result<KernelCertificate, KernelError> {
    let context = ArithmeticContext::new()?;
    let radius = 1.0_f64 / 262_144.0;
    let numerator = KernelEvaluation::sum(
        point(1311.0 / 131_072.0)?,
        Enclosure::new(-radius, radius)?,
        &context,
        budget,
    )?;
    let denominator = KernelEvaluation::sum(
        point(103.0 / 32_768.0)?,
        Enclosure::new(-radius, radius)?,
        &context,
        budget,
    )?;
    assert!(replay(numerator.export(budget)?));
    assert!(replay(denominator.export(budget)?));
    let numerator = if mapped {
        let scale =
            KernelEvaluation::rational(2, 1, LiteralLimits::new(32, 32, 128), &context, budget)?;
        let offset =
            KernelEvaluation::rational(-1, 100, LiteralLimits::new(32, 32, 128), &context, budget)?;
        let scaled =
            KernelEvaluation::product(numerator.enclosure(), scale.enclosure(), &context, budget)?;
        let shifted =
            KernelEvaluation::sum(scaled.enclosure(), offset.enclosure(), &context, budget)?;
        for value in [scale, offset, scaled, shifted] {
            assert!(replay(value.export(budget)?));
        }
        shifted
    } else {
        numerator
    };
    let reciprocal = KernelEvaluation::reciprocal(denominator.enclosure(), &context, budget)?;
    let product = KernelEvaluation::product(
        numerator.enclosure(),
        reciprocal.enclosure(),
        &context,
        budget,
    )?;
    assert!(replay(reciprocal.export(budget)?));
    product.export(budget)
}

/// The complete native LC arithmetic schedule still exports four conditional records for a family.
/// ´claim:scalar:kernel-family-schedule-baseline´
/// ´test:integration:kernel-family-schedule-baseline´
#[test]
fn kernel_family_schedule_baseline() -> TestResult {
    boundary(false, [749, 748], 4, 621)
}

/// Two exact coefficients and their product and sum add four executed records before the quotient.
/// ´claim:scalar:kernel-affine-schedule-baseline´
/// ´test:integration:kernel-affine-schedule-baseline´
#[test]
fn kernel_affine_schedule_baseline() -> TestResult {
    boundary(true, [1813, 1812], 8, 1685)
}

/// Each fresh ledger preserves spent Work when the final complete record cannot be exported.
fn boundary(mapped: bool, boundaries: [u64; 2], records: u64, spent: u64) -> TestResult {
    for work in boundaries {
        let mut budget = KernelBudget::new(work);
        budget.charge(17)?;
        let result = quotient(mapped, &mut budget);
        if work == boundaries[0] {
            assert!(replay(result?));
            assert_eq!(budget.usage().exported, records);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("one-below complete trace must refuse its final export");
            };
            assert_eq!(fault, KernelError::WorkLimit);
            assert_eq!(budget.usage().exported, records - 1);
            assert_eq!(budget.usage().work, spent);
        }
        assert_eq!(budget.usage().constructed, records);
        assert_eq!(budget.limit(), work);
    }
    Ok(())
}
