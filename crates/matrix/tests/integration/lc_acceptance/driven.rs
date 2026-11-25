// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit later continuations consume fixed impedance sources beside one independent spectrum.

use crate::integration::axis_phase::fixture as axis_fixture;
mod lifecycle;
mod resources;

use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_bindings::{AxisPhaseOutcome, Budget};
use nrz_matrix::{
    AxisPhasePlan, Heap, LcDrivenContinuations, LcDrivenLimits, LcDrivenPoint,
    LcDrivenStatus as Status, LcDrivenStratum as Stratum, LcSpectralBatch, LcSpectralBatchLimits,
    LcSpectralSchema, ReadingBinding,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Original below-resonance slot, independent of its caller point key.
const FIRST_ROW: usize = 0;
/// Original resonant slot retains domain outcomes without a numerical current.
const MIDDLE_ROW: usize = 1;
/// Original above-resonance slot remains independent of earlier refusal.
const LAST_ROW: usize = 2;

/// Independent original fixed phasors share a heap with one autonomous producer.
struct Case<P: Scalar> {
    /// All source and output numerical regions.
    heap: Heap,
    /// One finite cumulative continuation attempt.
    batch: LcDrivenContinuations<P>,
    /// Shared admitted arithmetic theorem.
    context: ArithmeticContext,
    /// Original whole/copy bindings retained for equal-value mutation twins.
    sources: Vec<Vec<ReadingBinding<P>>>,
}

/// Prepare every fixed source, exact alternative and output slot before numerical execution.
fn setup<P: Scalar + 'static>(widen: bool) -> TestResult<Case<P>> {
    let context = ArithmeticContext::new()?;
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let schema = LcSpectralSchema::new(
        &fixture.graph,
        fixture.law.clone(),
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    let (mut heap, inputs) = fixture.import([
        [scalar("0.025")?, scalar("0.00000001")?, P::ZERO],
        [scalar("0.00001")?, scalar("0.00000000001")?, P::ZERO],
    ])?;
    let spectrum = schema.bind(&heap, inputs, &mut ledger()?)?;
    let pi = crate::integration::pi_conversion::pi::<P>(&context, &mut ledger()?)?;
    let mut points = Vec::new();
    let mut sources = Vec::new();
    for (key, frequency, reactance, lower, upper) in [
        (41, 1000, -75, "-76", "-74"),
        (9, 2000, 0, "0", "0"),
        (73, 4000, 75, "74", "76"),
    ] {
        let original = axis_fixture::fixture(frequency)?;
        let radius = if widen && key == 41 {
            P::from_u8(76)
        } else {
            P::ZERO
        };
        let physical = crate::phasor_solve_testing::fixed_case_in_heap(
            reactance,
            radius,
            false,
            heap,
            true,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )?;
        let phase = AxisPhasePlan::new(
            original.case.clone(),
            &pi,
            LiteralLimits::new(32, 32, 1024),
            &context,
            &mut ledger()?,
        )?;
        let bounds = if widen && key == 41 {
            ["-152", "2"]
        } else {
            [lower, upper]
        };
        let point = LcDrivenPoint::new(
            key,
            physical.publication(&mut ledger()?)?,
            phase,
            axis_fixture::enclosure(&original, bounds[0], bounds[1])?,
            &mut ledger()?,
        )?;
        sources.push(physical.bindings.clone());
        heap = physical.heap;
        points.push(point);
    }
    let invariants = LcSpectralBatch::prepare(
        &schema,
        &heap,
        &[
            (41, spectrum.clone()),
            (9, spectrum.clone()),
            (73, spectrum),
        ],
        LcSpectralBatchLimits {
            points: 3,
            bindings: 1,
            key_bytes: 4096,
        },
        &context,
        &mut ledger()?,
    )?;
    let batch = LcDrivenContinuations::prepare(
        invariants,
        points,
        &heap,
        &context,
        LcDrivenLimits {
            points: 3,
            table_bytes: 32_768,
        },
        &mut ledger()?,
    )?;
    Ok(Case {
        heap,
        batch,
        context,
        sources,
    })
}

/// Represent fixture inputs; independent rational oracles check their output enclosures.
fn scalar<P: Scalar>(text: &str) -> TestResult<P> {
    P::parse_nearest(text).ok_or_else(|| "fixture scalar".into())
}

/// All four strata consume one existing cumulative test policy.
fn execute<P: Scalar + 'static>(case: &mut Case<P>, budget: &mut Budget) -> TestResult {
    Ok(case.batch.execute(&mut case.heap, &case.context, budget)?)
}

/// One retained invariant precedes every decision, then all current and phase continuations.
/// ´test:integration:lc-driven-fixed-row-continuation-table´
#[test]
fn lc_driven_fixed_row_continuation_table() -> TestResult {
    nominal::<f32>()?;
    nominal::<f64>()
}

