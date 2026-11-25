// SPDX-License-Identifier: AGPL-3.0-only
//! Exact independent-time elimination is checked against original equations and ratio oracles.

use super::super::support::{TestResult, ledger};
use super::support::{cycles, input, number, original, pi, sample_times};
use nrz_bindings::{
    ExactConstant, LcCertainty, LcPairParameters, LcPhaseGauge, LcQuantity as Q, LcRelation,
    LcRelationError, LcSampleTimes, LcTimingCover,
};
use nrz_cas::{Rational, syntax::Constant};
use std::cmp::Ordering;

/// Independent ratios give the exact interval and every boundary restores original times.
/// ´test:integration:lc-relation-timing-exact-recovery´
#[test]
fn lc_relation_timing_exact_recovery() -> TestResult {
    let timing = sample_times()?;
    let cover = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_eq!(cover.rate(), &[number("20/11")?, number("20/9")?]);
    assert_eq!(cover.certainty(), LcCertainty::Certified);
    assert_eq!(cover.phase_gauge(), LcPhaseGauge::Fixed);
    assert_eq!(cover.cycles(), cycles());
    assert!(cover.relation().shares_allocation(timing.relation()));
    assert!(
        cover
            .feasibility_witness()
            .relation()
            .shares_allocation(timing.relation())
    );
    check_fibres(&cover)?;
    let product = cover.bounds(Q::Product, &mut ledger()?)?;
    assert_eq!(product.lower(), Some(&number("81/400")?.into()));
    assert_eq!(product.upper(), Some(&number("121/400")?.into()));
    assert!(matches!(
        cover.phase_at_rate(&number("17/10")?, &mut ledger()?),
        Err(LcRelationError::Containment)
    ));
    Ok(())
}

/// Repeated phase intersections and supplied physical scales survive original reconstruction.
/// ´test:integration:lc-relation-timing-exact-original-inputs´
#[test]
fn lc_relation_timing_exact_original_inputs() -> TestResult {
    const FIRST: usize = 0;
    let original = original()?;
    let mut inputs = original.inputs().to_vec();
    inputs.push(input(Q::Phase(FIRST), ["19/10", "21/10"])?);
    inputs.push(input(Q::Capacitance, ["1/2", "1/2"])?);
    inputs.push(input(Q::Energy, ["1/4", "1/4"])?);
    let relation = LcRelation::new(original.epoch().into(), 2, inputs, &mut ledger()?)?;
    let timing = LcSampleTimes::new(relation, vec![number("1")?, number("2")?], &mut ledger()?)?;
    let cover = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_eq!(cover.rate(), &[number("20/11")?, number("20/9")?]);
    check_fibres(&cover)?;
    for rate in cover.rate() {
        let witness = cover.witness(&parameters(rate.clone(), number("0")?), &mut ledger()?)?;
        assert_eq!(
            witness.coordinate(Q::Capacitance),
            Some(&number("1/2")?.into())
        );
        assert_eq!(witness.coordinate(Q::Energy), Some(&number("1/4")?.into()));
        assert_eq!(
            witness.coordinate(Q::Phase(FIRST)),
            Some(&number("2")?.into())
        );
    }
    Ok(())
}

/// Unknown initial phase preserves a sloped joint fibre and the complete common integer gauge.
/// ´test:integration:lc-relation-timing-exact-joint-phase´
#[test]
fn lc_relation_timing_exact_joint_phase() -> TestResult {
    let original = original()?;
    let inputs = original
        .inputs()
        .iter()
        .filter(|input| input.quantity() != Q::InitialPhase)
        .cloned()
        .collect();
    let relation = LcRelation::new(original.epoch().into(), 2, inputs, &mut ledger()?)?;
    let timing = LcSampleTimes::new(relation, vec![number("1")?, number("2")?], &mut ledger()?)?;
    let cover = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_eq!(cover.rate(), &[number("10/7")?, number("20/7")?]);
    assert_eq!(cover.phase_gauge(), LcPhaseGauge::AllIntegers);
    let outer = timing.outer_pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_eq!(outer.phase_gauge(), cover.phase_gauge());
    for (rate, phase) in [("10/7", "5/7"), ("20/7", "-8/7")] {
        assert_eq!(
            cover.phase_at_rate(&number(rate)?, &mut ledger()?)?,
            [number(phase)?, number(phase)?]
        );
        for gauge in ["-2", "0", "3"] {
            let mut parameters = parameters(number(rate)?, number(phase)?);
            parameters.gauge = number(gauge)?;
            let witness = cover.witness(&parameters, &mut ledger()?)?;
            cover
                .relation()
                .check_witness(witness.values().clone(), &mut ledger()?)?;
        }
    }
    assert!(
        cover
            .witness(
                &parameters(number("10/7")?, number("-8/7")?),
                &mut ledger()?
            )
            .is_err()
    );
    check_fibres(&cover)
}

