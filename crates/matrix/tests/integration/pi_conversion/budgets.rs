// SPDX-License-Identifier: AGPL-3.0-only
//! Literal owner limits distinguish successful publication from atomic failed attempts.

use super::{exact_policy, owner, pi};
use crate::conversion_testing::{TestResult, shape};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_matrix::{ConversionContract, ConversionMode};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Pi owner Work and logical storage succeed exactly at the limit and refuse atomically below it.
/// ´test:integration:pi-conversion-constant-resource-boundaries´
#[test]
fn pi_conversion_constant_resource_boundaries() -> TestResult {
    constant::<f32>(669, 336, 96)?;
    constant::<f64>(2055, 384, 128)
}

/// Exercise every independent owner resource with prior caller Work already committed.
fn constant<P: Scalar + 'static>(work: u64, staged: u64, output: u64) -> TestResult {
    let context = ArithmeticContext::new()?;
    let exact = Limits {
        work: work + 17,
        staged_payload_bytes: staged,
        output_bytes: output,
        ..Limits::default()
    };
    let mut complete = ledger(exact.clone())?;
    let circle = pi::<P>(&context, &mut complete)?;
    assert_eq!(complete.usage().work, work + 17);
    assert_eq!(u64::try_from(circle.owned_bytes())?, output);
    assert!(circle.same_region(&circle.clone()));
    for field in 0..3 {
        let mut limits = exact.clone();
        match field {
            0 => limits.work -= 1,
            1 => limits.staged_payload_bytes -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut limited = ledger(limits)?;
        let before = limited.usage();
        assert!(pi::<P>(&context, &mut limited).is_err());
        assert_eq!(limited.usage(), before);
    }
    Ok(())
}

/// Cold coefficient admission pins Work and bytes, and failed requests never populate its cache.
/// ´test:integration:pi-conversion-coefficient-resource-boundaries´
#[test]
fn pi_conversion_coefficient_resource_boundaries() -> TestResult {
    coefficients::<f32>(82_168, 152)?;
    coefficients::<f64>(82_232, 184)
}

/// After each failed attempt the first valid retry is still a cold admission.
fn coefficients<P: Scalar + 'static>(staged: u64, output: u64) -> TestResult {
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let exact = Limits {
        work: 84_589,
        staged_payload_bytes: staged,
        output_bytes: output,
        ..Limits::default()
    };
    for field in 0..3 {
        let conversion = contract()?;
        let mut limits = exact.clone();
        match field {
            0 => limits.work -= 1,
            1 => limits.staged_payload_bytes -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut limited = ledger(limits)?;
        let before = limited.usage();
        assert!(
            conversion
                .certify_pi(&circle, &exact_policy(), &context, &mut limited)
                .is_err()
        );
        assert_eq!(limited.usage(), before);
        let mut complete = ledger(exact.clone())?;
        let usage = conversion.certify_pi(&circle, &exact_policy(), &context, &mut complete)?;
        assert_eq!(usage.work(), 84_572);
        assert_eq!(complete.usage().work, 84_589);
        assert!(!usage.reused());
    }
    Ok(())
}

/// A cloned compatible contract reuses the same numerical allocation at literal warm boundaries.
/// ´test:integration:pi-conversion-warm-cache-resource-boundaries´
#[test]
fn pi_conversion_warm_cache_resource_boundaries() -> TestResult {
    warm::<f32>()?;
    warm::<f64>()
}

