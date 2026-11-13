// SPDX-License-Identifier: AGPL-3.0-only
//! Admission, initialized setters and recoverable handback retain one unchanged owner core.

pub(super) mod admission;
pub(super) mod publication;

use super::{
    ExecutionAccount, HostChannel, HostTileDeclaration, LoanError, charge,
    payload::{LoanChannel, bit, set},
    reader::PublishedTile,
    registry::{
        BundleOwner, CancellationReceipt, CleanupReceipt, Declaration, Identity, Liveness,
        LoanTicket, PinRecord, PublicationReceipt, State, Token,
    },
};
use crate::{Heap, heap::descriptors::DescriptorRecord};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Request outcomes never discard an unreturned writable owner.
#[derive(Debug)]
pub enum LoanRequest<P: Scalar> {
    /// Fixed-capacity ownership is detached from the heap borrow.
    Granted(OwnedLoan<P>),
    /// Admission refused before installing an observable occupation.
    Refused(LoanError),
}
/// Consuming handback returns the sole writable owner on every refusal.
#[derive(Debug)]
pub enum HandbackOutcome<P: Scalar> {
    /// The complete immutable bundle and its initial use pin are published together.
    Published(PublishedTile<P>, PublicationReceipt),
    /// Ownership authentication refused before inspecting numerical payload.
    ReturnRejected(LoanError, OwnedLoan<P>),
    /// Authenticated preparation failed; repair or cancellation remains available.
    Repair(LoanError, OwnedLoan<P>),
}
/// Cancellation does not inspect numerical payload or require remaining evaluation Work.
#[derive(Debug)]
pub enum CancelOutcome<P: Scalar> {
    /// The local producer is terminal and its unpublished payload was destroyed.
    Cancelled(CancellationReceipt),
    /// The supplied owner is returned without changing the rightful producer.
    Rejected(LoanError, OwnedLoan<P>),
}
/// Sole writable host capability; tickets cannot reconstruct it and it is not cloneable.
#[derive(Debug)]
pub struct OwnedLoan<P: Scalar> {
    /// Every consuming wrapper moves this same core unchanged.
    pub(super) core: Box<HostCore>,
    /// Sealed scalar format.
    pub(super) marker: PhantomData<P>,
}
/// Destruction order releases numerical owners and all use edges before the final witness.
#[derive(Debug)]
pub(super) struct HostCore {
    /// A later checked consumer may move one funded law use through every wrapper.
    pub(super) retained_law: Option<super::retained_expm1::Attachment>,
    /// Complete exact ownership authority.
    pub(super) identity: Identity,
    /// Original binding declaration.
    pub(super) declaration: Declaration,
    /// Fixed channels with separate native allocations and initialization maps.
    pub(super) channels: [Option<LoanChannel>; 3],
    /// Native component layout is retained independently of heap table movement.
    pub(super) layout: Rc<crate::mask::storage::LayoutRecord>,
    /// Private empty immutable publication shell.
    pub(super) bundle: Rc<BundleOwner>,
    /// Private empty initial-pin shell; no weak reference escapes preparation.
    pub(super) pin: Rc<PinRecord>,
    /// Unique core, layout, liveness, bundle and initial-pin header reservations.
    pub(super) headers: [Token; 5],
    /// Fixed prepaid failure, cancellation and destruction entitlements.
    pub(super) cleanup: CleanupReceipt,
    /// Sole strong liveness witness is deliberately destroyed last.
    pub(super) witness: Rc<Liveness>,
}
impl<P: Scalar + 'static> OwnedLoan<P> {
    /// Diagnostic locator without ownership or retention authority.
    #[must_use]
    pub const fn ticket(&self) -> LoanTicket {
        LoanTicket {
            identity: self.core.identity,
        }
    }
    /// Fixed admitted logical cell capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.core.declaration.extent.rows()
    }
    /// Components required to complete one channel cell.
    #[must_use]
    pub const fn components(&self) -> usize {
        self.core.declaration.extent.columns()
    }
    /// Actual native component layout selected at admission.
    #[must_use]
    pub const fn layout(&self) -> crate::CellLayoutVersion {
        self.core.identity.layout
    }
    /// Store one initialized component, leaving it unchanged on bounds or Work refusal.
    /// # Errors
    /// Refuses foreign accounts, absent channels, bounds and exhausted original Work.
    pub fn write_component(
        &mut self,
        channel: HostChannel,
        cell: usize,
        component: usize,
        value: P,
        account: &mut ExecutionAccount,
    ) -> Result<(), LoanError> {
        account.authenticate(self.core.identity.account)?;
        let address = self.address(channel, cell, component)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "write component", |budget| {
                charge(budget, 1)?;
                let owner = self.core.channels[channel.index()]
                    .as_mut()
                    .ok_or(LoanError::UnsupportedContract)?;
                owner.matrix.write(address, value)?;
                set(
                    &mut owner.components,
                    cell + self.core.declaration.extent.rows() * component,
                    true,
                );
                Ok(())
            })
    }
    /// Store a complete cell after checking the entire shape and write allowance.
    /// # Errors
    /// A refusal changes no component or initialization bit in the addressed cell.
    pub fn write_cell(
        &mut self,
        channel: HostChannel,
        cell: usize,
        values: &[P],
        account: &mut ExecutionAccount,
    ) -> Result<(), LoanError> {
        account.authenticate(self.core.identity.account)?;
        if values.len() != self.components() {
            return Err(LoanError::LayoutMismatch);
        }
        self.address(
            channel,
            cell,
            values
                .len()
                .checked_sub(1)
                .ok_or(LoanError::LayoutMismatch)?,
        )?;
        let capacity = self.capacity();
        let layout = &self.core.layout.components;
        account
            .ledger
            .with_work_phase(super::FAMILY, "write cell", |budget| {
                charge(budget, values.len())?;
                let owner = self.core.channels[channel.index()]
                    .as_mut()
                    .ok_or(LoanError::UnsupportedContract)?;
                for (component, &value) in values.iter().enumerate() {
                    let address = layout.index(cell, component)?;
                    owner.matrix.write(address, value)?;
                    set(&mut owner.components, cell + capacity * component, true);
                }
                Ok(())
            })
    }
    /// Omit a previously initialized cell without reading or erasing its old numerical bytes.
    /// # Errors
    /// Refuses foreign accounts, absent channels, bounds or Work before changing any flag.
    pub fn clear_cell(
        &mut self,
        channel: HostChannel,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<(), LoanError> {
        account.authenticate(self.core.identity.account)?;
        self.address(channel, cell, 0)?;
        let capacity = self.capacity();
        let components = self.components();
        let last = cell + capacity * (components - 1);
        // A stride below one word visits every intervening word; larger strides visit
        // one distinct word per component ``rule:nrz:matrix-loans-owner-tariffs``.
        let touched = (last / 64)
            .checked_sub(cell / 64)
            .and_then(|span| span.checked_add(1))
            .ok_or(LoanError::CountOverflow)?
            .min(components);
        account
            .ledger
            .with_work_phase(super::FAMILY, "clear cell", |budget| {
                charge(
                    budget,
                    touched.checked_add(1).ok_or(LoanError::CountOverflow)?,
                )?;
                let owner = self.core.channels[channel.index()]
                    .as_mut()
                    .ok_or(LoanError::UnsupportedContract)?;
                for component in 0..components {
                    set(&mut owner.components, cell + capacity * component, false);
                }
                set(&mut owner.complete, cell, false);
                Ok(())
            })
    }
    /// Checked physical incidence precedes every write; initialization remains logical.
    fn address(
        &self,
        channel: HostChannel,
        cell: usize,
        component: usize,
    ) -> Result<usize, LoanError> {
        if cell >= self.capacity()
            || component >= self.components()
            || self.core.channels[channel.index()].is_none()
        {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(self.core.layout.components.index(cell, component)?)
    }
}
impl Heap {
    /// Admit checked fixed-capacity native ownership without retaining a heap borrow.
    pub fn request_loan<P: Scalar + 'static>(
        &mut self,
        declaration: HostTileDeclaration<P>,
        account: &mut ExecutionAccount,
    ) -> LoanRequest<P> {
        let layout = match crate::Layout::contiguous(declaration.extent) {
            Ok(layout) => layout,
            Err(error) => return LoanRequest::Refused(error.into()),
        };
        self.request_loan_with_layout(declaration, layout, account)
    }
    /// Admit an injective affine component map, including gapped and reversed storage.
    ///
    /// The map preserves the declaration's logical cells and components. All channels
    /// share its checked injective map; physical prefix and suffix padding is reserved.
    /// Explicit gather maps are outside this admission contract.
    pub fn request_loan_with_layout<P: Scalar + 'static>(
        &mut self,
        declaration: HostTileDeclaration<P>,
        layout: crate::Layout,
        account: &mut ExecutionAccount,
    ) -> LoanRequest<P> {
        match admission::request(self, declaration, layout, account) {
            Ok(loan) => LoanRequest::Granted(loan),
            Err(error) => LoanRequest::Refused(error),
        }
    }
    /// Authenticate a copyable locator without granting publication or cancellation authority.
    /// # Errors
    /// Reports foreign, stale, wrong or already completed identities.
    pub fn probe_loan(
        &self,
        ticket: LoanTicket,
        account: &mut ExecutionAccount,
    ) -> Result<(), LoanError> {
        account
            .ledger
            .with_work_phase(super::FAMILY, "ticket authentication", |budget| {
                charge(
                    budget,
                    2 + ticket.identity.channels.iter().flatten().count(),
                )
            })?;
        account.authenticate(ticket.identity.account)?;
        self.authenticate_identity(ticket.identity)
    }
    /// Scope precedes indexing, and every group member is authenticated before payload access.
    pub(super) fn authenticate_identity(&self, identity: Identity) -> Result<(), LoanError> {
        if identity.scope != self.scope {
            return Err(LoanError::ForeignOwner);
        }
        let group = self.loans.group(identity)?;
        if !matches!(group.state, State::Out(_)) {
            return Err(LoanError::DuplicateReturn);
        }
        for (channel, key) in HostChannel::ALL.into_iter().zip(identity.channels) {
            if let Some(key) = key {
                let DescriptorRecord::Managed(record) = self
                    .descriptors
                    .slots
                    .get(key)
                    .map_err(|_| LoanError::StaleOccupation)?
                else {
                    return Err(LoanError::StaleOccupation);
                };
                if record.identity != identity || record.channel != channel || record.filled {
                    return Err(LoanError::WrongLoan);
                }
            }
        }
        Ok(())
    }
    /// Authenticate the immutable witness and unique reservation identities without weak upgrade.
    pub(super) fn authenticate_core(
        &self,
        core: &HostCore,
        account: &ExecutionAccount,
    ) -> Result<(), LoanError> {
        account.authenticate(core.identity.account)?;
        if let Some(law) = &core.retained_law {
            law.validate_current(self)?;
        }
        self.authenticate_identity(core.identity)?;
        let State::Out(witness) = &self.loans.group(core.identity)?.state else {
            return Err(LoanError::DuplicateReturn);
        };
        if !witness.ptr_eq(&Rc::downgrade(&core.witness))
            || core.witness.identity != core.identity
            || core.witness.receipt != core.cleanup.identity
        {
            return Err(LoanError::WrongLoan);
        }
        for token in core.headers.iter().chain(
            core.channels
                .iter()
                .flatten()
                .flat_map(|channel| channel.tokens.iter()),
        ) {
            if !self.loans.authentic_token(token, core.identity) {
                return Err(LoanError::WrongLoan);
            }
        }
        Ok(())
    }
    /// Derive initialized coverage, validate selected payload and publish the entire bundle.
    pub fn handback<P: Scalar + 'static>(
        &mut self,
        loan: OwnedLoan<P>,
        account: &mut ExecutionAccount,
    ) -> HandbackOutcome<P> {
        let authenticated = account
            .ledger
            .with_work_phase(super::FAMILY, "return authentication", |budget| {
                // Request authentication, two group lookups, each channel/debit row and witness.
                charge(
                    budget,
                    9 + 4 * loan.core.channels.iter().flatten().count()
                        + super::retained_expm1::Attachment::WORK
                            * usize::from(loan.core.retained_law.is_some()),
                )
            })
            .and_then(|()| self.authenticate_core(&loan.core, account));
        if let Err(error) = authenticated {
            return HandbackOutcome::ReturnRejected(error, loan);
        }
        publication::handback(self, loan, account)
    }
    /// Cancel using the admission's prepaid authentication and terminal transition.
    pub fn cancel_loan<P: Scalar + 'static>(
        &mut self,
        loan: OwnedLoan<P>,
        account: &mut ExecutionAccount,
    ) -> CancelOutcome<P> {
        if let Err(error) = self.authenticate_core(&loan.core, account) {
            let charged =
                account
                    .ledger
                    .with_work_phase(super::FAMILY, "rejected cancellation", |budget| {
                        charge(budget, 1)
                    });
            return CancelOutcome::Rejected(charged.err().unwrap_or(error), loan);
        }
        match account
            .ledger
            .with_work_phase(super::FAMILY, "prepaid cancellation", |_| {
                self.cancel_authenticated(loan)
            }) {
            Ok(receipt) => CancelOutcome::Cancelled(receipt),
            Err((error, loan)) => CancelOutcome::Rejected(error, loan),
        }
    }
    /// All authentication and fixed terminal cleanup were paid before the owner escaped.
    fn cancel_authenticated<P: Scalar + 'static>(
        &mut self,
        loan: OwnedLoan<P>,
    ) -> Result<CancellationReceipt, (LoanError, OwnedLoan<P>)> {
        let identity = loan.core.identity;
        let revision = match self.loans.terminal_revision() {
            Ok(revision) => revision,
            // Admission reserves this successor; refusal is unreachable while that invariant holds.
            Err(error) => return Err((error, loan)),
        };
        let receipt = CancellationReceipt {
            ticket: loan.ticket(),
            reason: super::AbsenceReason::Cancelled,
        };
        if let Some(group) = self
            .loans
            .groups
            .get_mut(identity.root.slot)
            .and_then(Option::as_mut)
        {
            group.state = State::Retiring;
            group.outcome = super::ProducerOutcome::Absent(receipt);
            group.delivered = true;
        }
        drop(loan);
        self.release_owner_debits(identity);
        self.retire_native_reservations(identity);
        // Cancellation uses its stored terminal entitlement and cannot require new Work.
        self.loans.revision = revision;
        self.loans.pending -= 1;
        Ok(receipt)
    }
    /// Destroy conservative allocation records only after their owner has been destroyed.
    pub(super) fn release_owner_debits(&mut self, identity: Identity) {
        if let Some(group) = self
            .loans
            .groups
            .get(identity.root.slot)
            .and_then(Option::as_ref)
        {
            let keys = group.debits;
            for (slot, key) in keys.entries().iter().copied().flatten() {
                self.loans.release(slot, key);
            }
        }
    }
    /// Private native reservations have no external readers and need no allocating retirement plan.
    pub(super) fn retire_native_reservations(&mut self, identity: Identity) {
        if let Some(group) = self
            .loans
            .groups
            .get(identity.root.slot)
            .and_then(Option::as_ref)
        {
            let pin = group.pin;
            if self
                .loans
                .pins
                .get(pin)
                .and_then(Option::as_ref)
                .is_some_and(|entry| entry.witness.is_none())
            {
                self.loans.pins[pin] = None;
            }
        }
        drop(self.masks.layouts.remove(identity.layout.key));
        drop(self.masks.masks.remove(identity.mask.key));
    }
}
/// Derive complete channel masks directly from actual component flags.
pub(super) fn complete(
    core: &mut HostCore,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    let cells = core.declaration.extent.rows();
    let components = core.declaration.extent.columns();
    for channel in core.channels.iter_mut().flatten() {
        charge(
            budget,
            core.declaration
                .extent
                .entries()
                .checked_add(channel.complete.len())
                .ok_or(LoanError::CountOverflow)?,
        )?;
        channel.complete.fill(0);
        for cell in 0..cells {
            let initialized = (0..components).fold(true, |initialized, component| {
                bit(&channel.components, cell + cells * component) & initialized
            });
            set(&mut channel.complete, cell, initialized);
        }
    }
    Ok(())
}
