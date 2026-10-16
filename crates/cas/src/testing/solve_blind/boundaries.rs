// SPDX-License-Identifier: AGPL-3.0-only
//! Measured resource boundaries for generic generation and independent checking.

use crate::limits::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::solving::coefficients::model::CoefficientFailure;
use crate::solving::radicals::{proof::Produced, route::Audit};
use crate::testing::common::TestResult;

/// Only the selected counter changes; all other pinned limits stay identical.
struct Counter {
    /// Exact resource required to refuse one below its boundary.
    kind: BudgetKind,
    /// Measured cumulative consumption or peak.
    usage: fn(&BudgetUsage) -> u64,
    /// Set the corresponding ceiling in both ledgers.
    limit: fn(&mut Limits, u64),
}

/// Explicit accessors prevent substituting an unrelated resource ceiling.
fn counters() -> [Counter; 16] {
    [
        Counter {
            kind: BudgetKind::CoefficientOps,
            usage: |u| u.coefficient_ops,
            limit: |l, n| l.coefficient_ops = n,
        },
        Counter {
            kind: BudgetKind::CoefficientMaps,
            usage: |u| u.coefficient_maps,
            limit: |l, n| l.coefficient_maps = n,
        },
        Counter {
            kind: BudgetKind::TowerLevels,
            usage: |u| u.tower_levels,
            limit: |l, n| l.tower_levels = n,
        },
        Counter {
            kind: BudgetKind::TowerDegree,
            usage: |u| u.tower_degree,
            limit: |l, n| l.tower_degree = n,
        },
        Counter {
            kind: BudgetKind::TowerReductions,
            usage: |u| u.tower_reductions,
            limit: |l, n| l.tower_reductions = n,
        },
        Counter {
            kind: BudgetKind::GcdSteps,
            usage: |u| u.gcd_steps,
            limit: |l, n| l.gcd_steps = n,
        },
        Counter {
            kind: BudgetKind::OrbitPermutations,
            usage: |u| u.orbit_permutations,
            limit: |l, n| l.orbit_permutations = n,
        },
        Counter {
            kind: BudgetKind::OrbitMembers,
            usage: |u| u.orbit_members,
            limit: |l, n| l.orbit_members = n,
        },
        Counter {
            kind: BudgetKind::OrbitTerms,
            usage: |u| u.orbit_terms,
            limit: |l, n| l.orbit_terms = n,
        },
        Counter {
            kind: BudgetKind::SymmetricSteps,
            usage: |u| u.symmetric_steps,
            limit: |l, n| l.symmetric_steps = n,
        },
        Counter {
            kind: BudgetKind::ResolventDepth,
            usage: |u| u.resolvent_depth,
            limit: |l, n| l.resolvent_depth = n,
        },
        Counter {
            kind: BudgetKind::ResolventDegree,
            usage: |u| u.resolvent_degree,
            limit: |l, n| l.resolvent_degree = n,
        },
        Counter {
            kind: BudgetKind::SelectionSlots,
            usage: |u| u.selection_slots,
            limit: |l, n| l.selection_slots = n,
        },
        Counter {
            kind: BudgetKind::SolutionBranches,
            usage: |u| u.solution_branches,
            limit: |l, n| l.solution_branches = n,
        },
        Counter {
            kind: BudgetKind::SolveIdentities,
            usage: |u| u.solve_identities,
            limit: |l, n| l.solve_identities = n,
        },
        Counter {
            kind: BudgetKind::LinearPivots,
            usage: |u| u.linear_pivots,
            limit: |l, n| l.linear_pivots = n,
        },
    ]
}

/// These isolated fixtures have one exact resource owner at each measured failing boundary.
fn location(kind: BudgetKind) -> TestResult<crate::Location> {
    let operation = match kind {
        BudgetKind::CoefficientOps | BudgetKind::TowerReductions | BudgetKind::GcdSteps => {
            "exact coefficients"
        }
        BudgetKind::CoefficientMaps
        | BudgetKind::TowerLevels
        | BudgetKind::TowerDegree
        | BudgetKind::SelectionSlots
        | BudgetKind::SolutionBranches => "solving object admission",
        BudgetKind::OrbitPermutations
        | BudgetKind::OrbitTerms
        | BudgetKind::SymmetricSteps
        | BudgetKind::ResolventDepth
        | BudgetKind::ResolventDegree => "symmetric resolvent",
        BudgetKind::OrbitMembers => "finite permutation orbit",
        BudgetKind::SolveIdentities | BudgetKind::LinearPivots => "elementary solving",
        _ => return Err("unexpected boundary counter".into()),
    };
    Ok(crate::Location::new(operation))
}

/// Reset generation and replay independently for the actual production entry.
fn run(
    limits: Limits,
) -> TestResult<(
    Result<Produced, CoefficientFailure>,
    BudgetUsage,
    BudgetUsage,
)> {
    let (mut fixture, request) = super::source::polynomial(&[-4, -15, 0, 1])?;
    let mut generation = Budget::new(limits.clone())?;
    let mut checker = Budget::new(limits)?;
    let result = crate::solving::solve_search::produce(
        &mut fixture.cas,
        &request,
        &mut Audit::default(),
        &mut generation,
        &mut checker,
    );
    Ok((result, generation.usage(), checker.usage()))
}

/// Each exercised counter succeeds at N and fails specifically at N-1 with an exact fault location.
pub(crate) fn measured() -> TestResult {
    let (baseline, generation, checker) = run(Limits::blind_cubic())?;
    let baseline = baseline?;
    assert!(baseline.fault.is_none(), "{:?}", baseline.fault);
    assert_eq!(baseline.answer.unresolved_regions, []);
    let expected = baseline.answer.coverage.ok_or("boundary coverage")?.bytes;
    for counter in counters() {
        let required = (counter.usage)(&generation).max((counter.usage)(&checker));
        assert!(required > 0, "unexercised {:?}", counter.kind);
        for cap in [required, required - 1] {
            let mut limits = Limits::blind_cubic();
            (counter.limit)(&mut limits, cap);
            let (result, _, _) = run(limits)?;
            if cap == required {
                let result = result?;
                assert!(
                    result.fault.is_none(),
                    "{:?}: {:?}",
                    counter.kind,
                    result.fault
                );
                assert_eq!(result.answer.unresolved_regions, []);
                assert_eq!(
                    result.answer.coverage.ok_or("exact-cap coverage")?.bytes,
                    expected
                );
            } else {
                let fault = match result {
                    Err(CoefficientFailure::Fault(error)) => error,
                    Ok(result) => result.fault.ok_or("below-cap solve succeeded")?,
                    Err(error) => {
                        return Err(format!("boundary became unavailable: {error:?}").into());
                    }
                };
                assert_eq!(fault.location, location(counter.kind)?, "{fault:?}");
                assert!(
                    matches!(fault.kind, crate::FaultKind::Exhausted { budget, limit, needed, .. } if budget == counter.kind && limit == cap && needed > cap),
                    "{fault:?}"
                );
            }
        }
    }
    Ok(())
}
