// SPDX-License-Identifier: AGPL-3.0-only
//! The complete consuming facade exposes neither symbols nor numerical publication.
use super::{LcOmegaOutput, LcShapeError, borrowed, capture};
use crate::loan::{
    lc_origin::{LcClosure, LcSchedule},
    registry::Token,
};
use crate::{ExecutionAccount, Heap, LcShapeTile};
use nrz_bindings::{
    LcOmegaCorrespondence, Registry, readings::algebra::enclosure::lc_root::Certificate,
};
use nrz_scalar::Scalar;
use std::{fmt, rc::Rc};

/// Funded immutable preparation; numerical evaluation and publication remain unavailable.
///
/// Original symbols cannot be extracted from either scalar specialization:
/// ```compile_fail,E0616
/// use nrz_matrix::LcOmegaPrepared;
/// fn extract(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.declaration; }
/// ```
/// ```compile_fail,E0616
/// use nrz_matrix::LcOmegaPrepared;
/// fn extract(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.correspondence; }
/// ```
#[repr(C)]
pub struct LcOmegaPrepared<'i, P: Scalar, const N: usize> {
    /// Exact original declaration and inventory borrow, moved only after successful join.
    pub(in crate::loan) declaration: LcOmegaOutput<'i, P, N>,
    /// Frozen mathematical law, distinct from native occupation authority.
    pub(in crate::loan) correspondence: Rc<LcOmegaCorrespondence>,
    /// Complete captured input closure and its prepaid native reservation tokens.
    pub(in crate::loan) receipt: LcOmegaJoinReceipt,
}

/// Inline unforgeable result of the funded native join.
#[repr(C)]
pub(in crate::loan) struct LcOmegaJoinReceipt {
    /// Exact positional closure, independent of caller tile and reader lifetimes.
    pub(in crate::loan) closure: Rc<LcClosure>,
    /// Fresh join identity, also funding both cleanup receipts.
    pub(in crate::loan) join: u64,
    /// Fixed requested operands, specialization and revision stamps.
    pub(in crate::loan) schedule: LcSchedule,
    /// Shared closure header reservation.
    pub(in crate::loan) header: Token,
    /// Exact-capacity positional backing reservation.
    pub(in crate::loan) rows: Token,
}

