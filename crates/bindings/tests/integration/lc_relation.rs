// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC exact witness lifecycles; these are not projection or numerical acceptance tests.

mod forward_box;
mod quiescent;
mod refusals;
mod support;
mod timing;

use nrz_bindings::{LcBranch, LcInterval, LcQuantity as Q, LcRelation};
use support::{TestResult, exact_input, input, ledger, number, reference, relation};

/// The independent reference tuple satisfies every original equation and every supplied coordinate.
/// ´test:integration:lc-relation-exact-reference-witness´
#[test]
fn lc_relation_exact_reference_witness() -> TestResult {
    let values = reference()?;
    let mut inputs = Vec::new();
    for (quantity, text) in [
        (Q::Inductance, "1/40"),
        (Q::Capacitance, "1/100000"),
        (Q::InitialPhase, "0"),
        (Q::VoltageAmplitude, "5"),
        (Q::CurrentAmplitude, "1/10"),
        (Q::Energy, "1/8000"),
        (Q::Product, "1/4000000"),
        (Q::NaturalTime, "1/2000"),
        (Q::AmplitudeRatio, "50"),
        (Q::Rate, "2000"),
        (Q::AngularFrequency, "2000"),
    ] {
        inputs.push(exact_input(quantity, number(text)?)?);
    }
    for (i, observation) in values.observations.iter().enumerate() {
        inputs.push(exact_input(Q::Time(i), observation.time.clone())?);
        inputs.push(exact_input(
            Q::Phase(i),
            observation.phase.clone().ok_or("phase")?,
        )?);
    }
    let original = relation(inputs)?;
    let witness = original.check_witness(values, &mut ledger()?)?;
    assert!(original.shares_allocation(witness.relation()));
    assert_eq!(witness.branch(), LcBranch::Oscillating);
    assert_eq!(witness.coordinate(Q::AmplitudeRatio), Some(&number("50")?));
    assert_eq!(witness.coordinate(Q::Energy), Some(&number("1/8000")?));
    assert_eq!(witness.coordinate(Q::Product), Some(&number("1/4000000")?));
    assert_eq!(witness.relation().epoch(), "measurement epoch");
    assert_eq!(witness.relation().inputs().len(), 17);
    assert_eq!(witness.coordinate(Q::Time(3)), None);
    Ok(())
}

/// Unknown and supplied initial phase use distinct input families but share a valid exact witness.
/// ´test:integration:lc-relation-recovery-feasibility-witness´
#[test]
fn lc_relation_recovery_feasibility_witness() -> TestResult {
    for known_phase in [false, true] {
        let values = reference()?;
        let mut inputs = support::observations("1/20")?;
        inputs.push(input(
            Q::AngularFrequency,
            Some(number("1000")?),
            Some(number("3000")?),
        )?);
        if known_phase {
            inputs.push(exact_input(Q::InitialPhase, number("0")?)?);
        }
        let original = relation(inputs)?;
        let witness = original.check_witness(values, &mut ledger()?)?;
        assert_eq!(
            original
                .inputs()
                .iter()
                .any(|input| input.quantity() == Q::InitialPhase),
            known_phase
        );
        assert_eq!(witness.coordinate(Q::InitialPhase), Some(&number("0")?));
        for i in 0..3 {
            assert_eq!(
                witness.values().observations[i].cycle,
                Some(support::integer(i64::from(i == 2))?)
            );
        }
    }
    Ok(())
}

/// Supplied deterministic timing boxes are retained with every original phase equation.
/// ´test:integration:lc-relation-uncertain-time-witness´
#[test]
fn lc_relation_uncertain_time_witness() -> TestResult {
    let mut inputs = support::observations("1/20")?;
    inputs.push(input(
        Q::Time(1),
        Some(number("1/500")?),
        Some(number("3/1000")?),
    )?);
    let original = relation(inputs)?;
    let witness = original.check_witness(reference()?, &mut ledger()?)?;
    let timing = original.inputs().last().ok_or("timing input")?;
    assert_eq!(timing.interval().lower(), Some(&number("1/500")?));
    assert_eq!(timing.interval().upper(), Some(&number("3/1000")?));
    assert_eq!(witness.coordinate(Q::Time(1)), Some(&number("1/400")?));
    Ok(())
}

/// Empty input subsets keep missing bounds explicit and never choose an alias cutoff.
/// ´test:integration:lc-relation-unbounded-declaration´
#[test]
fn lc_relation_unbounded_declaration() -> TestResult {
    let original = relation(Vec::new())?;
    assert_eq!(original.inputs(), [] as [nrz_bindings::LcInput; 0]);
    let interval = LcInterval::unbounded();
    assert_eq!(interval.lower(), None);
    assert_eq!(interval.upper(), None);
    let witness = original.check_witness(reference()?, &mut ledger()?)?;
    assert!(witness.relation().shares_allocation(&original));
    let separate = relation(Vec::new())?;
    assert!(!witness.relation().shares_allocation(&separate));
    Ok(())
}

/// The quiescent domain retains natural frequency but supplies neither a phase nor a zero ratio.
/// ´test:integration:lc-relation-quiescent-witness´
#[test]
fn lc_relation_quiescent_witness() -> TestResult {
    let original = LcRelation::new(
        "rest epoch".into(),
        0,
        vec![
            exact_input(Q::Energy, number("0")?)?,
            exact_input(Q::Rate, number("2000")?)?,
        ],
        &mut ledger()?,
    )?;
    let witness = original.check_witness(support::quiescent()?, &mut ledger()?)?;
    assert_eq!(witness.branch(), LcBranch::Quiescent);
    assert_eq!(witness.coordinate(Q::InitialPhase), None);
    assert_eq!(witness.coordinate(Q::AmplitudeRatio), None);
    assert_eq!(witness.coordinate(Q::VoltageAmplitude), Some(&number("0")?));
    assert_eq!(witness.coordinate(Q::NaturalTime), Some(&number("1/2000")?));
    Ok(())
}
