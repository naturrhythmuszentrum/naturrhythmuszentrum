// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed producer outcomes and per-occupation debits outlive detached capabilities.

use super::{HostChannel, LoanError, payload::PublishedChannel};
use crate::{CellLayoutVersion, MaskVersion, heap::identity::Key, storage::Format};
use std::rc::{Rc, Weak};

/// Copyable diagnostic locator; it conveys no ownership or numerical retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoanTicket {
    /// Exact sealed identity, including grouped occupations.
    pub(super) identity: Identity,
}
/// Complete authority authenticated before inspecting a returned payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Identity {
    /// Owning heap scope.
    pub(super) scope: u64,
    /// Original ledger identity.
    pub(super) account: u64,
    /// Root occupation shared by the group record.
    pub(super) root: Key,
    /// Every present channel's occupied descriptor.
    pub(super) channels: [Option<Key>; 3],
    /// Exact prospective native layout.
    pub(super) layout: CellLayoutVersion,
    /// Exact prospective native filled mask.
    pub(super) mask: MaskVersion,
    /// Nonwrapping request identity.
    pub(super) request: u64,
    /// Nonwrapping nonce.
    pub(super) nonce: u64,
    /// Original declaration and absence scope.
    pub(super) declaration: u64,
    /// Unique prepaid cleanup receipt.
    pub(super) receipt: u64,
    /// Sealed precision.
    pub(super) format: Format,
}
/// A selected filled role is constructed only by complete authenticated publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualifiedMaskVersion {
    /// Exact complete owner identities and target group.
    pub(super) identity: Identity,
    /// Channel whose initialized coverage this attachment authenticates.
    pub(super) channel: HostChannel,
}
impl QualifiedMaskVersion {
    /// Ordinary native selection metadata; this locator grants no numerical read.
    #[must_use]
    pub const fn native(self) -> MaskVersion {
        self.identity.mask
    }
    /// Channel whose identity is qualified by this filled role.
    #[must_use]
    pub const fn channel(self) -> HostChannel {
        self.channel
    }
}
/// Fixed terminal absence reason, independent of floating payload validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbsenceReason {
    /// The rightful owner explicitly cancelled its contribution.
    Cancelled,
    /// The sole detached witness expired without a handback.
    Abandoned,
}
/// Publication evidence contains only locators and no numerical pin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationReceipt {
    /// Completed producer.
    pub(super) ticket: LoanTicket,
    /// Exact initialized filled role for every present channel.
    pub(super) present: [bool; 3],
}
impl PublicationReceipt {
    /// Completed producer locator, which cannot reconstruct the owning capability.
    #[must_use]
    pub const fn ticket(self) -> LoanTicket {
        self.ticket
    }
    /// Filled attachment for a declared channel.
    #[must_use]
    pub const fn filled(self, channel: HostChannel) -> Option<QualifiedMaskVersion> {
        if self.present[channel.index()] {
            Some(QualifiedMaskVersion {
                identity: self.ticket.identity,
                channel,
            })
        } else {
            None
        }
    }
}
/// Explicit local absence evidence has no numerical retaining edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationReceipt {
    /// Terminal producer.
    pub(super) ticket: LoanTicket,
    /// Deliberate cancellation or observed abandonment.
    pub(super) reason: AbsenceReason,
}
impl CancellationReceipt {
    /// Terminal producer identity.
    #[must_use]
    pub const fn ticket(self) -> LoanTicket {
        self.ticket
    }
    /// Why this contribution is absent.
    #[must_use]
    pub const fn reason(self) -> AbsenceReason {
        self.reason
    }
}
/// Local producer delivery never claims a graph-level flush.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducerOutcome {
    /// Ownership is still pending.
    Pending,
    /// Complete publication and explicit absence outside its filled mask.
    Published(PublicationReceipt),
    /// Entire declared contribution is absent.
    Absent(CancellationReceipt),
    /// The fixed terminal receipt has already been delivered once.
    AlreadyDelivered,
}
/// One occupied channel is unavailable to every legacy dense/view route.
#[derive(Debug)]
pub(crate) struct ManagedDescriptor {
    /// Complete group authority.
    pub(super) identity: Identity,
    /// Channel and exact companion target are fixed by this group.
    pub(super) channel: HostChannel,
    /// Filled attachment exists only after no-fail publication.
    pub(super) filled: bool,
}
/// Unique reservation authority follows the allocation through its ownership moves.
#[derive(Debug)]
pub(super) struct Token {
    /// Stable position in the bounded debit index.
    pub(super) slot: usize,
    /// Debit record identity.
    pub(super) key: u64,
    /// Original allocation cleanup entitlement.
    pub(super) receipt: u64,
}
/// Allocation classes keep residence observations separate from inline owner headers.
#[derive(Clone, Copy, Debug)]
pub(super) enum AllocationKind {
    /// Rounded numerical slab, including unused admitted capacity.
    Numerical,
    /// Component and complete-cell initialization words.
    Tracking,
    /// Detached core and immutable shared owner headers.
    Owner,
    /// Each independently admitted public pin header.
    Pin,
    /// One shared filled-word owner and its aligned shell.
    Filled,
}
/// The accounting record owns no payload and never duplicates its charge.
#[derive(Debug)]
pub(super) struct Debit {
    /// Fixed allocation role, independent of whether its owner is detached or published.
    pub(super) kind: AllocationKind,
    /// Exact occupation funded by this record.
    pub(super) owner: Identity,
    /// Allocation identity.
    pub(super) key: u64,
    /// Addressable scalar count; padding belongs only to bytes.
    pub(super) elements: usize,
    /// Full observable allocation capacity.
    pub(super) bytes: usize,
}
/// Fixed prepaid cleanup counts; no mutable secondary Work budget exists.
#[derive(Debug)]
pub(super) struct CleanupReceipt {
    /// Original nonwrapping receipt identity.
    pub(super) identity: u64,
    /// Allocation destruction entitlements.
    pub(super) allocations: usize,
    /// Index and producer closure entitlements.
    pub(super) records: usize,
    /// Retaining-edge destruction entitlements.
    pub(super) edges: usize,
}
impl CleanupReceipt {
    /// Identity-preserving placeholder owns no entitlements while the original receipt moves.
    pub(super) const fn empty(identity: u64) -> Self {
        Self {
            identity,
            allocations: 0,
            records: 0,
            edges: 0,
        }
    }
}

