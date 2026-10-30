// SPDX-License-Identifier: AGPL-3.0-only
//! Undecided evidence is complete, local and retained through atomic budget refusal.

use super::{TestResult, ledger, prepared};
use nrz_cas::lattice::{
    LatticeAttemptPolicy, LatticeEnclosure, LatticePendingObligation, LatticeSelectionOutcome,
    LatticeUndecidedReason,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{Cas, FaultKind, InvalidCode, Location, Rational};

/// A valid torus or phase preparation retains its Machin judgment even when the later cut is undecided.
/// ´test:integration:lattice-tuple-undecided-evidence´
#[test]
fn lattice_tuple_undecided_evidence() -> TestResult {
    for kind in [0, 3] {
        let mut source = prepared(kind)?;
        let before = source.source.frame.cas.counts();
        let mut preparation = ledger()?;
        preparation.work(37, Location::new("prior lattice preparation"))?;
        let attempt = source.source.frame.cas.prepare_lattice_selection(
            &source.cover,
            &source.expected,
            &source.enclosure,
            &mut preparation,
        )?;
        assert_eq!(attempt.candidate(), &source.expected);
        assert_eq!(attempt.enclosure(), &source.enclosure);
        assert_eq!(attempt.cover_proof(), source.cover.proof());
        assert_eq!(attempt.spent_work(), preparation.usage().work);
        assert_eq!(
            attempt.policy(),
            LatticeAttemptPolicy::NamedTupleSingleEnclosure
        );
        assert_eq!(
            attempt.pending_obligations(),
            [
                LatticePendingObligation::WholeBoxCuts,
                LatticePendingObligation::PortableWitness
            ]
        );
        let cut = attempt.cut(0).ok_or("missing attempted cut")?;
        assert_eq!(cut.candidate, &source.expected.values[0]);
        assert_eq!(attempt.cut(source.expected.values.len()), None);
        let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
        let mut budget = preparation;
        let LatticeSelectionOutcome::Undecided(outcome) = source
            .source
            .frame
            .cas
            .select_lattice_attempt(&attempt, &mut budget)?
        else {
            return Err("straddle returned a tuple".into());
        };
        assert_eq!(outcome.reason, LatticeUndecidedReason::BoundaryIntersection);
        assert!(std::ptr::eq(outcome.evidence, &raw const attempt));
        assert_eq!(outcome.spent_work, budget.usage().work);
        assert!(outcome.spent_work > attempt.spent_work());
        assert_eq!(outcome.fault.location.operation, "lattice tuple selection");
        crate::conformance::pi_bounds::graph_after_citation(&source.source.frame.cas, before, true);
        assert_eq!(
            source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
    }
    Ok(())
}

fn exhausted(work: u64) -> TestResult {
    let mut source = prepared(1)?;
    let attempt = source.source.frame.cas.prepare_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    let before = source.source.frame.cas.counts();
    let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let limits = Limits {
        work,
        ..Limits::default()
    };
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, Location::new("spent before lattice decision"))?;
    let LatticeSelectionOutcome::Undecided(outcome) = source
        .source
        .frame
        .cas
        .select_lattice_attempt(&attempt, &mut budget)?
    else {
        return Err("exhaustion returned a tuple".into());
    };
    assert_eq!(
        outcome.reason,
        LatticeUndecidedReason::DecisionBudgetExhausted
    );
    assert!(matches!(
        outcome.fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(outcome.spent_work, budget.usage().work);
    assert!(outcome.spent_work <= work);
    assert_eq!(outcome.spent_work == 37, work == 37);
    assert!(std::ptr::eq(outcome.evidence, &raw const attempt));
    assert_eq!(budget.limits(), &limits);
    assert_eq!(source.source.frame.cas.counts(), before);
    assert_eq!(
        source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    let LatticeSelectionOutcome::Decided(witness) = source
        .source
        .frame
        .cas
        .select_lattice_attempt(&attempt, &mut ledger()?)?
    else {
        return Err("valid retry remained undecided".into());
    };
    assert!(witness.is_discharged());
    Ok(())
}

/// Exhaustion before checking or at publication borrows complete evidence and publishes no tuple.
/// ´test:integration:lattice-tuple-undecided-budget´
/// ´claim:cas:lattice-tuple-undecided-budget´
#[test]
fn lattice_tuple_undecided_budget() -> TestResult {
    exhausted(37)?;
    exhausted(386_818)?;
    let mut source = prepared(1)?;
    let attempt = source.source.frame.cas.prepare_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    let mut budget = Budget::new(Limits {
        work: 386_819,
        ..Limits::default()
    })?;
    budget.work(37, Location::new("spent before lattice decision"))?;
    let LatticeSelectionOutcome::Decided(witness) = source
        .source
        .frame
        .cas
        .select_lattice_attempt(&attempt, &mut budget)?
    else {
        panic!("prepared selection must succeed at its measured warm Work boundary");
    };
    assert!(witness.is_discharged());
    assert_eq!(budget.usage().work, 386_819);
    Ok(())
}

/// Preparation admits a checked Machin node at N; its N−1 refusal preserves every graph byte.
/// ´test:integration:lattice-tuple-attempt-work´
/// ´claim:cas:lattice-tuple-attempt-work´
#[test]
fn lattice_tuple_attempt_work() -> TestResult {
    for (kind, work) in [
        (0, 144_697),
        (1, 144_407),
        (2, 132_292),
        (3, 131_366),
        (4, 96_932),
        (5, 95_395),
    ] {
        for succeeds in [false, true] {
            let mut source = prepared(kind)?;
            let before = source.source.frame.cas.counts();
            let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
            let amount = work + 36 + u64::from(succeeds);
            let limits = Limits {
                work: amount,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior lattice preparation"))?;
            let result = source.source.frame.cas.prepare_lattice_selection(
                &source.cover,
                &source.expected,
                &source.enclosure,
                &mut budget,
            );
            if succeeds {
                assert_eq!(result?.spent_work(), amount);
                assert_eq!(budget.usage().work, amount);
            } else {
                let fault = result.err().ok_or("preparation exceeded Work")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
                assert!(budget.usage().work > 37);
                assert!(budget.usage().work <= amount);
            }
            assert_eq!(budget.limits(), &limits);
            let published = kind < 5 && succeeds;
            crate::conformance::pi_bounds::graph_after_citation(
                &source.source.frame.cas,
                before,
                published,
            );
            if !published {
                assert_eq!(
                    source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
                    snapshot
                );
            }
        }
    }
    Ok(())
}

/// Invalid original evidence cannot become undecided, and a sealed local attempt cannot cross arenas.
/// ´test:integration:lattice-tuple-attempt-refusals´
#[test]
fn lattice_tuple_attempt_refusals() -> TestResult {
    let mut source = prepared(1)?;
    let attempt = source.source.frame.cas.prepare_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    let mut receiver = Cas::new(Limits::default())?;
    let before = receiver.counts();
    let fault = receiver
        .select_lattice_attempt(&attempt, &mut ledger()?)
        .err()
        .ok_or("foreign attempt accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    assert_eq!(fault.location.operation, "handle lookup");
    assert_eq!(receiver.counts(), before);
    let LatticeEnclosure::Original(value) = &mut source.enclosure else {
        return Err("missing original enclosure".into());
    };
    value.original.coordinates[0].upper = Rational::one();
    let before = source.source.frame.cas.counts();
    let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let fault = source
        .source
        .frame
        .cas
        .prepare_lattice_selection(
            &source.cover,
            &source.expected,
            &source.enclosure,
            &mut ledger()?,
        )
        .err()
        .ok_or("false original box prepared")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::DecisionBox));
    assert_eq!(fault.location.operation, "exact rational enclosure");
    assert_eq!(source.source.frame.cas.counts(), before);
    assert_eq!(
        source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}
