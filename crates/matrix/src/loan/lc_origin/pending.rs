// SPDX-License-Identifier: AGPL-3.0-only
//! Consuming checked admission owns private storage; private execution is implemented, publication remains gated.
mod admission;
mod check;
pub(in crate::loan) mod execution;
mod plan;
pub(in crate::loan) mod reclaim;

use super::{LcBundleOwner, LcStorage, LcUse};
use crate::loan::registry::{CancellationReceipt, Identity, LoanTicket};
use crate::mask::storage::{LayoutRecord, MaskRecord};
use crate::{
    ExecutionAccount, Heap, LcInputInventory, LcOmegaPrepared, LcOutputCellState, LoanError,
    MaskVersion,
};
use nrz_bindings::LcOmegaCorrespondence;
use nrz_scalar::Scalar;
use std::{fmt, rc::Rc};

/// Storage specialized to the sealed omega correspondence.
type Storage = LcStorage<LcOmegaCorrespondence>;
/// Complete-bundle shell specialized to the same correspondence.
type Bundle = LcBundleOwner<LcOmegaCorrespondence>;
/// Initial use shell specialized to the same correspondence.
type Use = LcUse<LcOmegaCorrespondence>;

/// Field order destroys both private shells before storage's final liveness edge.
#[repr(C)]
struct Native {
    /// Private, uniquely mutable bundle destination without weak references.
    bundle: Rc<Bundle>,
    /// Private, uniquely mutable initial use destination without weak references.
    initial_use: Rc<Use>,
    /// All payload fields precede the liveness edge inside this box.
    storage: Box<Storage>,
}

/// Checked private output storage with immutable demand and no numerical reader.
///
/// This owner cannot expose output symbols, a writable loan, or a publication capability.
/// Cancellation preserves demand inspection while releasing all numerical storage.
///
/// ```compile_fail,E0616
/// use nrz_matrix::LcApplicationPending;
/// fn escape(owner: LcApplicationPending<'_, f32, 5>) { let _ = owner.native; }
/// ```
/// ```compile_fail,E0616
/// use nrz_matrix::LcApplicationPending;
/// fn escape(owner: LcApplicationPending<'_, f64, 5>) { let _ = owner.native; }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::{LcApplicationPending, Heap, ExecutionAccount};
/// fn publish(mut owner: LcApplicationPending<'_, f32, 5>, heap: &mut Heap, account: &mut ExecutionAccount) {
///     let _ = owner.publish(heap, account);
/// }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationPending;
/// fn symbols(owner: LcApplicationPending<'_, f64, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::LcApplicationPending;
/// fn symbols(owner: LcApplicationPending<'_, f32, 5>) { let _ = owner.symbols(); }
/// ```
/// ```compile_fail,E0599
/// use nrz_matrix::{LcApplicationPending, Heap, ExecutionAccount};
/// fn publish(mut owner: LcApplicationPending<'_, f64, 5>, heap: &mut Heap, account: &mut ExecutionAccount) {
///     let _ = owner.publish(heap, account);
/// }
/// ```
#[repr(C)]
pub struct LcApplicationPending<'i, P: Scalar, const N: usize> {
    /// Original inventory borrow survives successful consuming admission.
    inventory: &'i LcInputInventory<P, N>,
    /// Exact native output identity; never a reconstructed ownership capability.
    identity: Identity,
    /// Original immutable demand version.
    demand: MaskVersion,
    /// Sole private storage; absent only after terminal cancellation.
    native: Option<Native>,
    /// Demand edges moved out of storage on cancellation, without reacquisition.
    cancelled: Option<(Rc<MaskRecord>, Rc<LayoutRecord>)>,
}

/// Refused admission returns the identical prepared facade without allocating an error box.
#[repr(C)]
pub struct LcApplicationAdmissionFailure<'i, P: Scalar, const N: usize> {
    /// Original input owner, never replaced with a reconstruction.
    prepared: LcOmegaPrepared<'i, P, N>,
    /// Exact native refusal, retaining completed Work.
    error: LoanError,
}
impl<'i, P: Scalar, const N: usize> LcApplicationAdmissionFailure<'i, P, N> {
    /// Recover the same prepared owner and the refusal without new Work.
    #[must_use]
    pub fn into_parts(self) -> (LcOmegaPrepared<'i, P, N>, LoanError) {
        (self.prepared, self.error)
    }
}
impl<P: Scalar, const N: usize> fmt::Debug for LcApplicationAdmissionFailure<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcApplicationAdmissionFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl<P: Scalar, const N: usize> fmt::Debug for LcApplicationPending<'_, P, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcApplicationPending")
            .field("pending", &self.native.is_some())
            .finish_non_exhaustive()
    }
}
#[expect(
    clippy::result_large_err,
    reason = "refusal returns the unchanged prepared owner without allocating"
)]
impl<'i, P: Scalar + 'static, const N: usize> LcOmegaPrepared<'i, P, N> {
    /// Reserve private checked storage while retaining the captured source closure.
    /// # Errors
    /// Refusal returns this identical owner after releasing only the new acquired prefix.
    pub fn admit(
        self,
        heap: &mut Heap,
        demand: MaskVersion,
        account: &mut ExecutionAccount,
    ) -> Result<LcApplicationPending<'i, P, N>, LcApplicationAdmissionFailure<'i, P, N>> {
        let span = tracing::trace_span!(
            "nrz.loan",
            family = "lc-origin",
            application = tracing::field::Empty
        );
        let _entered = span.enter();
        let result =
            account
                .ledger
                .with_work_phase(crate::loan::FAMILY, "LC pending admission", |budget| {
                    admission::prepare(&self, heap, demand, account.identity, budget)
                });
        match result {
            Ok(transaction) => {
                let owner = admission::install(self, heap, demand, transaction);
                span.record("application", owner.identity.request);
                Ok(owner)
            }
            Err(error) => Err(LcApplicationAdmissionFailure {
                prepared: self,
                error,
            }),
        }
    }
}
impl<P: Scalar + 'static, const N: usize> LcApplicationPending<'_, P, N> {
    /// Original complete inventory retained by the checked admission.
    #[must_use]
    pub const fn inventory(&self) -> &LcInputInventory<P, N> {
        self.inventory
    }