/// Expiration is observed only after all preceding core fields have been destroyed.
#[derive(Debug)]
pub(super) struct Liveness {
    /// Sealed group identity.
    pub(super) identity: Identity,
    /// Same receipt identity through every move and repair.
    pub(super) receipt: u64,
}
#[cfg(test)]
impl Drop for Liveness {
    fn drop(&mut self) {
        super::fault::witness_expiring(self.receipt);
    }
}
/// Complete immutable bundle retained by its one registry root and explicit pins.
#[derive(Debug)]
pub(super) struct BundleOwner {
    /// Empty shell becomes complete before any edge or weak reference escapes.
    pub(super) payload: Option<Bundle>,
}
/// The entire mutually dependent channel group has no strong backedges.
#[derive(Debug)]
pub(super) struct Bundle {
    /// Producer origin and sealed completion evidence, without a source retaining edge.
    pub(super) origin: super::application::Origin,
    /// Exact original declaration.
    pub(super) declaration: Declaration,
    /// Exact occupations and request.
    pub(super) identity: Identity,
    /// Moved separately allocated native channels.
    pub(super) channels: [Option<PublishedChannel>; 3],
    /// One native shared identity layout.
    pub(super) layout: Rc<crate::mask::storage::LayoutRecord>,
    /// One native shared filled-word allocation.
    pub(super) mask: Rc<crate::mask::storage::MaskRecord>,
    /// Distinct qualified attachments share the same immutable words.
    pub(super) qualified: [Option<QualifiedMaskVersion>; 3],
    /// Original cleanup record follows the owners.
    pub(super) cleanup: CleanupReceipt,
    /// Moved native-layout, bundle-header and mask allocation entitlements.
    pub(super) retained: [Token; 3],
}
/// Precision-neutral retained declaration; its public constructor remains sealed and typed.
#[derive(Debug)]
pub(super) struct Declaration {
    /// Original complete independent binding witnesses.
    pub(super) symbols: nrz_bindings::BindingSymbols,
    /// Exact original cell association.
    pub(super) axis: crate::CellAxis,
    /// Literal cells by components.
    pub(super) extent: crate::Extent,
    /// Explicit physical or represented assumption.
    pub(super) coverage: crate::Coverage,
}
/// Each public pin owns exactly one private record with prepaid constant-time drop.
#[derive(Debug)]
pub(super) struct PinRecord {
    /// Empty publication shell is filled before exposure.
    pub(super) payload: Option<PinPayload>,
}
/// Complete pin identity and its single numerical use edge.
#[derive(Debug)]
pub(super) struct PinPayload {
    /// Nonwrapping pin identity.
    pub(super) identity: u64,
    /// This pin's continuation account.
    pub(super) account: u64,
    /// Exact separately reserved weak-index position.
    pub(super) index: usize,
    /// Complete immutable bundle use edge.
    pub(super) bundle: Rc<BundleOwner>,
    /// Unique conservative header debit.
    pub(super) token: Token,
}
/// The index owns no numerical edge and never upgrades a weak reference.
#[derive(Debug)]
pub(super) struct PinIndex {
    /// Exact pin identity.
    pub(super) identity: u64,
    /// Expiration witness.
    pub(super) witness: Option<Weak<PinRecord>>,
    /// Header remains charged until this weak reference is removed.
    pub(super) debit: (usize, u64),
}
/// Fixed producer and terminal entitlement stay in the root descriptor's parallel row.
#[derive(Debug)]
pub(super) struct Group {
    /// Full authority, checked again before abandonment.
    pub(super) identity: Identity,
    /// Current ownership transition.
    pub(super) state: State,
    /// Fixed outcome; no growing replay history.
    pub(super) outcome: ProducerOutcome,
    /// Terminal evidence has been delivered.
    pub(super) delivered: bool,
    /// Initial pin-index occupation reserved before the detached core escapes.
    pub(super) pin: usize,
    /// Fixed cancellation inventory; each locator authenticates its exact allocation identity.
    pub(super) debits: DebitInventory,
}
/// Bounded cancellation inventories preserve the host's fifteen-entry representation.
#[derive(Clone, Copy, Debug)]
pub(super) enum DebitInventory {
    /// Host admission and its later filled owner.
    Host([Option<(usize, u64)>; 15]),
    /// Application admission includes its filled words, outcomes and per-cell extent.
    Application([Option<(usize, u64)>; 17]),
}
impl DebitInventory {
    /// Exact active variant, without inventing extra host debit positions.
    pub(super) const fn entries(&self) -> &[Option<(usize, u64)>] {
        match self {
            Self::Host(entries) => entries,
            Self::Application(entries) => entries,
        }
    }
    /// Mutate only the fixed inventory selected at admission.
    pub(super) const fn entries_mut(&mut self) -> &mut [Option<(usize, u64)>] {
        match self {
            Self::Host(entries) => entries,
            Self::Application(entries) => entries,
        }
    }
}
/// Managed allocation states are independent of legacy arena occupations.
#[derive(Debug)]
pub(super) enum State {
    /// Privately installed before ownership can escape.
    Reserved,
    /// Sole writable owner is outside the heap.
    Out(Weak<Liveness>),
    /// Exactly one immutable bundle registry root.
    Published(Rc<BundleOwner>),
    /// Payload is destroyed; fixed local evidence can still require delivery.
    Retiring,
}
/// Bounded per-occupation ownership, debit and weak-pin indexes.
#[derive(Debug)]
pub(crate) struct Registry {
    /// Parallel group slots indexed by the root descriptor position.
    pub(super) groups: Vec<Option<Group>>,
    /// Non-owning conservative per-allocation capacity records.
    pub(super) debits: Vec<Option<Debit>>,
    /// Non-owning pin expiration records.
    pub(super) pins: Vec<Option<PinIndex>>,
    /// Any visible ownership mutation invalidates outstanding scan cursors.
    pub(super) revision: u64,
    /// Reserved terminal successors; ordinary mutations cannot spend this headroom.
    pub(super) pending: u64,
}
impl Registry {
    /// Empty managed tables own no payload allocations.
    pub(crate) const fn new() -> Self {
        Self {
            groups: Vec::new(),
            debits: Vec::new(),
            pins: Vec::new(),
            revision: 1,
            pending: 0,
        }
    }
    /// Elements and bytes each enumerate every debit slot, including vacancies.
    pub(crate) const fn debit_work(&self) -> usize {
        self.debits.len() * 2
    }
    /// Every out debit counts in unrelated legacy admission.
    pub(crate) fn elements(&self) -> Result<[usize; 2], crate::HeapError> {
        let mut elements = [0_usize; 2];
        for debit in self.debits.iter().flatten() {
            let index = debit.owner.format.index();
            elements[index] = elements[index]
                .checked_add(debit.elements)
                .ok_or(crate::HeapError::Capacity)?;
        }
        Ok(elements)
    }
    /// Actual complete table capacities and conservatively retained allocation debits.
    pub(crate) fn owned_bytes(&self) -> Result<usize, crate::HeapError> {
        let add = crate::heap::resources::add;
        let mut bytes = add(
            self.groups
                .capacity()
                .checked_mul(size_of::<Option<Group>>())
                .ok_or(crate::HeapError::Capacity)?,
            self.debits
                .capacity()
                .checked_mul(size_of::<Option<Debit>>())
                .ok_or(crate::HeapError::Capacity)?,
        )?;
        bytes = add(
            bytes,
            self.pins
                .capacity()
                .checked_mul(size_of::<Option<PinIndex>>())
                .ok_or(crate::HeapError::Capacity)?,
        )?;
        for debit in self.debits.iter().flatten() {
            bytes = add(bytes, debit.bytes)?;
        }
        Ok(bytes)
    }
    /// Exact live root identity; stale tickets cannot inspect a reused occupant.
    pub(super) fn group(&self, identity: Identity) -> Result<&Group, LoanError> {
        let group = self
            .groups
            .get(identity.root.slot)
            .and_then(Option::as_ref)
            .ok_or(LoanError::StaleOccupation)?;
        if group.identity.root != identity.root {
            return Err(LoanError::StaleOccupation);
        }
        if group.identity != identity {
            return Err(LoanError::WrongLoan);
        }
        Ok(group)
    }
    /// Precompute a nonwrapping registry revision before any mutation.
    pub(super) fn next_revision(&self) -> Result<u64, LoanError> {
        self.revision
            .checked_add(self.pending)
            .and_then(|revision| revision.checked_add(1))
            .ok_or(LoanError::RevisionCapacity)?;
        self.revision
            .checked_add(1)
            .ok_or(LoanError::RevisionCapacity)
    }
    /// Admission also reserves the new producer's eventual terminal successor.
    pub(super) fn admission_revision(&self) -> Result<u64, LoanError> {
        self.revision
            .checked_add(self.pending)
            .and_then(|revision| revision.checked_add(2))
            .ok_or(LoanError::RevisionCapacity)?;
        self.next_revision()
    }
    /// The resumable reconciliation algorithm reserves one successor per pending producer.
    /// Ordinary mutations cannot spend that headroom; a terminal transition consumes it once.
    pub(super) fn terminal_revision(&self) -> Result<u64, LoanError> {
        self.revision
            .checked_add(1)
            .ok_or(LoanError::RevisionCapacity)
    }
    /// Select a vacant weak-pin row without retaining or upgrading any owner.
    pub(super) fn vacant_pin(&self) -> usize {
        self.pins
            .iter()
            .enumerate()
            .fold(self.pins.len(), |first, (index, entry)| {
                if entry.is_none() {
                    first.min(index)
                } else {
                    first
                }
            })
    }
    /// Install one preallocated weak-pin row, including an occupied private reservation.
    pub(super) fn install_pin(&mut self, index: usize, pin: PinIndex) {
        while self.pins.len() <= index {
            self.pins.push(None);
        }
        self.pins[index] = Some(pin);
    }
    /// Plan fixed per-occupation debit positions, reusing vacant entries before growing.
    pub(super) fn vacant_debits(&self, count: usize) -> Result<[usize; 15], LoanError> {
        self.vacant_inventory(count)
    }
    /// Application admission selects a separately bounded seventeen-row inventory.
    pub(super) fn vacant_inventory<const N: usize>(
        &self,
        count: usize,
    ) -> Result<[usize; N], LoanError> {
        if count > N {
            return Err(LoanError::CountOverflow);
        }
        let mut slots = [0; N];
        let mut found = 0;
        for (slot, entry) in self.debits.iter().enumerate() {
            if entry.is_none() && found < count {
                slots[found] = slot;
                found += 1;
            }
        }
        let mut next = self.debits.len();
        while found < count {
            slots[found] = next;
            found += 1;
            next = next.checked_add(1).ok_or(LoanError::CountOverflow)?;
        }
        Ok(slots)
    }
    /// Install a preallocated exact debit row without searching another owner's records.
    pub(super) fn install(&mut self, slot: usize, debit: Debit) {
        while self.debits.len() <= slot {
            self.debits.push(None);
        }
        self.debits[slot] = Some(debit);
    }
    /// Remove one authenticated allocation's debit after its actual destruction.
    pub(super) fn release(&mut self, slot: usize, key: u64) {
        if let Some(entry) = self.debits.get_mut(slot)
            && entry.as_ref().is_some_and(|debit| debit.key == key)
        {
            *entry = None;
        }
    }
    /// Validate the original receipt and exact allocation index without a global scan.
    pub(super) fn authentic_token(&self, token: &Token, identity: Identity) -> bool {
        token.receipt == identity.receipt
            && self
                .debits
                .get(token.slot)
                .and_then(Option::as_ref)
                .is_some_and(|debit| {
                    let same_allocation = debit.key == token.key;
                    let same_owner = debit.owner == identity;
                    same_allocation && same_owner
                })
    }
}
