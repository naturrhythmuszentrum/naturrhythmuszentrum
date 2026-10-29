// SPDX-License-Identifier: AGPL-3.0-only
//! Physical coordinate preparation shares exact checking without widening scalar ratio admission.

use super::{decision_fixture::ledger, lattice_physical::source};
use crate::testing::declarations::TestResult;
use nrz_bindings::{Budget, DecisionCoordinate, DecisionError, DecisionExpression, Limits};
use nrz_cas::{FaultKind, Location, limits::BudgetKind};
use std::rc::Rc;

/// One independently measured physical-coordinate preparation owner.
struct Cost {
    /// Phase, two-angle or grid fixture.
    fixture: u8,
    /// Original declared physical coordinate.
    name: &'static str,
    /// Complete closed-source checker Work under unchanged defaults.
    work: u64,
}

/// Literal counters from the deliberately failing coordinate probe.
const COSTS: [Cost; 13] = [
    Cost {
        fixture: 0,
        name: "axis_a",
        work: 39_823,
    },
    Cost {
        fixture: 0,
        name: "period0",
        work: 39_184,
    },
    Cost {
        fixture: 0,
        name: "zero",
        work: 3_964,
    },
    Cost {
        fixture: 1,
        name: "axis_a",
        work: 6_463,
    },
    Cost {
        fixture: 1,
        name: "period0",
        work: 39_184,
    },
    Cost {
        fixture: 1,
        name: "zero",
        work: 3_964,
    },
    Cost {
        fixture: 1,
        name: "axis_b",
        work: 6_463,
    },
    Cost {
        fixture: 1,
        name: "period1",
        work: 39_184,
    },
    Cost {
        fixture: 2,
        name: "axis_a",
        work: 6_751,
    },
    Cost {
        fixture: 2,
        name: "period0",
        work: 6_656,
    },
    Cost {
        fixture: 2,
        name: "zero",
        work: 3_964,
    },
    Cost {
        fixture: 2,
        name: "axis_b",
        work: 6_655,
    },
    Cost {
        fixture: 2,
        name: "period1",
        work: 6_656,
    },
];

/// Every physical source has a literal N/N−1 check, retaining its kind and original graph.
/// ´claim:bindings:lattice-coordinate-work´
/// ´test:integration:bindings-lattice-coordinate-work´
#[test]
fn bindings_lattice_coordinate_work() -> TestResult {
    for cost in COSTS {
        let owner = source(cost.fixture, "1", "original physical source")?;
        let before = owner.cas()?.counts();
        for (work, accepted) in [(cost.work + 17, true), (cost.work + 16, false)] {
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            budget.work(17, Location::new("prior physical coordinate preparation"))?;
            let result = DecisionCoordinate::declared(Rc::clone(&owner), cost.name, &mut budget);
            if accepted {
                let coordinate = result?;
                assert_eq!(
                    coordinate.checked()?.descriptor(),
                    owner.checked(cost.name)?.descriptor()
                );
                assert_eq!(coordinate.owner().id(), owner.id());
                assert_eq!(budget.usage().work, work);
            } else {
                let error = result
                    .err()
                    .ok_or("short coordinate preparation accepted")?;
                let DecisionError::Algebra(fault) = error else {
                    return Err("wrong coordinate refusal owner".into());
                };
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(owner.cas()?.counts(), before);
        }
    }
    Ok(())
}

/// Physical angle preparation cannot turn the scalar count route's ratio into an angle.
/// ´claim:bindings:lattice-coordinate-ratio-boundary´
/// ´test:integration:bindings-lattice-coordinate-ratio-boundary´
#[test]
fn bindings_lattice_coordinate_ratio_boundary() -> TestResult {
    let owner = source(0, "1", "original phase source")?;
    assert!(matches!(
        DecisionExpression::declared(Rc::clone(&owner), "axis_a", &mut ledger()?),
        Err(DecisionError::Target)
    ));
    let coordinate = DecisionCoordinate::declared(owner, "axis_a", &mut ledger()?)?;
    assert_eq!(
        coordinate.checked()?.descriptor().kind(),
        &nrz_bindings::Kind::Angle
    );
    Ok(())
}