/// Cache hits have no new owner allocation and require only lookup Work and one receipt.
fn warm<P: Scalar + 'static>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let original = contract()?;
    original.certify_pi(
        &circle,
        &exact_policy(),
        &context,
        &mut Budget::new(Limits::default())?,
    )?;
    let cloned = original.clone();
    let exact = Limits {
        work: 19,
        staged_payload_bytes: 24,
        output_bytes: 24,
        ..Limits::default()
    };
    for field in 0..3 {
        let mut limits = exact.clone();
        match field {
            0 => limits.work -= 1,
            1 => limits.staged_payload_bytes -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut limited = ledger(limits)?;
        let before = limited.usage();
        assert!(
            cloned
                .certify_pi(&circle, &exact_policy(), &context, &mut limited)
                .is_err()
        );
        assert_eq!(limited.usage(), before);
    }
    let mut complete = ledger(exact)?;
    let hit = cloned.certify_pi(&circle, &exact_policy(), &context, &mut complete)?;
    assert_eq!(hit.work(), 2);
    assert_eq!(hit.new_owned_bytes(), 0);
    assert!(hit.reused());
    assert_eq!(complete.usage().work, 19);
    assert_eq!(original.map(), cloned.map());
    Ok(())
}

/// A fresh inverse-degree contract leaves the exact map owner independent of numerical admission.
fn contract() -> TestResult<ConversionContract> {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    Ok(ConversionContract::new(
        shape(&declarations, &radian, 1)?,
        shape(&declarations, degree, 1)?,
        declarations.conversion_map(&radian, degree)?,
        LiteralLimits::default(),
        ConversionMode::Deterministic,
    )?)
}

/// Every atomic test begins with an existing successful caller charge.
fn ledger(limits: Limits) -> TestResult<Budget> {
    let mut result = Budget::new(limits)?;
    result.work(17, Location::new("prior caller work"))?;
    Ok(result)
}

/// Format and original-region identities separate cache entries while cloned consumers reuse them.
/// ´test:integration:pi-conversion-cache-keys-retain-original-regions´
#[test]
fn pi_conversion_cache_keys_retain_original_regions() -> TestResult {
    let context = ArithmeticContext::new()?;
    let single = pi::<f32>(&context, &mut Budget::new(Limits::default())?)?;
    let double = pi::<f64>(&context, &mut Budget::new(Limits::default())?)?;
    let conversion = contract()?;
    let mut complete = Budget::new(Limits::default())?;
    let first = conversion.certify_pi(&single, &exact_policy(), &context, &mut complete)?;
    let second = conversion.certify_pi(&double, &exact_policy(), &context, &mut complete)?;
    assert_eq!(first.work(), 84_572);
    assert_eq!(second.work(), 84_573);
    assert!(!second.reused());
    let hit = conversion.certify_pi(&single, &exact_policy(), &context, &mut complete)?;
    assert_eq!(hit.work(), 3);
    assert_eq!(hit.new_owned_bytes(), 0);
    assert!(hit.reused());
    let distinct = pi::<f32>(&context, &mut Budget::new(Limits::default())?)?;
    assert!(!single.same_region(&distinct));
    let replacement = conversion.certify_pi(&distinct, &exact_policy(), &context, &mut complete)?;
    assert!(!replacement.reused());
    assert_eq!(replacement.work(), 84_574);
    assert_eq!(replacement.new_owned_bytes(), 112);
    Ok(())
}

/// One cumulative caller ledger covers pi creation, coefficient admission and compatible reuse.
/// ´test:integration:pi-conversion-cumulative-owner-boundary´
#[test]
fn pi_conversion_cumulative_owner_boundary() -> TestResult {
    cumulative::<f32>(85_260)?;
    cumulative::<f64>(86_646)
}

/// The final cache hit succeeds at the complete limit and refuses atomically one Work below it.
fn cumulative<P: Scalar + 'static>(work: u64) -> TestResult {
    let context = ArithmeticContext::new()?;
    for missing in [false, true] {
        let conversion = contract()?;
        let mut complete = ledger(Limits {
            work: work - u64::from(missing),
            ..Limits::default()
        })?;
        let circle = pi::<P>(&context, &mut complete)?;
        conversion.certify_pi(&circle, &exact_policy(), &context, &mut complete)?;
        let before = complete.usage();
        let result = conversion.certify_pi(&circle, &exact_policy(), &context, &mut complete);
        if missing {
            assert!(result.is_err());
            assert_eq!(complete.usage(), before);
        } else {
            assert!(result?.reused());
            assert_eq!(complete.usage().work, work);
        }
    }
    Ok(())
}