/// Generated exact domains never widen the corner-hull relaxation, including phase intervals.
/// ´test:integration:lc-relation-timing-exact-tightens-outer´
#[test]
fn lc_relation_timing_exact_tightens_outer() -> TestResult {
    for width in 1..=5 {
        let timing = generated(width)?;
        let outer = timing.outer_pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
        let exact = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
        assert_ne!(
            exact.rate()[0].compare(&outer.rate()[0], &mut ledger()?)?,
            Ordering::Less
        );
        assert_ne!(
            exact.rate()[1].compare(&outer.rate()[1], &mut ledger()?)?,
            Ordering::Greater
        );
        assert_eq!(exact.rate(), &generated_oracle(width)?);
        check_fibres(&exact)?;
    }
    let timing = sample_times()?;
    let outer = timing.outer_pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    let exact = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_ne!(outer.rate(), exact.rate());
    Ok(())
}

/// Lifted irrational phase representatives preserve their original nonzero integer branches.
/// ´test:integration:lc-relation-timing-exact-lifted-cycles´
#[test]
fn lc_relation_timing_exact_lifted_cycles() -> TestResult {
    let mut inputs = Vec::new();
    let original = original()?;
    let turn = ExactConstant::named(Constant::Pi, &mut ledger()?)?
        .mul(&number("2")?.into(), &mut ledger()?)?;
    for original in original.inputs() {
        match original.quantity() {
            Q::InitialPhase => inputs.push(input(Q::InitialPhase, ["5", "5"])?),
            Q::Phase(index) => {
                let value = original
                    .interval()
                    .lower()
                    .ok_or("phase")?
                    .add(&number("5")?.into(), &mut ledger()?)?
                    .sub(&turn, &mut ledger()?)?;
                inputs.push(super::super::support::exact_input(Q::Phase(index), value)?);
            }
            _ => inputs.push(original.clone()),
        }
    }
    let relation = LcRelation::new(original.epoch().into(), 2, inputs, &mut ledger()?)?;
    let timing = LcSampleTimes::new(relation, vec![number("1")?, number("2")?], &mut ledger()?)?;
    let cycles = vec![number("1")?, number("1")?];
    let mut measured = ledger()?;
    let cover = timing.pairs_cover(cycles.clone(), &pi()?, &mut measured)?;
    assert_eq!(measured.usage().work, 1_404_973);
    assert_eq!(cover.cycles(), cycles);
    assert_eq!(cover.rate(), &[number("20/11")?, number("20/9")?]);
    lifted_work_boundaries(&timing, &cover)
}

/// Zero-crossing time supports require no division by a nominal time or uncertain difference.
/// ´test:integration:lc-relation-timing-exact-zero-crossing´
#[test]
fn lc_relation_timing_exact_zero_crossing() -> TestResult {
    let mut inputs = vec![
        input(Q::Rate, ["1", "3"])?,
        input(Q::InitialPhase, ["0", "0"])?,
    ];
    for (index, (times, phase)) in [(["-1", "1"], "0"), (["0", "2"], "2")]
        .into_iter()
        .enumerate()
    {
        inputs.push(input(Q::Time(index), times)?);
        inputs.push(input(Q::Phase(index), [phase, phase])?);
    }
    let relation = LcRelation::new("zero crossing epoch".into(), 2, inputs, &mut ledger()?)?;
    let timing = LcSampleTimes::new(relation, vec![number("0")?, number("1")?], &mut ledger()?)?;
    let cover = timing.pairs_cover(cycles(), &pi()?, &mut ledger()?)?;
    assert_eq!(cover.rate(), &[number("1")?, number("3")?]);
    check_fibres(&cover)
}