    /// Immutable requested demand, independent of initialized output coverage.
    #[must_use]
    pub const fn demand(&self) -> MaskVersion {
        self.demand
    }

    /// Diagnostic producer locator; it grants no ordinary loan or reader authority.
    #[must_use]
    pub const fn ticket(&self) -> LoanTicket {
        LoanTicket {
            identity: self.identity,
        }
    }

    /// Inspect pending or cancelled demand without reading numerical storage.
    /// # Errors
    /// Refuses foreign accounts, out-of-range tuples and exhausted inspection Work.
    pub fn cell_state(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<LcOutputCellState, LoanError> {
        account.authenticate(self.identity.account)?;
        crate::loan::charge(&mut account.ledger, 1)?;
        let (mask, layout) = if let Some(native) = &self.native {
            (&native.storage.demand_mask, &native.storage.demand_layout)
        } else {
            let (mask, layout) = self.cancelled.as_ref().ok_or(LoanError::WrongLoan)?;
            (mask, layout)
        };
        if cell >= layout.axis.capacity() {
            return Err(LoanError::LayoutMismatch);
        }
        if !crate::loan::payload::bit(&mask.words, cell) {
            return Ok(LcOutputCellState::NotRequested);
        }
        Ok(if self.native.is_some() {
            LcOutputCellState::Pending
        } else {
            LcOutputCellState::Cancelled
        })
    }

    /// Release private storage with prepaid cleanup, preserving immutable demand inspection.
    /// # Errors
    /// Foreign or stale authority preserves the same pending owner and spends no new Work.
    pub fn cancel(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<CancellationReceipt, LoanError> {
        let _span = tracing::trace_span!(
            "nrz.retire",
            family = "lc-origin",
            application = self.identity.request,
            reason = "cancelled"
        )
        .entered();
        account.authenticate(self.identity.account)?;
        let native = self.native.as_ref().ok_or(LoanError::WrongLoan)?;
        check::pending(heap, self.identity, &native.storage.liveness)?;
        let revision = heap.loans.terminal_revision()?;
        let receipt = account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "LC pending cancellation",
            |_| {
                let native = self.native.take().ok_or(LoanError::WrongLoan)?;
                let Native {
                    bundle,
                    initial_use,
                    storage,
                } = native;
                drop(bundle);
                drop(initial_use);
                self.cancelled = Some(release_storage(storage));
                reclaim::terminal(
                    heap,
                    self.identity,
                    revision,
                    crate::AbsenceReason::Cancelled,
                )
            },
        )?;
        Ok(receipt)
    }
}

/// Finish payload destruction before returning edges that survive cancellation.
fn release_storage(storage: Box<Storage>) -> (Rc<MaskRecord>, Rc<LayoutRecord>) {
    let Storage {
        demand_mask,
        demand_layout,
        ..
    } = *storage;
    (demand_mask, demand_layout)
}

#[cfg(test)]
pub(in crate::loan) fn assert_private_demand_0_3_11<P: Scalar, const N: usize>(
    owner: &LcApplicationPending<'_, P, N>,
) {
    let native = owner.native.as_ref().expect("pending native storage");
    assert_eq!(Rc::weak_count(&native.bundle), 0);
    assert_eq!(Rc::weak_count(&native.initial_use), 0);
    assert_eq!(Rc::strong_count(&native.bundle), 1);
    assert_eq!(Rc::strong_count(&native.initial_use), 1);
    assert_eq!(Rc::weak_count(&native.storage.liveness), 18);
    assert_eq!(native.storage.outcomes.len(), 12);
    for (index, record) in native.storage.outcomes.iter().enumerate() {
        assert_eq!(record.status, u8::from([0, 3, 11].contains(&index)));
        assert_eq!((record.target, record.reason), (0, 0));
    }
    for channel in &native.storage.channels {
        assert!(channel.components.iter().all(|word| *word == 0));
        assert!(channel.complete.iter().all(|word| *word == 0));
    }
    assert!(native.storage.completed.words.iter().all(|word| *word == 0));
}

/// Independent inline preparation inventory, separate from retained native payloads.
#[cfg(test)]
pub(in crate::loan) fn layout<P: Scalar>() {
    use std::mem::{align_of, offset_of};
    assert_eq!(size_of::<crate::heap::slots::FixedSlotPlan<3>>(), 64);
    assert_eq!(
        size_of::<Option<crate::heap::slots::FixedSlotPlan<3>>>(),
        72
    );
    assert_eq!(
        size_of::<Option<crate::heap::slots::FixedSlotPlan<1>>>(),
        40
    );
    // Three optional fixed-key plans 152, capacities 24, slots and keys 272,
    // identities 16, table capacities 16, revision 8, cleanup 32, permit 48.
    assert_eq!(size_of::<crate::loan::registry::LcDebitIdentity>(), 64);
    assert_eq!(size_of::<plan::Plan>(), 568);
    assert_eq!(align_of::<plan::Plan>(), 8);
    // Identity 216, Plan 568, three PreparedSlots 216, debit Vec 24,
    // channels 336, outcome Vec 24, seven pointers 56, cleanup 32, liveness 8.
    assert_eq!(size_of::<admission::Transaction>(), 1_480);
    assert_eq!(align_of::<admission::Transaction>(), 8);
    assert_eq!(size_of::<Native>(), 24);
    assert_eq!(size_of::<crate::loan::registry::Debit>(), 264);
    assert_eq!(size_of::<crate::heap::identity::Key>(), 16);
    assert_eq!(size_of::<crate::heap::descriptors::DescriptorRecord>(), 248);
    assert_eq!(size_of::<crate::mask::storage::Native<LayoutRecord>>(), 24);
    assert_eq!(size_of::<crate::mask::storage::Native<MaskRecord>>(), 24);
    assert_eq!(size_of::<crate::loan::preparation::Preparation>(), 72);
    assert_eq!(size_of::<Storage>(), 1_392);
    assert_eq!(size_of::<LcOmegaPrepared<'_, P, 5>>(), 920);
    assert_eq!(size_of::<LcApplicationPending<'_, P, 5>>(), 312);
    assert_eq!(align_of::<LcApplicationPending<'_, P, 5>>(), 8);
    assert_eq!(
        [
            offset_of!(LcApplicationPending<'_, P, 5>, inventory),
            offset_of!(LcApplicationPending<'_, P, 5>, identity),
            offset_of!(LcApplicationPending<'_, P, 5>, demand),
            offset_of!(LcApplicationPending<'_, P, 5>, native),
            offset_of!(LcApplicationPending<'_, P, 5>, cancelled),
        ],
        [0, 8, 224, 272, 296]
    );
    assert_eq!(
        offset_of!(LcApplicationAdmissionFailure<'_, P, 5>, error),
        920
    );
    assert_eq!(
        size_of::<LcApplicationAdmissionFailure<'_, P, 5>>(),
        920 + size_of::<LoanError>()
    );
}

/// Reverse new debit positions, keeping every token and group locator paired with its record.
#[cfg(test)]
pub(in crate::loan) fn reverse_slots<P: Scalar, const N: usize>(
    heap: &mut Heap,
    owner: &mut LcApplicationPending<'_, P, N>,
) {
    let group = heap.loans.groups[owner.identity.root.slot]
        .as_mut()
        .expect("pending group");
    let crate::loan::registry::DebitInventory::LcApplication(entries) = &mut group.debits else {
        panic!("LC inventory")
    };
    let slots: Vec<_> = entries[..17]
        .iter()
        .map(|entry| entry.expect("new token").0)
        .collect();
    for (index, entry) in entries[..17].iter_mut().enumerate() {
        entry.as_mut().expect("new token").0 = slots[16 - index];
    }
    for index in 0..8 {
        heap.loans.debits.swap(slots[index], slots[16 - index]);
    }
    let debits = &mut owner.native.as_mut().expect("pending owner").storage.debits;
    for token in debits
        .numerical
        .iter_mut()
        .chain(debits.tracking.iter_mut())
        .chain([
            &mut debits.storage,
            &mut debits.liveness,
            &mut debits.layout,
            &mut debits.outcomes,
            &mut debits.completed_words,
            &mut debits.completed_header,
            &mut debits.bundle,
            &mut debits.initial_use,
        ])
    {
        let token = token.as_mut().expect("new allocation token");
        let index = slots
            .iter()
            .position(|slot| *slot == token.slot)
            .expect("original slot");
        token.slot = slots[16 - index];
    }
}

/// Exercise the approved zero-capacity Work fallback without a fictitious source owner.
#[cfg(test)]
pub(in crate::loan) fn zero_work() -> Result<(), LoanError> {
    plan::zero_work()
}

/// Exercise the independently calculated zero-capacity resource branch.
#[cfg(test)]
pub(in crate::loan) fn zero_bytes<P: Scalar + 'static>() -> Result<(), LoanError> {
    plan::zero_bytes::<P>()
}
