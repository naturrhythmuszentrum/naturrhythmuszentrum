// SPDX-License-Identifier: AGPL-3.0-only
//! Wrapper state inspection retains only immutable demand after prepaid cancellation.
use crate::{ExecutionAccount, Heap, LcApplicationPending, LcOutputCellState, LoanError};
use nrz_scalar::Scalar;
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Both wrappers preserve the original state-query tariff and release their numerical storage.
pub(in crate::loan) fn boundaries<P: Scalar + 'static>(
    mut pending: LcApplicationPending<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    failed: bool,
) -> TestResult {
    if failed {
        pending
            .native
            .as_mut()
            .expect("pending storage")
            .storage
            .outcomes[11]
            .status = 6;
        let Err(super::super::LcApplicationEvaluationFailure::Failed { mut owner, .. }) =
            pending.evaluate(account)
        else {
            return Err("late malformed outcome must fail traversal".into());
        };
        inspect(
            |cell, account| owner.cell_state(cell, account),
            account,
            false,
        )?;
        let before = account.usage().work;
        owner.cancel(heap, account)?;
        super::super::surface::assert_cancelled_failed(&owner);
        assert_eq!(account.usage().work, before);
        inspect(
            |cell, account| owner.cell_state(cell, account),
            account,
            true,
        )?;
    } else {
        let mut owner = pending.evaluate(account).map_err(|_| "evaluation")?;
        inspect(
            |cell, account| owner.cell_state(cell, account),
            account,
            false,
        )?;
        let before = account.usage().work;
        owner.cancel(heap, account)?;
        assert!(owner.owner.native.is_none());
        assert!(owner.owner.cancelled.is_some());
        assert_eq!(account.usage().work, before);
        inspect(
            |cell, account| owner.cell_state(cell, account),
            account,
            true,
        )?;
        let before = account.usage().work;
        let failure = owner
            .publish(heap, account)
            .expect_err("cancelled candidate");
        assert!(matches!(failure.error, LoanError::WrongLoan));
        assert_eq!(account.usage().work, before);
    }
    Ok(())
}

/// Foreign authority precedes Work; bounds spend one; exact and zero neighbors use the same ledger.
fn inspect(
    query: impl Fn(usize, &mut ExecutionAccount) -> Result<LcOutputCellState, LoanError>,
    account: &mut ExecutionAccount,
    cancelled: bool,
) -> TestResult {
    let mut foreign =
        ExecutionAccount::new(nrz_bindings::Budget::new(nrz_bindings::Limits::default())?)?;
    let foreign_before = foreign.usage().work;
    let before = account.usage().work;
    assert!(matches!(
        query(0, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(foreign.usage().work, foreign_before);
    assert_eq!(account.usage().work, before);
    assert!(matches!(query(12, account), Err(LoanError::LayoutMismatch)));
    assert_eq!(account.usage().work - before, 1);
    assert_eq!(query(1, account)?, LcOutputCellState::NotRequested);
    assert_eq!(account.usage().work - before, 2);
    if cancelled {
        let consume = account.limits().work - account.usage().work - 1;
        account.ledger.work(consume, crate::loan::AT)?;
    }
    let before = account.usage().work;
    assert_eq!(
        query(0, account)?,
        if cancelled {
            LcOutputCellState::Cancelled
        } else {
            LcOutputCellState::Pending
        }
    );
    assert_eq!(account.usage().work - before, 1);
    if cancelled {
        assert!(matches!(query(0, account), Err(LoanError::Budget(_))));
        assert_eq!(account.usage().work - before, 1);
    }
    Ok(())
}
