// SPDX-License-Identifier: AGPL-3.0-only
//! Operation-local exact preparations share checked evidence while preserving original identities.

use super::{
    decision_fixture::ledger,
    lattice_baseline::exhausted,
    lattice_coordinate::COSTS,
    lattice_fixture::lattice,
    lattice_physical::{source, specification},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{Budget, DecisionCoordinate, LatticeDecisionCase, Limits};
use nrz_cas::Location;
use std::rc::Rc;

/// Every fixture constant shares one checked preparation; a repeated request costs exactly one Work.
/// ´claim:bindings:lattice-coordinate-cache-work´
/// ´test:integration:bindings-lattice-coordinate-cache-work´
#[test]
fn bindings_lattice_coordinate_cache_work() -> TestResult {
    for cost in COSTS {
        let owner = source(cost.fixture, "1", "original repeated coordinate")?;
        let before = owner.cas()?.counts();
        let mut single = ledger()?;
        DecisionCoordinate::declared_many(&[(Rc::clone(&owner), cost.name)], &mut single)?;
        assert_eq!(single.usage().work, cost.work);
        let requests = [
            (Rc::clone(&owner), cost.name),
            (Rc::clone(&owner), cost.name),
        ];
        for (work, accepted) in [(cost.work + 18, true), (cost.work + 17, false)] {
            let mut budget = prior(work)?;
            let result = DecisionCoordinate::declared_many(&requests, &mut budget);
            if accepted {
                let values = result?;
                assert_eq!(values.len(), 2);
                assert!(std::ptr::eq(values[0].exact(), values[1].exact()));
                assert_eq!(budget.usage().work, work);
            } else {
                exhausted(&result.err().ok_or("short cache lookup accepted")?)?;
                assert_eq!(budget.usage().work, cost.work + 17);
            }
            assert_eq!(owner.cas()?.counts(), before);
        }
    }
    Ok(())
}

/// Independent source snapshots and different original declarations never share an equal full turn.
/// ´claim:bindings:lattice-coordinate-cache-identity´
/// ´test:integration:bindings-lattice-coordinate-cache-identity´
#[test]
fn bindings_lattice_coordinate_cache_identity() -> TestResult {
    let owner = source(1, "1", "original full turn")?;
    let foreign = source(1, "1", "changed equal full turn")?;
    for other in [(Rc::clone(&owner), "period1"), (foreign, "period0")] {
        let before = owner.cas()?.counts();
        let foreign_before = other.0.cas()?.counts();
        let requests = [(Rc::clone(&owner), "period0"), other];
        for (work, accepted) in [(78_385, true), (78_384, false)] {
            let mut budget = prior(work)?;
            let result = DecisionCoordinate::declared_many(&requests, &mut budget);
            if accepted {
                let values = result?;
                assert_eq!(values[0].exact(), values[1].exact());
                assert!(!std::ptr::eq(values[0].exact(), values[1].exact()));
                let (first, _) = values[0].exact().graph().ok_or("missing first graph")?;
                let (second, _) = values[1]
                    .exact()
                    .graph()
                    .ok_or("missing independent graph")?;
                assert!(!std::ptr::eq(first, second));
                assert_eq!(budget.usage().work, 78_385);
            } else {
                exhausted(
                    &result
                        .err()
                        .ok_or("short independent source preparation accepted")?,
                )?;
                assert!(budget.usage().work >= 17);
            }
            assert_eq!(owner.cas()?.counts(), before);
            assert_eq!(requests[1].0.cas()?.counts(), foreign_before);
        }
    }
    Ok(())
}

/// The torus retains one full-turn graph for both basis entries; no cache crosses an operation.
/// ´claim:bindings:lattice-question-cache-evidence´
/// ´test:integration:bindings-lattice-question-cache-evidence´
#[test]
fn bindings_lattice_question_cache_evidence() -> TestResult {
    for (kind, expected) in [
        (0, [3, 0, 2, 32, 6_181]),
        (1, [8, 4, 1, 16, 4_407]),
        (2, [8, 3, 0, 0, 1_993]),
    ] {
        let mut fixture = lattice(kind)?;
        let owner = source(kind, "1", "original question source")?;
        let spec = specification(&mut fixture, kind, &owner)?;
        let before = fixture.frame.cas.counts();
        let mut previous = None;
        for _ in 0..2 {
            let mut budget = ledger()?;
            let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut budget)?;
            let usage = case.preparation_usage();
            assert_eq!(
                [
                    usage.requests,
                    usage.hits,
                    usage.graphs,
                    usage.records,
                    usage.bytes
                ],
                expected
            );
            if let Some(work) = previous {
                assert_eq!(budget.usage().work, work);
            }
            previous = Some(budget.usage().work);
        }
        assert_eq!(fixture.frame.cas.counts(), before);
    }
    Ok(())
}

/// Named prior caller spending is preserved by every cache success and refusal.
fn prior(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior exact preparation"))?;
    Ok(budget)
}