/// Complete coverage refuses broad unresolved cycle domains and preserves cumulative Work.
/// ´test:integration:lc-relation-timing-exact-refusals-work´
#[test]
fn lc_relation_timing_exact_refusals_work() -> TestResult {
    use nrz_bindings::{Budget, Limits, LimitsOverride};
    let original = original()?;
    let mut inputs: Vec<_> = original
        .inputs()
        .iter()
        .filter(|input| input.quantity() != Q::Rate)
        .cloned()
        .collect();
    inputs.push(input(Q::Rate, ["1", "20"])?);
    let relation = LcRelation::new(original.epoch().into(), 2, inputs, &mut ledger()?)?;
    let broad = LcSampleTimes::new(relation, vec![number("1")?, number("2")?], &mut ledger()?)?;
    assert!(matches!(
        broad.pairs_cover(cycles(), &pi()?, &mut ledger()?),
        Err(LcRelationError::CycleCover)
    ));
    let timing = sample_times()?;
    let pi = pi()?;
    let mut measured = ledger()?;
    let cover = timing.pairs_cover(cycles(), &pi, &mut measured)?;
    let work = measured.usage().work;
    assert_eq!(cover.usage()[1].work - cover.usage()[0].work, work);
    assert!(cover.retained_payload_bytes() > 0);
    for (limit, succeeds) in [(work + 17, true), (work + 16, false)] {
        let limits = Limits::default().resolve(&LimitsOverride {
            work: Some(limit),
            ..LimitsOverride::default()
        })?;
        let mut budget = Budget::new(limits)?;
        budget.work(17, nrz_cas::Location::new("exact timing prefix"))?;
        assert_eq!(
            timing.pairs_cover(cycles(), &pi, &mut budget).is_ok(),
            succeeds
        );
        assert!(budget.usage().work >= 17);
    }
    Ok(())
}

/// Generated observation boxes are independent of the strip implementation.
fn generated(width: i64) -> TestResult<LcSampleTimes> {
    let mut budget = ledger()?;
    let error = Rational::from_ratio(width, 100, &mut budget)?;
    let mut inputs = vec![
        input(Q::Rate, ["1", "3"])?,
        input(Q::InitialPhase, ["0", "0"])?,
    ];
    for (index, (center, phase)) in [("1", "2"), ("2", "4")].into_iter().enumerate() {
        let center = number(center)?;
        let phase = number(phase)?;
        let left_error = if index == 0 {
            error.clone()
        } else {
            error.mul(&number("2")?, &mut budget)?
        };
        for (quantity, range) in [
            (
                Q::Time(index),
                [
                    center.sub(&left_error, &mut budget)?,
                    center.add(&error, &mut budget)?,
                ],
            ),
            (
                Q::Phase(index),
                [
                    phase.sub(&error, &mut budget)?,
                    phase.add(&error, &mut budget)?,
                ],
            ),
        ] {
            inputs.push(rational_input(quantity, range)?);
        }
    }
    let relation = LcRelation::new("generated timing epoch".into(), 2, inputs, &mut ledger()?)?;
    let timing = LcSampleTimes::new(relation, vec![number("1")?, number("2")?], &mut ledger()?)?;
    Ok(timing)
}

/// Positive ratios independently determine both endpoints for the generated known-zero family.
fn generated_oracle(width: i64) -> TestResult<[Rational; 2]> {
    let mut budget = ledger()?;
    let error = Rational::from_ratio(width, 100, &mut budget)?;
    let lower = number("4")?
        .sub(&error, &mut budget)?
        .div(&number("2")?.add(&error, &mut budget)?, &mut budget)?;
    let upper = number("4")?.add(&error, &mut budget)?.div(
        &number("2")?.sub(&error.mul(&number("2")?, &mut budget)?, &mut budget)?,
        &mut budget,
    )?;
    Ok([lower, upper])
}

/// Independent exact input construction for generated rational endpoint oracles.
fn rational_input(quantity: Q, range: [Rational; 2]) -> TestResult<nrz_bindings::LcInput> {
    let [lower, upper] = range;
    super::super::support::input(quantity, Some(lower.into()), Some(upper.into()))
}

