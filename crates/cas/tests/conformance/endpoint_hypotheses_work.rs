// SPDX-License-Identifier: AGPL-3.0-only
//! Complete scalar endpoint closure boundaries retain caller usage and rollback.

use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::{FaultKind, Location};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::endpoint_hypotheses::chain;

fn attempt(consume: bool, amount: u64) -> TestResult<(bool, u64)> {
    let mut f = chain(false, consume)?;
    let before = f.fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut limits = budget()?.limits().clone();
    limits.work = amount;
    let mut ledger = Budget::new(limits)?;
    ledger.work(37, Location::new("prior endpoint Work"))?;
    let result =
        ledger.with_retained_payload(19, Location::new("prior endpoint payload"), |ledger| {
            f.fixture
                .cas
                .check_proof(&f.fixture.context, &f.steps, ledger)
        });
    if let Err(fault) = &result {
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(f.fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok((result.is_ok(), ledger.usage().work))
}

/// The complete close runs at N and refuses at N minus one after its measured consumed prefix.
/// ´test:integration:endpoint-closure-work-boundaries´
#[test]
fn endpoint_closure_work_boundaries() -> TestResult {
    // Checking charges its actual prefix; it does not prepay the complete operation.
    // Totals include 37 prior Work and retain 19 caller payload bytes throughout.
    // Unchanged operation Work totals: 47364/95356. Local entries add 8+2+12+12 = 34,
    // respectively 14+2+18+18 = 52. The final charge is 4 Work.
    for (consume, total, prefix) in [(false, 47_398, 47_394), (true, 95_408, 95_404)] {
        assert_eq!(attempt(consume, total)?, (true, total));
        assert_eq!(attempt(consume, total - 1)?, (false, prefix));
    }
    Ok(())
}
