// SPDX-License-Identifier: AGPL-3.0-only
//! Original shared-voltage moments are a declared later consumer of the finite driven table.

use super::{TestResult, fixture, ledger};
use crate::integration::axis_phase::fixture as axis;
use crate::reciprocal_testing::oracle::Rational;
use nrz_bindings::{Budget, readings::algebra::Location};
use nrz_matrix::{
    AxisPhasePlan, Heap, LcDrivenContinuations, LcDrivenLimits, LcDrivenPoint, LcDrivenStatistics,
    LcDrivenStatus as Status, LcSpectralBatch, LcSpectralBatchLimits, LcSpectralSchema,
    ReadingBinding,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// One original shared voltage and finite covariance family surround two fixed driven rows.
struct Case<P: Scalar> {
    /// Actual source, current and parameter versions.
    heap: Heap,
    /// Complete predeclared driven and later statistical owner.
    plan: LcDrivenStatistics<P>,
    /// Original voltage, variance and variance-error source readings.
    sources: Vec<ReadingBinding<P>>,
    /// Shared native arithmetic revision.
    context: ArithmeticContext,
}

/// Every equation, covariance law, current and selected branch exists before execution.
fn setup<P: Scalar + 'static>(wide: bool, wrong: bool) -> TestResult<Case<P>> {
    let mut shared = fixture::axis_case::<P>()?;
    let physical = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let l = P::parse_nearest("0.025").ok_or("L fixture")?;
    let c = P::parse_nearest("0.00001").ok_or("C fixture")?;
    let le = P::parse_nearest("0.00000001").ok_or("L radius")?;
    let ce = P::parse_nearest("0.00000000001").ok_or("C radius")?;
    let inputs = physical.import_into(
        &mut shared.heap,
        [[l, le, P::ZERO], [c, ce, P::ZERO]],
        &mut ledger()?,
    )?;
    let schema = LcSpectralSchema::new(
        &physical.graph,
        physical.law.clone(),
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    let spectrum = schema.bind(&shared.heap, inputs, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let invariants = LcSpectralBatch::prepare(
        &schema,
        &shared.heap,
        &[(41, spectrum.clone()), (73, spectrum)],
        LcSpectralBatchLimits {
            points: 2,
            bindings: 1,
            key_bytes: 4096,
        },
        &context,
        &mut ledger()?,
    )?;
    let pi = crate::integration::pi_conversion::pi::<P>(&context, &mut ledger()?)?;
    let mut points = Vec::new();
    for (index, (key, frequency, lower, upper)) in
        [(41, 1000, "-76", "-74"), (73, 4000, "74", "76")]
            .into_iter()
            .enumerate()
    {
        let original = axis::fixture(frequency)?;
        let bounds = if wide && index == 0 {
            ["-152", "2"]
        } else {
            [lower, upper]
        };
        points.push(LcDrivenPoint::new(
            key,
            shared.publications[index].clone(),
            AxisPhasePlan::new(
                original.case.clone(),
                &pi,
                LiteralLimits::new(32, 32, 1024),
                &context,
                &mut ledger()?,
            )?,
            axis::enclosure(&original, bounds[0], bounds[1])?,
            &mut ledger()?,
        )?);
    }
    let driven = LcDrivenContinuations::prepare(
        invariants,
        points,
        &shared.heap,
        &context,
        LcDrivenLimits {
            points: 2,
            table_bytes: 32_768,
        },
        &mut ledger()?,
    )?;
    let mut exact = shared.exact;
    if wrong {
        exact.swap(0, 1);
    }
    let plan = LcDrivenStatistics::prepare(
        driven,
        exact,
        [shared.bindings[15].clone(), shared.bindings[16].clone()],
        [0, 1],
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    Ok(Case {
        heap: shared.heap,
        plan,
        sources: [10, 15, 16]
            .map(|index| shared.bindings[index].clone())
            .to_vec(),
        context,
    })
}

/// The exact `-U_V/5625` cross block follows both actual current producers in one bounded attempt.
/// ´test:integration:lc-driven-plan-shared-voltage-statistics´
#[test]
fn lc_driven_plan_shared_voltage_statistics() -> TestResult {
    nominal::<f32>()?;
    nominal::<f64>()
}

/// Independent scalar rational oracles retain the signed cross block and shared source identity.
fn nominal<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false, false)?;
    let mut budget = super::fresh_budget()?;
    case.plan
        .execute(&mut case.heap, &case.context, &mut budget)?;
    assert_eq!(
        case.plan.status(),
        Status::Successful,
        "{:?}",
        reasons(&case)
    );
    assert_eq!(
        case.plan.usage().ok_or("complete attempt")?[1],
        budget.usage()
    );
    assert_eq!(
        case.plan.driven().point_keys().collect::<Vec<_>>(),
        [41, 73]
    );
    let result = case
        .plan
        .statistics(&case.heap, &case.context, &mut budget)?;
    let values = result.moments(&case.heap, &case.context)?;
    let errors = result.parameter_errors(&case.heap, &case.context)?;
    for ((value, error), numerator) in values.into_iter().zip(errors).zip([1, -1, 1]) {
        assert!(Rational::new(numerator, 22_500)?.covered(value.to_f64(), error.to_f64())?);
    }
    assert!(values[1] < P::ZERO);
    let receipt = case
        .plan
        .driven()
        .usage(nrz_matrix::LcDrivenStratum::Phase)
        .ok_or("last driven receipt")?;
    assert!(result.usage()[0].work > receipt[1].work);
    assert!(
        case.plan
            .execute(&mut case.heap, &case.context, &mut super::fresh_budget()?)
            .is_err()
    );
    Ok(())
}

/// A named unresolved current blocks the entire covariance family without a default marginal.
/// ´test:integration:lc-driven-plan-statistics-unavailable-and-refusals´
#[test]
fn lc_driven_plan_statistics_unavailable_and_refusals() -> TestResult {
    unavailable::<f32>()?;
    unavailable::<f64>()
}

/// All declarations are checked before execution, and an admitted wide certificate has no fallback.
fn unavailable<P: Scalar + 'static>() -> TestResult {
    assert!(setup::<P>(false, true).is_err());
    let mut case = setup::<P>(true, false)?;
    case.plan
        .execute(&mut case.heap, &case.context, &mut super::fresh_budget()?)?;
    assert_eq!(case.plan.status(), Status::Blocked, "{:?}", reasons(&case));
    assert_eq!(
        case.plan.driven().continuation(0),
        Some("retain-unresolved-reactance"),
        "{:?}",
        reasons(&case)
    );
    assert!(
        case.plan
            .statistics(&case.heap, &case.context, &mut ledger()?)
            .is_err()
    );
    case.plan
        .driven()
        .current(1, &case.heap, &case.context, &mut ledger()?)?;
    case.plan
        .driven()
        .invariants()
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    Ok(())
}