/// Unit free physical scales with an explicitly selected local phase and common gauge.
fn parameters(rate: Rational, initial_phase: Rational) -> LcPairParameters {
    LcPairParameters {
        rate,
        initial_phase,
        ratio: Rational::one(),
        voltage: Rational::one(),
        gauge: Rational::zero(),
    }
}

/// Endpoints and midpoint of each fibre reconstruct original equations at all tested rates.
fn check_fibres(cover: &LcTimingCover) -> TestResult {
    let mut budget = ledger()?;
    let middle = cover.rate()[0]
        .add(&cover.rate()[1], &mut budget)?
        .div(&number("2")?, &mut budget)?;
    for rate in [cover.rate()[0].clone(), middle, cover.rate()[1].clone()] {
        let phase = cover.phase_at_rate(&rate, &mut ledger()?)?;
        for initial_phase in phase {
            let mut measured = ledger()?;
            let witness = cover.witness(&parameters(rate.clone(), initial_phase), &mut measured)?;
            assert_eq!(
                witness.values().observations.len(),
                cover.relation().observations()
            );
            cover
                .relation()
                .check_witness(witness.values().clone(), &mut ledger()?)?;
        }
    }
    Ok(())
}

/// Literal budgets identify cover construction and three distinct rate witnesses at phase five.
/// Earlier duplicate measurements were the equal lower/upper phase endpoints on this exact route.
fn lifted_work_boundaries(timing: &LcSampleTimes, cover: &LcTimingCover) -> TestResult {
    let cycles = vec![number("1")?, number("1")?];
    let pi = pi()?;
    for (remaining, succeeds) in [(1_404_973, true), (1_404_972, false)] {
        let mut budget = literal_budget(remaining)?;
        let outcome = timing.pairs_cover(cycles.clone(), &pi, &mut budget);
        assert_eq!(outcome.is_ok(), succeeds);
        if succeeds {
            assert_eq!(outcome?.rate(), cover.rate());
            assert_eq!(budget.usage().work, 1_404_973 + 17);
        } else {
            assert_work_refusal(&outcome);
        }
        assert!(budget.usage().work >= 17);
    }
    // Each witness adds 316 retained kinds + 105 local kinds + 368 identity
    // + 368 vertices = 1157 Work. The 17 earlier caller Work is separate.
    for (case, rate, limits) in [
        (
            "minimum rate, fixed phase five",
            "20/11",
            [897_059, 897_058],
        ),
        (
            "midpoint rate, fixed phase five",
            "200/99",
            [899_652, 899_651],
        ),
        ("maximum rate, fixed phase five", "20/9", [894_275, 894_274]),
    ] {
        let parameters = parameters(number(rate)?, number("5")?);
        for (remaining, succeeds) in limits.into_iter().zip([true, false]) {
            let mut budget = literal_budget(remaining)?;
            let outcome = cover.witness(&parameters, &mut budget);
            assert_eq!(outcome.is_ok(), succeeds, "{case}");
            if succeeds {
                let point = outcome?;
                cover
                    .relation()
                    .check_witness(point.values().clone(), &mut ledger()?)?;
                assert_eq!(budget.usage().work, remaining + 17, "{case}");
            } else {
                assert_work_refusal(&outcome);
            }
            assert!(budget.usage().work >= 17);
        }
    }
    Ok(())
}

/// A caller prefix is consumed before the literal remaining allowance reaches the operation.
fn literal_budget(remaining: u64) -> TestResult<nrz_bindings::Budget> {
    use nrz_bindings::{Budget, Limits, LimitsOverride};
    let limits = Limits::default().resolve(&LimitsOverride {
        work: Some(remaining + 17),
        ..LimitsOverride::default()
    })?;
    let mut budget = Budget::new(limits)?;
    budget.work(17, nrz_cas::Location::new("lifted timing literal prefix"))?;
    Ok(budget)
}

/// The adjacent failing row must exhaust Work, not pass because an unrelated refusal happened.
fn assert_work_refusal<T>(outcome: &Result<T, LcRelationError>) {
    assert!(matches!(
        outcome,
        Err(LcRelationError::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: nrz_cas::limits::BudgetKind::Work,
                ..
            },
            ..
        }))
    ));
}