/// Consuming refusal retains the identical original declaration without allocating an error box.
#[repr(C)]
pub struct LcOmegaPrepareFailure<'i, P: Scalar, const N: usize> {
    /// Original declaration is recoverable even after private CAS publication.
    pub(in crate::loan) declaration: LcOmegaOutput<'i, P, N>,
    /// Precise bindings or native refusal, including its completed cumulative prefix.
    error: LcShapeError,
}
impl<'i, P: Scalar, const N: usize> LcOmegaPrepareFailure<'i, P, N> {
    /// Recover the unchanged declaration and refusal with no allocation or new Work.
    #[must_use]
    pub fn into_parts(self) -> (LcOmegaOutput<'i, P, N>, LcShapeError) {
        (self.declaration, self.error)
    }
}
impl<P: Scalar, const N: usize> fmt::Debug for LcOmegaPrepared<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaPrepared").finish_non_exhaustive()
    }
}
impl<P: Scalar, const N: usize> fmt::Debug for LcOmegaPrepareFailure<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaPrepareFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
#[expect(
    clippy::result_large_err,
    reason = "consuming refusal must return the identical declaration without allocating"
)]
impl<'i, P: Scalar + 'static, const N: usize> LcOmegaOutput<'i, P, N> {
    /// Consume the declaration into a funded immutable correspondence join.
    /// # Errors
    /// Returns the exact input declaration and precise refusal, retaining spent Work.
    pub fn prepare(
        self,
        heap: &mut Heap,
        tiles: &[&LcShapeTile<'i, P>],
        registry: &Registry,
        account: &mut ExecutionAccount,
    ) -> Result<LcOmegaPrepared<'i, P, N>, LcOmegaPrepareFailure<'i, P, N>> {
        self.prepare_inner(heap, tiles, registry, None, account)
    }

    /// Replay explicit portable evidence before the same funded consuming join.
    /// # Errors
    /// Returns the exact input declaration on replay or native refusal without resetting Work.
    pub fn prepare_replay(
        self,
        heap: &mut Heap,
        tiles: &[&LcShapeTile<'i, P>],
        registry: &Registry,
        certificate: &Certificate,
        account: &mut ExecutionAccount,
    ) -> Result<LcOmegaPrepared<'i, P, N>, LcOmegaPrepareFailure<'i, P, N>> {
        self.prepare_inner(heap, tiles, registry, Some(certificate), account)
    }

    /// Borrowing the ledger preserves its identity and all consumed usage counters.
    fn prepare_inner(
        self,
        heap: &mut Heap,
        tiles: &[&LcShapeTile<'i, P>],
        registry: &Registry,
        certificate: Option<&Certificate>,
        account: &mut ExecutionAccount,
    ) -> Result<LcOmegaPrepared<'i, P, N>, LcOmegaPrepareFailure<'i, P, N>> {
        self.prepare_on(
            heap,
            tiles,
            registry,
            certificate,
            account.identity,
            &mut account.ledger,
        )
    }

    /// Shared reader borrows end before the exclusive reservation/capture transaction.
    pub(in crate::loan::lc_shape) fn prepare_on(
        self,
        heap: &mut Heap,
        tiles: &[&LcShapeTile<'i, P>],
        registry: &Registry,
        certificate: Option<&Certificate>,
        identity: u64,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<LcOmegaPrepared<'i, P, N>, LcOmegaPrepareFailure<'i, P, N>> {
        let joined = (|| {
            let (candidate, plan) = borrowed::prepare_borrowed_on(
                &self,
                &*heap,
                tiles,
                registry,
                certificate,
                identity,
                budget,
            )?;
            capture::join_on(
                heap,
                candidate,
                plan,
                certificate.map_or(0, Certificate::retained_bytes),
                identity,
                budget,
            )
        })();
        match joined {
            Ok((correspondence, receipt)) => Ok(LcOmegaPrepared {
                declaration: self,
                correspondence,
                receipt,
            }),
            Err(error) => Err(LcOmegaPrepareFailure {
                declaration: self,
                error,
            }),
        }
    }
}

/// Field offsets pin the carrier without constructing a checked owner.
#[cfg(test)]
pub(super) fn layout<P: Scalar>() {
    use std::mem::{align_of, offset_of};
    assert_eq!(
        (
            size_of::<LcOmegaJoinReceipt>(),
            align_of::<LcOmegaJoinReceipt>()
        ),
        (112, 8)
    );
    assert_eq!(
        [
            offset_of!(LcOmegaJoinReceipt, closure),
            offset_of!(LcOmegaJoinReceipt, join),
            offset_of!(LcOmegaJoinReceipt, schedule),
            offset_of!(LcOmegaJoinReceipt, header),
            offset_of!(LcOmegaJoinReceipt, rows),
        ],
        [0, 8, 16, 64, 88]
    );
    assert_eq!(
        (
            size_of::<LcOmegaPrepared<'_, P, 5>>(),
            align_of::<LcOmegaPrepared<'_, P, 5>>()
        ),
        (920, 8)
    );
    assert_eq!(
        [
            offset_of!(LcOmegaPrepared<'_, P, 5>, declaration),
            offset_of!(LcOmegaPrepared<'_, P, 5>, correspondence),
            offset_of!(LcOmegaPrepared<'_, P, 5>, receipt),
        ],
        [0, 800, 808]
    );
    assert_eq!(offset_of!(LcOmegaPrepareFailure<'_, P, 5>, error), 800);
    assert_eq!(
        size_of::<LcOmegaPrepareFailure<'_, P, 5>>(),
        800 + size_of::<LcShapeError>()
    );
    assert_eq!(align_of::<LcOmegaPrepareFailure<'_, P, 5>>(), 8);
}
