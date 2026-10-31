// SPDX-License-Identifier: AGPL-3.0-only
//! Literal child and parent ceilings preserve every caller prefix on refusal.

use super::{TestResult, fixture, ledger, pi};
use nrz_bindings::{Budget, Limits, LimitsOverride, readings::algebra::Location};
use nrz_matrix::PiCoordinateContract;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Both formats pin cold graph owners and independent live consumer schedules.
/// ´test:integration:pi-live-coordinate-owner-boundaries´
#[test]
fn pi_live_coordinate_owner_boundaries() -> TestResult {
    let plan = fixture::plan()?;
    boundaries::<f32>(
        &plan,
        [209_249, 31_021, 5401],
        [96_223, 79_544],
        [116_430, 368],
        [80_216, 80_184, 80_680],
        [80_832, 80_824, 81_296],
    )?;
    boundaries::<f64>(
        &plan,
        [209_249, 31_021, 5401],
        [96_223, 79_544],
        [116_590, 496],
        [80_520, 80_368, 81_144],
        [81_296, 81_192, 81_920],
    )
}

/// The affine origin changes exact owner payload while numerical arrays keep their original schedule.
/// ´test:integration:pi-live-coordinate-point-owner-boundaries´
#[test]
fn pi_live_coordinate_point_owner_boundaries() -> TestResult {
    let plan = super::point::point_plan()?;
    boundaries::<f32>(
        &plan,
        [298_608, 33_854, 5979],
        [100_220, 81_969],
        [122_266, 368],
        [82_641, 82_609, 83_105],
        [83_257, 83_249, 83_721],
    )?;
    boundaries::<f64>(
        &plan,
        [298_608, 33_854, 5979],
        [100_220, 81_969],
        [122_426, 496],
        [82_945, 82_793, 83_569],
        [83_721, 83_617, 84_345],
    )
}

/// Authentication and exact coefficients are cold owners; subsequent consumers share the same arrays.
fn boundaries<P: Scalar + 'static>(
    plan: &super::state_fixture::Plan,
    exact: [u64; 3],
    evidence: [u64; 2],
    certificate: [u64; 2],
    normalization: [u64; 3],
    restoration: [u64; 3],
) -> TestResult {
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    boundary(
        |policy, budget| {
            let contract = PiCoordinateContract::new(
                &plan.graph,
                &plan.contract,
                &circle,
                policy,
                &context,
                budget,
            )?;
            assert_eq!(contract.owned_bytes(), certificate[1]);
            Ok(())
        },
        exact,
        [exact[0], certificate[0], certificate[1]],
    )?;
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &policy(exact),
        &context,
        &mut ledger()?,
    )?;
    let (heap, source) = fixture::imported::<P>(plan)?;
    let normalize = [3248, normalization[0], normalization[1]];
    boundary(
        |policy, budget| {
            heap.normalize_certified_reading(&source, &contract, policy, &context, budget)?;
            Ok(())
        },
        normalize,
        [3248, normalization[2], normalization[1]],
    )?;
    let normalized = heap.normalize_certified_reading(
        &source,
        &contract,
        &policy(normalize),
        &context,
        &mut ledger()?,
    )?;
    boundary(
        |policy, budget| {
            heap.restore_certified_reading(&normalized, &contract, policy, &context, budget)?;
            Ok(())
        },
        [177, restoration[0], restoration[1]],
        [177, restoration[2], restoration[1]],
    )?;
    assert_eq!(
        plan.contract.normalization().retained_payload_bytes()
            + plan.contract.restoration().retained_payload_bytes(),
        evidence[1]
    );
    let mut authentication = ledger()?;
    for image in [plan.contract.normalization(), plan.contract.restoration()] {
        plan.graph
            .validate_reuse_key(image.reuse_key(), &mut authentication)?;
    }
    assert_eq!(authentication.usage().work, evidence[0]);
    Ok(())
}

/// Every resource has exactly one successful literal boundary and one failure immediately below it.
fn boundary(
    mut attempt: impl FnMut(&LimitsOverride, &mut Budget) -> TestResult,
    child: [u64; 3],
    parent: [u64; 3],
) -> TestResult {
    let mut cumulative = parent;
    cumulative[0] += 17;
    let mut budget = prior(cumulative)?;
    attempt(&policy(child), &mut budget)?;
    assert_eq!(budget.usage().work, cumulative[0]);
    for owner in 0..2 {
        for resource in 0..3 {
            let mut child_limit = child;
            let mut parent_limit = cumulative;
            if owner == 0 {
                child_limit[resource] -= 1;
            } else {
                parent_limit[resource] -= 1;
            }
            let mut budget = prior(parent_limit)?;
            let before = budget.usage();
            assert!(
                attempt(&policy(child_limit), &mut budget).is_err(),
                "owner {owner}, resource {resource}"
            );
            assert_eq!(budget.usage(), before);
        }
    }
    Ok(())
}

/// Resolve literal explicit allowances without changing any production default.
fn policy(profile: [u64; 3]) -> LimitsOverride {
    LimitsOverride {
        work: Some(profile[0]),
        staged_payload_bytes: Some(profile[1]),
        output_bytes: Some(profile[2]),
        ..LimitsOverride::default()
    }
}

/// A nonzero caller prefix exposes accidental commits on every failed child attempt.
fn prior(profile: [u64; 3]) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits::default().resolve(&policy(profile))?)?;
    budget.work(17, Location::new("prior live coordinate caller"))?;
    Ok(budget)
}

/// Repeated consumers pay only their live schedule while both coefficient arrays stay shared.
/// ´test:integration:pi-live-coordinate-cumulative-reuse-boundary´
#[test]
fn pi_live_coordinate_cumulative_reuse_boundary() -> TestResult {
    cumulative::<f32>()?;
    cumulative::<f64>()
}

/// One cold admission, two normalizations and one restoration retain every earlier Work charge.
fn cumulative<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    for limit in [215_939, 215_938] {
        let mut budget = prior([limit, 116_590, 81_192])?;
        let contract = PiCoordinateContract::new(
            &plan.graph,
            &plan.contract,
            &circle,
            &policy([209_249, 31_021, 5401]),
            &context,
            &mut budget,
        )?;
        let (heap, source) = fixture::imported::<P>(&plan)?;
        let normalized = heap.normalize_certified_reading(
            &source,
            &contract,
            &fixture::execution(),
            &context,
            &mut budget,
        )?;
        let warm = heap.normalize_certified_reading(
            &source,
            &contract,
            &fixture::execution(),
            &context,
            &mut budget,
        )?;
        assert!(std::ptr::eq(
            normalized.coefficients().as_ptr(),
            warm.coefficients().as_ptr()
        ));
        assert_eq!(budget.usage().work, 215_762);
        let before = budget.usage();
        let result = heap.restore_certified_reading(
            &normalized,
            &contract,
            &fixture::execution(),
            &context,
            &mut budget,
        );
        if limit == 215_939 {
            result?;
            assert_eq!(budget.usage().work, limit);
        } else {
            assert!(result.is_err());
            assert_eq!(budget.usage(), before);
        }
    }
    Ok(())
}
