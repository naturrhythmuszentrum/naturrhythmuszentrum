// SPDX-License-Identifier: AGPL-3.0-only
//! Complete fallible validation precedes the atomic move into private preallocated shells.
#[cfg(test)]
pub(in crate::loan) mod checks;
mod commit;
mod outcomes;
mod readers;
pub(in crate::loan) mod reclaim;
mod uses;
pub(in crate::loan) mod validate;
use super::execution::LcApplicationEvaluated;
use super::{Bundle, Use};
use crate::{ExecutionAccount, Heap, LoanError, LoanTicket};
use nrz_scalar::Scalar;
pub use outcomes::{LcCellOutcome, LcOutcomeKind, LcOutcomeView};
pub use readers::{LcAssurance, LcResultReader};
use std::{fmt, marker::PhantomData, rc::Rc};

/// A specialized immutable use retains its seal, complete source closure and numerical storage.
/// No ordinary tile, pin, output symbols or writable conversion is exposed.
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f32>) { let _ = result.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f32>) { let _ = result.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f32>) { let _ = result.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f32>) { let _ = result.error_symbol(); }
/// ```
/// ```compile_fail,E0277
/// use nrz_matrix::{LcPublishedResult, PublishedTile};
/// fn escape(result: LcPublishedResult<f32>) { let _: PublishedTile<f32> = result.into(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f64>) { let _ = result.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f64>) { let _ = result.value_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f64>) { let _ = result.bound_symbol(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcPublishedResult;
/// fn escape(result: LcPublishedResult<f64>) { let _ = result.error_symbol(); }
/// ```
/// ```compile_fail,E0277
/// use nrz_matrix::{LcPublishedResult, PublishedTile};
/// fn escape(result: LcPublishedResult<f64>) { let _: PublishedTile<f64> = result.into(); }
/// ```
pub struct LcPublishedResult<P: Scalar> {
    /// Separately funded use, including the initial preallocated use.
    record: Rc<Use>,
    /// Closed native scalar representation, authenticated at every reader admission.
    marker: PhantomData<P>,
}
impl<P: Scalar> fmt::Debug for LcPublishedResult<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcPublishedResult").finish_non_exhaustive()
    }
}
/// Refused publication returns the identical evaluated owner without recomputing its private cells.
#[derive(Debug)]
pub struct LcPublicationFailure<'i, P: Scalar, const N: usize> {
    /// Original evaluated owner and its prepaid cleanup.
    pub owner: LcApplicationEvaluated<'i, P, N>,
    /// Original structured authority, state, layout or resource refusal.
    pub error: LoanError,
}
impl<'i, P: Scalar + 'static, const N: usize> LcApplicationEvaluated<'i, P, N> {
    /// Validate and atomically publish the closed terminal partition on the original account.
    /// # Errors
    /// Refusal preserves this evaluated owner; retries validate without executing any kernel.
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns the original owner without allocation"
    )]
    pub fn publish(
        mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<LcPublishedResult<P>, LcPublicationFailure<'i, P, N>> {
        let result = account
            .authenticate(self.owner.identity.account)
            .and_then(|()| {
                account
                    .ledger
                    .with_work_phase(crate::loan::FAMILY, "LC publication", |budget| {
                        let native = self.owner.native.as_ref().ok_or(LoanError::WrongLoan)?;
                        let storage = &native.storage;
                        let original = self
                            .owner
                            .inventory
                            .input(0)
                            .ok_or(LoanError::TargetMismatch)?;
                        let query =
                            validate::address(std::ptr::from_ref(original.question().time()))?;
                        let correspondence =
                            validate::address(Rc::as_ptr(&storage.correspondence))?;
                        let source_closure = validate::address(Rc::as_ptr(&storage.closure))?;
                        let outcome_records = validate::address(storage.outcomes.as_ptr())?;
                        let span = tracing::trace_span!(
                            "nrz.publish",
                            family = "lc-origin",
                            application = storage.identity.request,
                            join = storage.closure.identity,
                            correspondence,
                            query,
                            output_declaration = storage.schedule.specialization,
                            sign_attempts_application = storage.identity.request,
                            source_closure,
                            demand = ?storage.demand,
                            outcome_records,
                            completed_mask = ?storage.identity.mask,
                            seal_application = storage.identity.request,
                            terminal_ticket = ?LoanTicket { identity: storage.identity },
                            terminal_revision = tracing::field::Empty,
                            delivered = false
                        );
                        let _entered = span.enter();
                        let (seal, revision) = validate::pending(&self.owner, heap, budget)?;
                        let result = commit::publish(&mut self.owner, heap, &seal, revision);
                        span.record("terminal_revision", revision);
                        span.record("delivered", true);
                        Ok(result)
                    })
            });
        match result {
            Ok(result) => Ok(result),
            Err(error) => Err(LcPublicationFailure { owner: self, error }),
        }
    }
}
impl<P: Scalar> LcPublishedResult<P> {
    /// The published law remains available while any original specialized use survives.
    pub(in crate::loan) fn retained_correspondence(
        &self,
    ) -> Option<&Rc<nrz_bindings::LcOmegaCorrespondence>> {
        self.bundle().retained_correspondence()
    }

    /// Diagnostic locator has no ordinary pin or import authority.
    /// # Panics
    /// Panics only if an internal invariant admits an incomplete published shell.
    #[must_use]
    pub fn ticket(&self) -> LoanTicket {
        LoanTicket {
            identity: self
                .bundle()
                .storage
                .as_ref()
                .expect("published storage")
                .identity,
        }
    }
    /// Owning use payloads are constructed only by checked no-fail publication or retention.
    fn bundle(&self) -> &Rc<Bundle> {
        &self.record.payload.as_ref().expect("published use").bundle
    }
}