/// Actual shared voltage, source parameter and produced mean revisions retire the complete family.
/// ´test:integration:lc-driven-plan-statistics-source-and-target-lifetime´
#[test]
fn lc_driven_plan_statistics_source_and_target_lifetime() -> TestResult {
    lifetime::<f32>()?;
    lifetime::<f64>()
}

/// Equal-value replacements cannot revive historical successful statistics or their selected branches.
fn lifetime<P: Scalar + 'static>() -> TestResult {
    for index in 0..4 {
        let mut case = setup::<P>(false, false)?;
        case.plan
            .execute(&mut case.heap, &case.context, &mut super::fresh_budget()?)?;
        let target = if index < 3 {
            case.sources[index].clone()
        } else {
            case.plan
                .driven()
                .current(0, &case.heap, &case.context, &mut ledger()?)?
                .readings()
                .bindings()[0]
                .clone()
        };
        let values = case.heap.reading_entries(&target)?;
        case.heap
            .replace_reading_entries(&target, &values, &mut ledger()?)?;
        assert_eq!(case.plan.status(), Status::Successful);
        assert!(
            case.plan
                .statistics(&case.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        case.plan
            .driven()
            .invariants()
            .result(0, &case.heap, &case.context, &mut ledger()?)?;
    }
    Ok(())
}

/// Literal scoped execution twins preserve deterministic current prefixes after a late statistical refusal.
/// ´test:integration:lc-driven-plan-statistics-execution-boundaries´
/// The scoped execution gains 360 receiving-store Work over unchanged operation Work: 124
/// retained-kind counts, 112 identity insertions, 12 retained-kind insertions,
/// and 112 vertex admissions. Lookup floors cancel their old graph counterparts.
#[test]
fn lc_driven_plan_statistics_execution_boundaries() -> TestResult {
    let work = [measure::<f32>()?, measure::<f64>()?];
    assert_eq!(work, [[1_913_478, 4_136], [1_913_574, 4_144]]);
    boundaries::<f32>(work[0][0])?;
    boundaries::<f64>(work[1][0])
}

/// Declaration costs remain outside this scoped execution pin; fixed wrapper bytes exclude child owners.
fn measure<P: Scalar + 'static>() -> TestResult<[u64; 2]> {
    let mut case = setup::<P>(false, false)?;
    let mut budget = super::fresh_budget()?;
    case.plan
        .execute(&mut case.heap, &case.context, &mut budget)?;
    assert_eq!(
        case.plan.status(),
        Status::Successful,
        "{:?}",
        reasons(&case)
    );
    Ok([
        budget.usage().work,
        u64::try_from(case.plan.wrapper_bytes())?,
    ])
}

/// A consumed prefix spends the same finite allowance; no retry or statistical prefix can escape.
fn boundaries<P: Scalar + 'static>(work: u64) -> TestResult {
    for (limit, prefix, success) in [(work, 0, true), (work - 1, 0, false), (work, 1, false)] {
        let mut case = setup::<P>(false, false)?;
        let mut policy = super::policy();
        policy.work = limit;
        let mut budget = Budget::new(policy)?;
        budget.work(prefix, Location::new("earlier driven statistics work"))?;
        case.plan
            .execute(&mut case.heap, &case.context, &mut budget)?;
        assert_eq!(case.plan.status() == Status::Successful, success);
        assert_eq!(
            case.plan
                .statistics(&case.heap, &case.context, &mut ledger()?)
                .is_ok(),
            success
        );
        for row in 0..2 {
            case.plan
                .driven()
                .current(row, &case.heap, &case.context, &mut ledger()?)?;
        }
        assert!(
            case.plan
                .execute(&mut case.heap, &case.context, &mut super::fresh_budget()?)
                .is_err()
        );
    }
    Ok(())
}

/// Retain concise typed child diagnostics when unavailable means block the statistical family.
fn reasons<P: Scalar + 'static>(case: &Case<P>) -> [Option<&nrz_matrix::LcDrivenError>; 3] {
    [
        case.plan.failure(),
        case.plan
            .driven()
            .failure(0, nrz_matrix::LcDrivenStratum::Decision),
        case.plan
            .driven()
            .failure(0, nrz_matrix::LcDrivenStratum::Current),
    ]
}