/// The middle domain refusal preserves both outer currents and the shared original spectrum.
fn nominal<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false)?;
    let mut budget = crate::phasor_solve_testing::resources::fresh_budget()?;
    assert!(
        case.batch
            .phase(FIRST_ROW, &case.heap, &case.context, &mut budget)
            .is_err()
    );
    execute(&mut case, &mut budget)?;
    assert_eq!(case.batch.point_keys().collect::<Vec<_>>(), [41, 9, 73]);
    assert_eq!(
        case.batch.statuses(0),
        Some([Status::Successful; 3]),
        "{:?}",
        case.batch.failure(0, Stratum::Current)
    );
    assert_eq!(
        case.batch.statuses(1),
        Some([
            Status::Successful,
            Status::DomainUnavailable,
            Status::Undefined
        ])
    );
    assert_eq!(case.batch.statuses(2), Some([Status::Successful; 3]));
    assert_eq!(case.batch.verdict(1), Some(AxisPhaseOutcome::Undefined));
    assert!(
        case.batch
            .current(1, &case.heap, &case.context, &mut budget)
            .is_err()
    );
    assert!(
        case.batch
            .phase(MIDDLE_ROW, &case.heap, &case.context, &mut budget)
            .is_err()
    );
    let root = case
        .batch
        .invariants()
        .result(0, &case.heap, &case.context, &mut budget)?;
    assert!(std::ptr::eq(
        root,
        case.batch
            .invariants()
            .result(1, &case.heap, &case.context, &mut budget)?
    ));
    let frequency = root.imaginary_rates(&case.heap, &case.context)?[0];
    assert!(
        crate::reciprocal_testing::oracle::Rational::new(2000, 1)?
            .covered(frequency.center.to_f64(), frequency.total.to_f64())?
    );
    for (row, sign) in [(0, 1), (2, -1)] {
        current(&case, row, sign, &mut budget)?;
        phase(&case, row, sign, &mut budget)?;
    }
    let mut after = None;
    for stratum in [
        Stratum::Invariant,
        Stratum::Decision,
        Stratum::Current,
        Stratum::Phase,
    ] {
        let usage = case.batch.usage(stratum).ok_or("stratum receipt")?;
        if let Some(previous) = after {
            assert_eq!(usage[0], previous);
        }
        assert!(usage[1].work > usage[0].work);
        after = Some(usage[1]);
    }
    assert!(execute(&mut case, &mut ledger()?).is_err());
    Ok(())
}

/// Check the published current against an independent rational mean and its actual total.
fn current<P: Scalar + 'static>(
    case: &Case<P>,
    row: usize,
    sign: i64,
    budget: &mut Budget,
) -> TestResult {
    let current = case.batch.current(row, &case.heap, &case.context, budget)?;
    let value = &current.readings().bindings()[0];
    let entries = case.heap.reading_entries(value)?;
    assert_eq!(entries.len(), 4);
    let radius = current.evaluation().totals(&case.heap, &case.context)?;
    assert!(
        crate::reciprocal_testing::oracle::Rational::new(sign, 25)?
            .covered(entries[1].to_f64(), radius[1].to_f64())?
    );
    for reading in current.readings().bindings() {
        case.heap.reading_entries(reading)?;
    }
    Ok(())
}

/// Check both half-pi branches against independently generated rational bounds.
fn phase<P: Scalar + 'static>(
    case: &Case<P>,
    row: usize,
    sign: i64,
    budget: &mut Budget,
) -> TestResult {
    let value = case.batch.phase(row, &case.heap, &case.context, budget)?;
    let positive = if sign == 1 { value.negated() } else { value };
    let oracle = crate::integration::pi_conversion::oracle::factor(1, 2, false, 56)?;
    let lower = crate::kernel_oracle::Rational::binary(positive.lower().get().to_f64())?;
    let upper = crate::kernel_oracle::Rational::binary(positive.upper().get().to_f64())?;
    assert!(lower.at_most(oracle[0])?);
    assert!(oracle[1].at_most(upper)?);
    Ok(())
}

/// A wider admitted original box retains its exact unresolved continuation and no default output.
/// ´test:integration:lc-driven-fixed-row-undecided-continuation´
#[test]
fn lc_driven_fixed_row_undecided_continuation() -> TestResult {
    undecided::<f32>()?;
    undecided::<f64>()
}

/// No default current or phase appears when the complete admitted box crosses zero.
fn undecided<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(true)?;
    execute(
        &mut case,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    assert_eq!(case.batch.statuses(0), Some([Status::Undecided; 3]));
    assert_eq!(
        case.batch.continuation(0),
        Some("retain-unresolved-reactance")
    );
    assert!(
        case.batch
            .current(0, &case.heap, &case.context, &mut ledger()?)
            .is_err()
    );
    assert!(
        case.batch
            .phase(FIRST_ROW, &case.heap, &case.context, &mut ledger()?)
            .is_err()
    );
    assert_eq!(case.batch.statuses(2), Some([Status::Successful; 3]));
    case.batch
        .invariants()
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    Ok(())
}
