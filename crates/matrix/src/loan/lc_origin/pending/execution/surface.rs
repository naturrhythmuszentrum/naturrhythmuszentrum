// SPDX-License-Identifier: AGPL-3.0-only
//! Separate terminal wrappers prohibit a second traversal without changing the pending layout.
use super::super::LcApplicationPending;
use crate::{CancellationReceipt, ExecutionAccount, Heap, LcProductError, LoanError, LoanTicket};
use nrz_scalar::Scalar;
use std::fmt;

/// An evaluated private candidate can attempt sealed publication or cancel, without exposing private values.
///
/// ```compile_fail,E0599
/// use nrz_matrix::{LcApplicationEvaluated, ExecutionAccount};
/// fn repeat(owner: LcApplicationEvaluated<'_, f32, 5>, account: &mut ExecutionAccount) {
///     let _ = owner.evaluate(account);
/// }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f32, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f32, 5>) { let _ = owner.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f32, 5>) { let _ = owner.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f32, 5>) { let _ = owner.error_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f32, 5>) { let _ = owner.read(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.error_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationEvaluated;
/// fn escape(owner: LcApplicationEvaluated<'_, f64, 5>) { let _ = owner.read(); }
/// ```
pub struct LcApplicationEvaluated<'i, P: Scalar, const N: usize> {
    /// Same funded storage, captured closure and original inventory borrow.
    pub(in crate::loan::lc_origin::pending) owner: LcApplicationPending<'i, P, N>,
}
/// An unexpected traversal failure retains cleanup authority but cannot evaluate again.
///
/// ```compile_fail,E0599
/// use nrz_matrix::{LcApplicationFailed, ExecutionAccount};
/// fn repeat(owner: LcApplicationFailed<'_, f64, 5>, account: &mut ExecutionAccount) {
///     let _ = owner.evaluate(account);
/// }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.error_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.read(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f32, 5>) { let _ = owner.publish(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.error_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.read(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationFailed;
/// fn escape(owner: LcApplicationFailed<'_, f64, 5>) { let _ = owner.publish(); }
/// ```
pub struct LcApplicationFailed<'i, P: Scalar, const N: usize> {
    /// Earlier private records grant no result authority.
    owner: LcApplicationPending<'i, P, N>,
}
impl<P: Scalar, const N: usize> LcApplicationEvaluated<'_, P, N> {
    /// Borrow the still-owned law through the original native continuation.
    pub(in crate::loan) fn retained_correspondence(
        &self,
    ) -> Option<&std::rc::Rc<nrz_bindings::LcOmegaCorrespondence>> {
        self.owner.retained_correspondence()
    }
}
impl<P: Scalar, const N: usize> LcApplicationFailed<'_, P, N> {
    /// A failed traversal retains its law until the original cancellation releases it.
    pub(in crate::loan) fn retained_correspondence(
        &self,
    ) -> Option<&std::rc::Rc<nrz_bindings::LcOmegaCorrespondence>> {
        self.owner.retained_correspondence()
    }
}

/// Readiness or prepayment refusal preserves retry; a started traversal failure permits cancellation.
#[derive(Debug)]
pub enum LcApplicationEvaluationFailure<'i, P: Scalar, const N: usize> {
    /// No numerical traversal started and the identical pending owner can retry.
    Pending {
        /// Original owner with unchanged private buffers.
        owner: LcApplicationPending<'i, P, N>,
        /// Original structured error with completed Work retained.
        error: LcProductError,
    },
    /// A traversal started; no private prefix can be exposed or evaluated again.
    Failed {
        /// Same allocations and prepaid cleanup in a cancel-only wrapper.
        owner: LcApplicationFailed<'i, P, N>,
        /// Original structured phase error, never truncated to a cell code.
        error: LcProductError,
    },
}
impl<P: Scalar, const N: usize> fmt::Debug for LcApplicationEvaluated<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcApplicationEvaluated")
            .finish_non_exhaustive()
    }
}
impl<P: Scalar, const N: usize> fmt::Debug for LcApplicationFailed<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcApplicationFailed")
            .finish_non_exhaustive()
    }
}
impl<P: Scalar + 'static, const N: usize> LcApplicationEvaluated<'_, P, N> {
    /// Inspect producer state and immutable demand, including after cancellation.
    /// Requested unpublished cells report `Pending`; cancelled ones report `Cancelled`.
    /// This does not expose private terminal outcomes.
    /// # Errors
    /// Refuses foreign accounts, out-of-range tuples or exhausted one-Work inspection.
    pub fn cell_state(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<crate::LcOutputCellState, LoanError> {
        self.owner.cell_state(cell, account)
    }

    /// Diagnostic producer identity grants no ordinary pin or result access.
    #[must_use]
    pub const fn ticket(&self) -> LoanTicket {
        self.owner.ticket()
    }
    /// Discard the private candidate using its original prepaid cleanup.
    /// # Errors
    /// Refuses foreign or stale cancellation authority without spending Work.
    pub fn cancel(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<CancellationReceipt, LoanError> {
        self.owner.cancel(heap, account)
    }
}
impl<P: Scalar + 'static, const N: usize> LcApplicationFailed<'_, P, N> {
    /// Inspect producer state and immutable demand, including after cancellation.
    /// Requested unpublished cells report `Pending`; cancelled ones report `Cancelled`.
    /// This does not expose private terminal outcomes.
    /// # Errors
    /// Refuses foreign accounts, out-of-range tuples or exhausted one-Work inspection.
    pub fn cell_state(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<crate::LcOutputCellState, LoanError> {
        self.owner.cell_state(cell, account)
    }

    /// Discard the failed traversal using its original prepaid cleanup.
    /// # Errors
    /// Refuses foreign or stale cancellation authority without spending Work.
    pub fn cancel(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<CancellationReceipt, LoanError> {
        self.owner.cancel(heap, account)
    }
}
#[expect(
    clippy::result_large_err,
    reason = "failures return the original owner without allocating"
)]
impl<'i, P: Scalar + 'static, const N: usize> LcApplicationPending<'i, P, N> {
    /// Preflight complete captured coverage and execute one wholly prepaid private traversal.
    /// # Errors
    /// Readiness and prepayment refusals return this pending owner; an unexpected started
    /// traversal failure returns a cancel-only owner and its original structured error.
    pub fn evaluate(
        mut self,
        account: &mut ExecutionAccount,
    ) -> Result<LcApplicationEvaluated<'i, P, N>, LcApplicationEvaluationFailure<'i, P, N>> {
        let mut started = false;
        match super::run(&mut self, account, &mut started) {
            Ok(()) => Ok(LcApplicationEvaluated { owner: self }),
            Err(error) if started => Err(LcApplicationEvaluationFailure::Failed {
                owner: LcApplicationFailed { owner: self },
                error,
            }),
            Err(error) => Err(LcApplicationEvaluationFailure::Pending { owner: self, error }),
        }
    }
}

/// Test the failed facade's retained demand without exporting its private owner or storage.
#[cfg(test)]
pub(super) fn assert_cancelled_failed<P: Scalar, const N: usize>(
    owner: &LcApplicationFailed<'_, P, N>,
) {
    assert!(owner.owner.native.is_none());
    assert!(owner.owner.cancelled.is_some());
}
