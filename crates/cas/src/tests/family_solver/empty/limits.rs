// SPDX-License-Identifier: AGPL-3.0-only
//! Empty family pieces preserve both caller ledgers and roll back publication on refusal.

use super::{empty_witness, ledger, setup};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, FaultKind};
use crate::testing::common::TestResult;

fn attempt(checking: bool, cap: u64, prior: u64) -> TestResult<(Result<(), Fault>, u64)> {
    let (mut fixture, problem, _, steps) = setup()?;
    let witness = empty_witness(&mut fixture, &steps)?;
    let before = fixture.cas.counts();
    let mut limits = ledger()?.limits().clone();
    limits.work = cap + prior;
    let mut constrained = Budget::new(limits)?;
    constrained.work(prior, crate::Location::new("earlier caller work"))?;
    let mut independent = ledger()?;
    let result = if checking {
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut independent, &mut constrained)
    } else {
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut constrained, &mut independent)
    };
    if result.is_err() {
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok((result.map(|_| ()), constrained.usage().work))
}

/// Each independent caller ledger accepts N and refuses N-minus-one with atomic graph rollback.
/// ´test:crate:family-emptiness-cumulative-work-boundaries´
#[test]
fn family_emptiness_cumulative_work_boundaries() -> TestResult {
    for checking in [false, true] {
        let (result, required) = attempt(checking, 100_000_000, 0)?;
        result?;
        for prior in [0, 37] {
            for short in [0, 1] {
                let cap = required - short;
                let (result, work) = attempt(checking, cap, prior)?;
                if short == 0 {
                    result?;
                    assert_eq!(work, required + prior);
                } else {
                    let fault = result.err().ok_or("below-cap empty piece accepted")?;
                    assert!(
                        matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, limit, needed, .. }
                        if limit == cap + prior && needed == required + prior),
                        "{fault:?}"
                    );
                    assert!(work >= prior);
                }
            }
        }
    }
    Ok(())
}
