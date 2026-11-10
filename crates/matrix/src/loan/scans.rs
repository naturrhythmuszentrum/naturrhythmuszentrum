// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded serialized scans preserve conservative debits and local delivery obligations.

use super::{
    ExecutionAccount, LoanError, charge,
    registry::{AbsenceReason, CancellationReceipt, LoanTicket, ProducerOutcome, State},
};
use crate::Heap;
use std::rc::Rc;

/// Borrow-free cursor; stale revisions restart and charge repeated visits.
#[derive(Clone, Copy, Debug)]
pub struct ScanCursor {
    /// Exact heap scope.
    scope: u64,
    /// Registry revision after the last completed step.
    revision: u64,
    /// Reconciliation precedes root reclamation.
    reclaim: bool,
    /// Next unvisited combined slot.
    next: usize,
    /// Stable endpoint of this pass.
    end: usize,
    /// A root removal requires another oldest-first pass.
    changed: bool,
}
/// Bounded traversal distinguishes completion from stopped or pending work.
#[derive(Debug)]
pub enum ScanOutcome {
    /// This pass is complete at one unchanged revision.
    Complete,
    /// More slots or a further reclaim pass remain.
    Pending(ScanCursor),
    /// Work stopped before the cursor's next slot was inspected.
    WorkExhausted(ScanCursor),
    /// Authentication or nonwrapping revision admission refused.
    Refused(LoanError),
}
/// Occupied owner residence is separate from reusable empty table capacity.
#[derive(Debug)]
pub enum Residence {
    /// Complete scans proved that no numerical owner remains.
    Empty {
        /// Retained empty table capacity and immutable execution-history metadata.
        retained_bytes: usize,
    },
    /// A borrower, pin, terminal delivery or unfinished traversal prevents completion.
    Pending {
        /// Managed and legacy scalar elements, absent when inventory Work was unavailable.
        elements: Option<[usize; 2]>,
        /// Complete allocation bytes, absent when inventory Work was unavailable.
        bytes: Option<usize>,
    },
}
impl Heap {
    /// Reconcile expired detached witnesses and weak pin records without upgrading them.
    pub fn reconcile_detached(
        &mut self,
        account: &mut ExecutionAccount,
        slot_allowance: usize,
        cursor: Option<ScanCursor>,
    ) -> ScanOutcome {
        self.scan(account, slot_allowance, cursor, false)
    }
    /// Reclaim registry-only bundles after every reader and evidence obligation releases.
    pub fn reclaim(
        &mut self,
        account: &mut ExecutionAccount,
        slot_allowance: usize,
        cursor: Option<ScanCursor>,
    ) -> ScanOutcome {
        self.scan(account, slot_allowance, cursor, true)
    }
    /// Deliver one reserved terminal outcome without allocating a growing replay log.
    /// # Errors
    /// Refuses foreign, stale or wrong accounts and charges the original record access.
    pub fn take_producer_outcome(
        &mut self,
        ticket: LoanTicket,
        account: &mut ExecutionAccount,
    ) -> Result<ProducerOutcome, LoanError> {
        if ticket.identity.scope != self.scope {
            return Err(LoanError::ForeignOwner);
        }
        account.authenticate(ticket.identity.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "producer delivery", |budget| {
                charge(budget, 1)
            })?;
        let group = self.loans.group(ticket.identity)?;
        if group.delivered {
            return Ok(ProducerOutcome::AlreadyDelivered);
        }
        let outcome = group.outcome;
        if matches!(outcome, ProducerOutcome::Pending) {
            return Ok(outcome);
        }
        let revision = self.loans.next_revision()?;
        if let Some(group) = self
            .loans
            .groups
            .get_mut(ticket.identity.root.slot)
            .and_then(Option::as_mut)
        {
            group.delivered = true;
        }
        self.loans.revision = revision;
        Ok(outcome)
    }
    /// Reconciliation and reclamation must finish before zero managed residence is reported.
    /// # Errors
    /// Refuses byte overflow or a scan identity failure; Work-stopped scans remain pending.
    pub fn residence(
        &mut self,
        account: &mut ExecutionAccount,
        slot_allowance: usize,
    ) -> Result<Residence, LoanError> {
        let reconciled = self.reconcile_detached(account, slot_allowance, None);
        let reclaimed = if matches!(reconciled, ScanOutcome::Complete) {
            self.reclaim(account, slot_allowance, None)
        } else {
            ScanOutcome::Pending(self.cursor(false))
        };
        let complete = matches!(reconciled, ScanOutcome::Complete)
            && matches!(reclaimed, ScanOutcome::Complete);
        if let ScanOutcome::Refused(error) = reconciled {
            return Err(error);
        }
        if let ScanOutcome::Refused(error) = reclaimed {
            return Err(error);
        }
        if !complete {
            return Ok(Residence::Pending {
                elements: None,
                bytes: None,
            });
        }
        self.resident_inventory(account)
    }
    /// Inventory traversals begin only after their full slot and owner cost is paid.
    fn resident_inventory(&self, account: &mut ExecutionAccount) -> Result<Residence, LoanError> {
        let inventory_work = [
            self.resource_work()?,
            self.storage.scan_work(),
            self.masks.layouts.scan_work(),
            self.masks.masks.scan_work(),
            self.executions.trace_work(),
            self.loans.groups.len(),
            self.loans.pins.len(),
            self.loans.debits.len(),
        ]
        .into_iter()
        .try_fold(0usize, |sum, work| {
            sum.checked_add(work).ok_or(LoanError::CountOverflow)
        })?;
        if account
            .ledger
            .with_work_phase(super::FAMILY, "residence inventory", |budget| {
                charge(budget, inventory_work)
            })
            .is_err()
        {
            return Ok(Residence::Pending {
                elements: None,
                bytes: None,
            });
        }
        let occupied = self
            .loans
            .groups
            .iter()
            .fold(false, |found, group| group.is_some() | found)
            | self
                .loans
                .pins
                .iter()
                .fold(false, |found, pin| pin.is_some() | found)
            | self
                .loans
                .debits
                .iter()
                .fold(false, |found, debit| debit.is_some() | found)
            | self.storage.has_owners()
            | self.executions.has_payload()
            | (self.masks.layouts.iter().count() != 0)
            | (self.masks.masks.iter().count() != 0);
        let managed = self.loans.elements()?;
        let legacy = self.storage.elements();
        let elements = [
            crate::heap::resources::add(managed[0], legacy[0])?,
            crate::heap::resources::add(managed[1], legacy[1])?,
        ];
        let bytes = self.resource_bytes()?;
        if occupied {
            Ok(Residence::Pending {
                elements: Some(elements),
                bytes: Some(bytes),
            })
        } else {
            Ok(Residence::Empty {
                retained_bytes: bytes,
            })
        }
    }
    /// Authenticate every moved allocation entitlement before the root can be destroyed.
    pub(super) fn validate_published_bundle(
        &self,
        bundle: &super::registry::Bundle,
    ) -> Result<(), LoanError> {
        if let Some(law) = &bundle.retained_law {
            law.validate_current(self)?;
        }
        let receipt_matches = bundle.cleanup.identity == bundle.identity.receipt;
        if !receipt_matches {
            return Err(LoanError::WrongLoan);
        }
        for token in bundle.retained.iter().chain(
            bundle
                .channels
                .iter()
                .flatten()
                .map(|channel| &channel.token)
                .chain(
                    bundle
                        .endpoints
                        .iter()
                        .flatten()
                        .map(|channel| &channel.token),
                ),
        ) {
            if !self.loans.authentic_token(token, bundle.identity) {
                return Err(LoanError::WrongLoan);
            }
        }
        if let Some(evidence) = &bundle.origin.evidence
            && !self.loans.authentic_token(&evidence.token, bundle.identity)
        {
            return Err(LoanError::WrongLoan);
        }
        let layout = self
            .masks
            .layouts
            .get(bundle.identity.layout.key)?
            .owner()?;
        let mask = self.masks.masks.get(bundle.identity.mask.key)?.owner()?;
        if !Rc::ptr_eq(layout, &bundle.layout) || !Rc::ptr_eq(mask, &bundle.mask) {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(())
    }
    /// Capture a pass endpoint without retaining any owner or heap borrow.
    const fn cursor(&self, reclaim: bool) -> ScanCursor {
        ScanCursor {
            scope: self.scope,
            revision: self.loans.revision,
            reclaim,
            next: 0,
            end: self.loans.groups.len()
                + if reclaim {
                    0
                } else {
                    self.loans.pins.len() + self.loans.debits.len()
                },
            changed: false,
        }
    }
    /// Charge a slot before inspection, preserving the exact stopped position on refusal.
    fn scan(
        &mut self,
        account: &mut ExecutionAccount,
        allowance: usize,
        cursor: Option<ScanCursor>,
        reclaim: bool,
    ) -> ScanOutcome {
        let mut cursor = cursor
            .filter(|cursor| {
                cursor.scope == self.scope
                    && cursor.revision == self.loans.revision
                    && cursor.reclaim == reclaim
            })
            .unwrap_or_else(|| self.cursor(reclaim));
        let result = account.ledger.with_work_phase(
            super::FAMILY,
            if reclaim {
                "registry reclaim"
            } else {
                "detached reconciliation"
            },
            |budget| {
                for _ in 0..allowance {
                    if cursor.next == cursor.end {
                        if reclaim && cursor.changed {
                            cursor = self.cursor(true);
                        } else {
                            return Ok(true);
                        }
                    }
                    charge(budget, 1)?;
                    if reclaim {
                        self.reclaim_slot(cursor.next, budget, &mut cursor.changed)?;
                    } else {
                        self.reconcile_slot(cursor.next, budget)?;
                    }
                    cursor.next += 1;
                    cursor.revision = self.loans.revision;
                }
                Ok(cursor.next == cursor.end && !cursor.changed)
            },
        );
        match result {
            Ok(true) => ScanOutcome::Complete,
            Ok(false) => ScanOutcome::Pending(cursor),
            Err(LoanError::Budget(_)) => ScanOutcome::WorkExhausted(cursor),
            Err(error) => ScanOutcome::Refused(error),
        }
    }
    /// Expired detached payloads are already destroyed, but remain charged until this transition.
    fn reconcile_slot(
        &mut self,
        slot: usize,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        let prepared_start = self.loans.groups.len() + self.loans.pins.len();
        if slot >= prepared_start {
            return self.reconcile_prepared(slot - prepared_start, budget);
        }
        if slot >= self.loans.groups.len() {
            let index = slot - self.loans.groups.len();
            if let Some(pin) = self.loans.pins.get(index).and_then(Option::as_ref)
                && pin
                    .witness
                    .as_ref()
                    .is_some_and(|witness| witness.strong_count() == 0)
            {
                let revision = self.loans.next_revision()?;
                charge(budget, 1)?;
                if pin.identity != pin.debit.1 {
                    return Err(LoanError::WrongLoan);
                }
                let key = pin.debit;
                self.loans.pins[index] = None;
                self.loans.release(key.0, key.1);
                self.loans.revision = revision;
            }
            return Ok(());
        }
        let Some(group) = self.loans.groups.get(slot).and_then(Option::as_ref) else {
            return Ok(());
        };
        let State::Out(witness) = &group.state else {
            return Ok(());
        };
        if witness.strong_count() != 0 {
            return Ok(());
        }
        let identity = group.identity;
        charge(budget, 1 + identity.channels.iter().flatten().count())?;
        self.authenticate_identity(identity)?;
        let revision = self.loans.terminal_revision()?;
        if let Some(group) = self.loans.groups[slot].as_mut() {
            group.state = State::Retiring;
            group.outcome = ProducerOutcome::Absent(CancellationReceipt {
                ticket: LoanTicket { identity },
                reason: AbsenceReason::Abandoned,
            });
        }
        self.release_owner_debits(identity);
        self.retire_native_reservations(identity);
        self.loans.revision = revision;
        self.loans.pending -= 1;
        Ok(())
    }
    /// A law owner and each expired use are distinct authenticated removal events.
    fn reconcile_prepared(
        &mut self,
        slot: usize,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<(), LoanError> {
        use super::registry::{AllocationKind, DebitOwner};
        let Some(debit) = self.loans.debits.get(slot).and_then(Option::as_ref) else {
            return Ok(());
        };
        let expired = match &debit.kind {
            AllocationKind::PreparedOwner { witness, .. } => witness.strong_count() == 0,
            AllocationKind::PreparedUse(witness) => witness.strong_count() == 0,
            AllocationKind::Numerical
            | AllocationKind::Tracking
            | AllocationKind::Owner
            | AllocationKind::Pin
            | AllocationKind::Filled => return Ok(()),
        };
        let DebitOwner::Prepared(identity) = debit.owner else {
            return Err(LoanError::WrongLoan);
        };
        if identity.scope != self.scope || identity.owner == 0 || identity.account == 0 {
            return Err(LoanError::WrongLoan);
        }
        if matches!(debit.kind, AllocationKind::PreparedOwner { .. }) && debit.key != identity.owner
        {
            return Err(LoanError::WrongLoan);
        }
        if expired {
            let revision = self.loans.next_revision()?;
            charge(budget, 1)?;
            let key = debit.key;
            self.loans.release(slot, key);
            self.loans.revision = revision;
        }
        Ok(())
    }
    /// Prepay outgoing/index traversal before removing a protected root or any related record.
    fn reclaim_slot(
        &mut self,
        slot: usize,
        budget: &mut nrz_bindings::Budget,
        changed: &mut bool,
    ) -> Result<(), LoanError> {
        let Some(group) = self.loans.groups.get(slot).and_then(Option::as_ref) else {
            return Ok(());
        };
        if !group.delivered {
            return Ok(());
        }
        let eligible = match &group.state {
            State::Retiring => true,
            State::Published(owner) => Rc::strong_count(owner) == 1,
            State::Out(_) | State::Reserved => false,
        };
        if !eligible {
            return Ok(());
        }
        let identity = group.identity;
        charge(budget, 1)?;
        let (key, descriptor) = self
            .descriptors
            .slots
            .at(slot)
            .ok_or(LoanError::StaleOccupation)?;
        let correct_root = key == identity.root && descriptor.is_some();
        if !correct_root {
            return Err(LoanError::StaleOccupation);
        }
        if let State::Published(owner) = &group.state {
            let bundle = owner.payload.as_ref().ok_or(LoanError::WrongLoan)?;
            charge(
                budget,
                identity.channels.iter().flatten().count()
                    + 2 * usize::from(bundle.endpoints.is_some())
                    + 5
                    + usize::from(bundle.origin.evidence.is_some())
                    + super::retained_expm1::Attachment::WORK
                        * usize::from(bundle.retained_law.is_some()),
            )?;
            self.validate_published_bundle(bundle)?;
        }
        let revision = self.loans.next_revision()?;
        let edges = identity.channels.into_iter().flatten().count() + 4;
        charge(budget, edges)?;
        let debits = group.debits;
        self.loans.groups[slot] = None;
        self.retire_native_reservations(identity);
        for (position, key) in debits.entries().iter().copied().flatten() {
            self.loans.release(position, key);
        }
        for key in identity.channels.into_iter().flatten() {
            drop(self.descriptors.slots.remove(key));
        }
        self.loans.revision = revision;
        *changed = true;
        Ok(())
    }
}
