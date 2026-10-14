// SPDX-License-Identifier: AGPL-3.0-only
//! Synthetic frozen graph paths, work boundaries and invalid edge costs.

use crate::frozen::{Arc, dijkstra};
use crate::limits::{Budget, BudgetKind, Limits};
use crate::testing::common::default_budget;
use crate::testing::frozen::node;
use crate::{Fault, FaultKind};

/// Every three-vertex directed graph agrees with enumeration of all simple target paths.
/// ´test:crate:frozen-path-enumeration´
#[test]
fn frozen_path_enumeration() -> Result<(), Fault> {
    let candidates = [
        (0, 1, 2),
        (0, 2, 5),
        (1, 0, 3),
        (1, 2, 1),
        (2, 0, 4),
        (2, 1, 2),
    ];
    for mask in 0_u8..64 {
        let mut arcs = Vec::new();
        for ((left, right, cost), bit) in candidates.into_iter().zip([1, 2, 4, 8, 16, 32]) {
            if mask & bit != 0 {
                arcs.push(Arc {
                    left: node(left),
                    right: node(right),
                    cost,
                });
            }
        }
        let direct = (mask & 2 != 0).then_some(5);
        let via_middle = (mask & 1 != 0 && mask & 8 != 0).then_some(3);
        let expected = direct.into_iter().chain(via_middle).min();
        let mut budget = default_budget()?;
        assert_eq!(dijkstra(&arcs, node(0), node(2), &mut budget)?.0, expected);
    }
    Ok(())
}

/// Exact work capacity succeeds, one unit less refuses, and no partial result is certified.
/// ´test:crate:frozen-work-boundary´
#[test]
fn frozen_work_boundary() -> Result<(), Fault> {
    let arcs = [
        Arc {
            left: node(0),
            right: node(1),
            cost: 2,
        },
        Arc {
            left: node(1),
            right: node(2),
            cost: 3,
        },
    ];
    let mut measured = default_budget()?;
    assert_eq!(
        dijkstra(&arcs, node(0), node(2), &mut measured)?,
        (Some(5), 3)
    );
    let needed = measured.usage().work;
    let mut exact = Budget::new(Limits {
        work: needed,
        ..Limits::default()
    })?;
    assert_eq!(dijkstra(&arcs, node(0), node(2), &mut exact)?, (Some(5), 3));
    let smaller = needed.checked_sub(1).ok_or_else(|| {
        Fault::capacity(
            BudgetKind::Work,
            crate::Location::new("independent fixture"),
        )
    })?;
    let mut narrow = Budget::new(Limits {
        work: smaller,
        ..Limits::default()
    })?;
    assert!(matches!(
        dijkstra(&arcs, node(0), node(2), &mut narrow),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Cost addition cannot wrap and zero-weight input cannot satisfy positive-edge assumptions.
/// ´test:crate:frozen-cost-refusals´
#[test]
fn frozen_cost_refusals() -> Result<(), Fault> {
    for first in [0, u64::MAX] {
        let arcs = [
            Arc {
                left: node(0),
                right: node(1),
                cost: first,
            },
            Arc {
                left: node(1),
                right: node(2),
                cost: 1,
            },
        ];
        assert!(
            dijkstra(
                &arcs,
                node(0),
                node(2),
                &mut Budget::new(Limits::default())?,
            )
            .is_err()
        );
    }
    Ok(())
}
