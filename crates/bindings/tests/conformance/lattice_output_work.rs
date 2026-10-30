// SPDX-License-Identifier: AGPL-3.0-only
//! Literal owner boundaries preserve selected proofs and spent Work on every refusal.

use super::{
    decision_fixture::ledger,
    lattice_baseline::exhausted,
    lattice_selection::{admitted, prepared, select},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{Budget, DecisionError, Limits};
use nrz_cas::{FaultKind, Location, lattice::LatticeConvention, limits::BudgetKind};
use std::sync::{Arc, Mutex};

/// Separate physical admission and optional-site ownership counters.
struct Cost {
    /// Original fixture selector.
    kind: u8,
    /// Complete physical admission Work.
    admission: u64,
    /// Optional site Work.
    site: u64,
    /// Newly owned selected evidence and constants.
    bytes: u64,
    /// Newly owned site constants.
    site_bytes: u64,
}

/// Literal measurements from the deliberately failing checked-value reuse probe.
const COSTS: [Cost; 3] = [
    Cost {
        kind: 0,
        admission: 87_711,
        site: 385,
        bytes: 35_044,
        site_bytes: 384,
    },
    Cost {
        kind: 1,
        admission: 199_056,
        site: 514,
        bytes: 58_054,
        site_bytes: 512,
    },
    Cost {
        kind: 2,
        admission: 47_439,
        site: 514,
        bytes: 45_918,
        site_bytes: 512,
    },
];

/// Physical output admission has literal N/N−1 limits and cannot alter original graph or proof records.
/// ´claim:bindings:lattice-output-work´
/// ´test:integration:bindings-lattice-output-work´
#[test]
fn bindings_lattice_output_work() -> TestResult {
    for cost in COSTS {
        let mut value = prepared(cost.kind, LatticeConvention::Floor)?;
        let witness = select(&mut value)?;
        let outputs = value
            .fixture
            .frame
            .cas
            .lattice_selection_outputs(&witness, &mut ledger()?)?;
        let graph = Arc::new(Mutex::new(value.fixture.frame.cas));
        let before = graph.lock().map_err(|_| "graph owner poisoned")?.counts();
        for (work, accepted) in [(cost.admission + 17, true), (cost.admission + 16, false)] {
            let mut budget = prior(work)?;
            let result = value.case.admit(
                Arc::clone(&graph),
                &outputs,
                value.case.attachment(),
                &mut budget,
            );
            if accepted {
                let selected = result?;
                assert_eq!(selected.retained_payload_bytes(), cost.bytes);
                assert_eq!(selected.usage()[0].work, 17);
                assert_eq!(selected.usage()[1].work, work);
                assert_eq!(selected.witness().proof(), witness.proof());
                assert_eq!(
                    selected.reconstruction().proof(),
                    outputs.identity().proof()
                );
            } else {
                exhausted(&result.err().ok_or("short physical admission accepted")?)?;
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(
                graph.lock().map_err(|_| "graph owner poisoned")?.counts(),
                before
            );
        }
    }
    Ok(())
}

/// Optional sites have their own literal N/N−1 budget and leave the selected tuple reusable.
/// ´claim:bindings:lattice-site-work´
/// ´test:integration:bindings-lattice-site-work´
#[test]
fn bindings_lattice_site_work() -> TestResult {
    for cost in COSTS {
        let selected = admitted(cost.kind, LatticeConvention::Floor)?;
        let before = selected.cas()?.counts();
        let proof = selected.witness().proof().clone();
        for (work, accepted) in [(cost.site + 17, true), (cost.site + 16, false)] {
            let mut budget = prior(work)?;
            let result = selected.site(&mut budget);
            if accepted {
                let site = result?;
                assert_eq!(site.retained_payload_bytes(), cost.site_bytes);
                assert_eq!(budget.usage().work, work);
                assert_eq!(site.selection().witness().proof(), &proof);
            } else {
                exhausted(&result.err().ok_or("short optional site accepted")?)?;
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(selected.cas()?.counts(), before);
        }
    }
    Ok(())
}

/// Returned-output storage refusal leaves the mathematical witness intact for a later admission.
/// ´claim:bindings:lattice-output-storage´
/// ´test:integration:bindings-lattice-output-storage´
#[test]
fn bindings_lattice_output_storage() -> TestResult {
    let mut value = prepared(2, LatticeConvention::Floor)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    let graph = Arc::new(Mutex::new(value.fixture.frame.cas));
    let before = graph.lock().map_err(|_| "graph owner poisoned")?.counts();
    let mut budget = Budget::new(Limits {
        output_bytes: 1,
        ..Limits::default()
    })?;
    let error = value
        .case
        .admit(
            Arc::clone(&graph),
            &outputs,
            value.case.attachment(),
            &mut budget,
        )
        .err()
        .ok_or("short physical output accepted")?;
    let DecisionError::Algebra(fault) = error else {
        return Err("wrong output storage checker".into());
    };
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::OutputBytes,
            ..
        }
    ));
    assert_eq!(
        graph.lock().map_err(|_| "graph owner poisoned")?.counts(),
        before
    );
    let selected = value
        .case
        .admit(graph, &outputs, value.case.attachment(), &mut ledger()?)?;
    assert_eq!(selected.witness().proof(), witness.proof());
    Ok(())
}

/// A literal prior allowance belongs to the caller, not to a hidden fresh wrapper budget.
fn prior(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior physical lattice outputs"))?;
    Ok(budget)
}
