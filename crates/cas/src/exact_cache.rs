// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-held exact references to graph-owned LC judgments.

use std::sync::Arc;

use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, Handle, size};
use crate::{Fault, InvalidCode, Location};

/// Location reported for local index failures.
const AT: Location = Location::new("exact LC judgment index");

/// The two currently admitted checked graph kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    /// Checked LC root judgment.
    Root,
    /// Checked LC source substitution judgment.
    Substitution,
}

/// One complete local judgment and proof reference; result payload stays in the graph.
#[derive(Clone, Debug)]
struct Entry {
    /// Registered certificate kind.
    kind: Kind,
    /// Complete original statement key.
    statement: Vec<u8>,
    /// Graph-owned proof transcript reference.
    proof: Arc<[u8]>,
    /// Particular root proof dependency, if any.
    root: Option<Handle>,
    /// Authenticated graph judgment.
    judgment: Handle,
    /// Stable admission serial.
    serial: u64,
}

/// Speculative new entry, discarded automatically if graph publication refuses.
#[derive(Debug)]
pub(crate) struct Proposal {
    /// Complete statement copied for admission.
    statement: Vec<u8>,
    /// Accounted new local entry bytes.
    bytes: u64,
}

impl Proposal {
    /// New statement and entry slot only.
    pub(crate) const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

/// One evicted reference's serial and recovered byte count. History never grants authority.
#[derive(Debug)]
struct WarmHistory {
    /// Admission serial of the evicted reference.
    serial: u64,
    /// Logical bytes removed from the live inventory; vector slots may remain allocated.
    recovered_bytes: u64,
}

/// A warm entry can be protected while a caller needs its local reference.
#[derive(Debug)]
struct WarmEntry {
    /// Complete checked graph reference.
    reference: Entry,
    /// Protection against eviction while the caller needs this reference.
    pinned: bool,
}

/// Preflighted warm admission. Mutation is deferred until graph publication succeeds.
#[derive(Debug)]
pub(crate) struct WarmProposal {
    /// Prepaid complete canonical key.
    statement: Vec<u8>,
    /// Prepaid warm entry and key bytes.
    bytes: u64,
    /// Ordered eviction serials, applied only after publication.
    evict_serials: Vec<u64>,
    /// Temporary serial plan capacity held through publication.
    scratch_bytes: u64,
}

impl WarmProposal {
    /// New retained bytes before any planned eviction.
    pub(crate) const fn retained_bytes(&self) -> u64 {
        self.bytes
    }

    /// Simultaneously live new reference and eviction-plan scratch.
    pub(crate) const fn staged_bytes(&self) -> u64 {
        self.bytes.saturating_add(self.scratch_bytes)
    }
}

/// Optional bounded references to checked judgments in one retained store.
///
/// Entries are ordered by admission serial. Neither a hit nor an unrelated entry
/// changes the logical Work tariff; full comparisons are still performed.
#[derive(Debug)]
pub struct ExactWarmIndex {
    /// Strong identity of the retained graph owner.
    owner: Option<Arc<()>>,
    /// Rollback namespace of the current references.
    epoch: u64,
    /// Nonwrapping per-owner admission counter.
    next_serial: u64,
    /// Ordered live references.
    entries: Vec<WarmEntry>,
    /// Bounded eviction records with no proof authority.
    history: Vec<WarmHistory>,
    /// Retained live-reference and history bytes.
    bytes: u64,
    /// Byte ceiling for this optional index.
    max_bytes: u64,
    /// Maximum count of retained eviction records.
    max_history: usize,
}

impl ExactWarmIndex {
    /// Bound both live references and eviction history without changing store limits.
    #[must_use]
    pub const fn with_limits(max_bytes: u64, max_history: usize) -> Self {
        Self {
            owner: None,
            epoch: 0,
            next_serial: 0,
            entries: Vec::new(),
            history: Vec::new(),
            bytes: 0,
            max_bytes,
            max_history,
        }
    }

    /// Construct with a one-megabyte reference allowance and 1024 history records.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_limits(1_048_576, 1024)
    }

    /// Number of live references.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the warm index has no live references.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Accounted live reference and history bytes.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }

    /// Number of historical evictions, bounded by the configured ceiling.
    #[must_use]
    pub const fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Cumulative logical bytes removed from live references by eviction.
    /// Physical slot capacity can remain until the index is released.
    #[must_use]
    pub fn recovered_bytes(&self) -> u64 {
        self.history.iter().map(|event| event.recovered_bytes).sum()
    }

    /// Most recent evicted admission serial.
    #[must_use]
    pub fn last_evicted_serial(&self) -> Option<u64> {
        self.history.last().map(|event| event.serial)
    }

    /// Last admitted serial, which a hit never refreshes.
    #[must_use]
    pub fn last_serial(&self) -> Option<u64> {
        self.entries.last().map(|entry| entry.reference.serial)
    }

    /// Protect a live serial from eviction until it is explicitly unpinned.
    /// Caller-local search has no budget parameter and charges no logical Work.
    pub fn pin(&mut self, serial: u64) -> bool {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.reference.serial == serial)
        {
            entry.pinned = true;
            true
        } else {
            false
        }
    }

    /// Make a live serial eligible for oldest-first eviction.
    /// Caller-local search has no budget parameter and charges no logical Work.
    pub fn unpin(&mut self, serial: u64) -> bool {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.reference.serial == serial)
        {
            entry.pinned = false;
            true
        } else {
            false
        }
    }

    /// Bulk-drop this owner's index without map maintenance or logical Work.
    /// Graph judgments remain store-owned.
    pub fn release(&mut self) {
        self.entries = Vec::new();
        self.history = Vec::new();
        self.bytes = 0;
    }

    /// Bind one store and invalidate old rollback generations.
    pub(crate) fn prepare(&mut self, cas: &Cas) -> Result<(), Fault> {
        if let Some(owner) = &self.owner {
            if !Arc::ptr_eq(owner, &cas.memo_owner) {
                return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
            }
        } else {
            self.owner = Some(Arc::clone(&cas.memo_owner));
        }
        if self.epoch != cas.memo_epoch || cas.memo_epoch == u64::MAX {
            self.release();
            self.epoch = cas.memo_epoch;
        }
        Ok(())
    }

    /// Compare a complete statement and proof, then authenticate the graph node.
    pub(crate) fn lookup(
        &self,
        cas: &Cas,
        kind: Kind,
        statement: &[u8],
        proof: Option<&[u8]>,
        root: Option<&Handle>,
        budget: &mut Budget,
    ) -> Result<Option<Handle>, Fault> {
        budget.work(
            size(statement.len(), AT)?.saturating_add(proof.map_or(0, |bytes| bytes.len() as u64)),
            AT,
        )?;
        for warm_entry in &self.entries {
            let entry = &warm_entry.reference;
            if entry.kind == kind
                && entry.statement == statement
                && proof.is_none_or(|bytes| entry.proof.get(2..) == Some(bytes))
                && entry.root.as_ref() == root
            {
                budget.work(
                    size(entry.statement.len(), AT)?
                        .saturating_add(proof.map_or(0, |_| entry.proof.len() as u64)),
                    AT,
                )?;
                if cas.checked(&entry.judgment, budget).is_ok() {
                    return Ok(Some(entry.judgment.clone()));
                }
            }
        }
        Ok(None)
    }

    /// Preflight capacity and an oldest-first eviction plan, without releasing authority.
    pub(crate) fn reserve(
        &mut self,
        statement: &[u8],
        budget: &mut Budget,
    ) -> Result<WarmProposal, Fault> {
        if self.next_serial == u64::MAX {
            return Err(Fault::capacity(BudgetKind::Work, AT));
        }
        let bytes = (size_of::<WarmEntry>() as u64).saturating_add(statement.len() as u64);
        let history_bytes = size_of::<WarmHistory>() as u64;
        if bytes > self.max_bytes {
            return Err(Fault::capacity(BudgetKind::PayloadBytes, AT));
        }
        let mut recovered = 0_u64;
        let mut evict_serials = Vec::new();
        while self
            .bytes
            .saturating_sub(recovered)
            .saturating_add(bytes)
            .saturating_add((evict_serials.len() as u64).saturating_mul(history_bytes))
            > self.max_bytes
        {
            let Some(entry) = self
                .entries
                .iter()
                .filter(|entry| !entry.pinned)
                .nth(evict_serials.len())
            else {
                return Err(Fault::capacity(BudgetKind::PayloadBytes, AT));
            };
            let entry = &entry.reference;
            recovered = recovered.saturating_add(
                (size_of::<WarmEntry>() as u64).saturating_add(entry.statement.len() as u64),
            );
            evict_serials.push(entry.serial);
        }
        if self.history.len().saturating_add(evict_serials.len()) > self.max_history {
            return Err(Fault::capacity(BudgetKind::PayloadBytes, AT));
        }
        let next = self
            .bytes
            .saturating_sub(recovered)
            .saturating_add(bytes)
            .saturating_add((evict_serials.len() as u64).saturating_mul(history_bytes));
        if next > self.max_bytes {
            return Err(Fault::capacity(BudgetKind::PayloadBytes, AT));
        }
        budget.check(BudgetKind::PayloadBytes, next, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        let scratch_bytes =
            (evict_serials.capacity() as u64).saturating_mul(size_of::<u64>() as u64);
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes.saturating_add(scratch_bytes),
            AT,
        )?;
        budget.work(
            bytes
                .saturating_add(1)
                .saturating_add(evict_serials.len() as u64),
            AT,
        )?;
        let mut key = Vec::new();
        key.try_reserve_exact(statement.len())
            .map_err(|_| Fault::allocation(AT))?;
        key.extend_from_slice(statement);
        self.entries
            .try_reserve_exact(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.history
            .try_reserve_exact(evict_serials.len())
            .map_err(|_| Fault::allocation(AT))?;
        Ok(WarmProposal {
            statement: key,
            bytes,
            evict_serials,
            scratch_bytes,
        })
    }

    /// Commit a preflighted reference after the graph and session have committed.
    pub(crate) fn admit(
        &mut self,
        kind: Kind,
        proposal: WarmProposal,
        proof: Arc<[u8]>,
        root: Option<Handle>,
        judgment: Handle,
    ) {
        for serial in proposal.evict_serials {
            if let Some(position) = self
                .entries
                .iter()
                .position(|entry| entry.reference.serial == serial)
            {
                let entry = self.entries.remove(position);
                let recovered = (size_of::<WarmEntry>() as u64)
                    .saturating_add(entry.reference.statement.len() as u64);
                self.bytes = self.bytes.saturating_sub(recovered);
                self.history.push(WarmHistory {
                    serial,
                    recovered_bytes: recovered,
                });
                self.bytes = self.bytes.saturating_add(size_of::<WarmHistory>() as u64);
            }
        }
        self.entries.push(WarmEntry {
            reference: Entry {
                kind,
                statement: proposal.statement,
                proof,
                root,
                judgment,
                serial: self.next_serial,
            },
            pinned: false,
        });
        self.next_serial += 1;
        self.bytes = self.bytes.saturating_add(proposal.bytes);
    }

    #[cfg(test)]
    pub(crate) fn seed_unrelated(
        &mut self,
        cas: &Cas,
        judgment: &Handle,
        count: u8,
    ) -> Result<(), Fault> {
        self.prepare(cas)?;
        self.entries
            .try_reserve_exact(count as usize)
            .map_err(|_| Fault::allocation(AT))?;
        for index in 0..count {
            let statement = vec![index; (index as usize % 3) + 1];
            self.bytes = self
                .bytes
                .saturating_add(size_of::<WarmEntry>() as u64 + statement.len() as u64);
            self.entries.push(WarmEntry {
                reference: Entry {
                    kind: if index % 2 == 0 {
                        Kind::Root
                    } else {
                        Kind::Substitution
                    },
                    statement,
                    proof: Arc::from([0_u8, 0_u8]),
                    root: None,
                    judgment: judgment.clone(),
                    serial: self.next_serial,
                },
                pinned: false,
            });
            self.next_serial += 1;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn saturate_serial_for_test(&mut self) {
        self.next_serial = u64::MAX;
    }
}

impl Default for ExactWarmIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Optional ordered index for one live mutable CAS and rollback generation.
#[derive(Debug, Default)]
pub struct ExactCacheSession {
    /// Store identity bound at first use.
    owner: Option<Arc<()>>,
    /// Rollback generation for these references.
    epoch: u64,
    /// Next stable admission serial.
    next_serial: u64,
    /// Lazily allocated ordered references.
    entries: Option<Vec<Entry>>,
    /// Accounted local reference and key bytes.
    bytes: u64,
}

impl ExactCacheSession {
    /// Construct an empty caller-held index without changing the CAS store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live, ordered local references.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.as_ref().map_or(0, Vec::len)
    }

    /// Whether the index contains no references.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Accounted local reference and complete-key residence.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }

    /// Refuse a foreign owner and clear references after a rollback namespace change.
    pub(crate) fn prepare(&mut self, cas: &Cas) -> Result<(), Fault> {
        if let Some(owner) = &self.owner {
            if !Arc::ptr_eq(owner, &cas.memo_owner) {
                return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
            }
        } else {
            self.owner = Some(Arc::clone(&cas.memo_owner));
        }
        if self.epoch != cas.memo_epoch || cas.memo_epoch == u64::MAX {
            self.entries = None;
            self.bytes = 0;
            self.epoch = cas.memo_epoch;
        }
        Ok(())
    }

    /// Inspect every admitted reference in order using full byte comparisons.
    pub(crate) fn lookup(
        &self,
        cas: &Cas,
        kind: Kind,
        statement: &[u8],
        proof: Option<&[u8]>,
        root: Option<&Handle>,
        budget: &mut Budget,
    ) -> Result<Option<Handle>, Fault> {
        budget.work(
            size(statement.len(), AT)?.saturating_add(proof.map_or(0, |bytes| bytes.len() as u64)),
            AT,
        )?;
        for entry in self.entries.iter().flat_map(|entries| entries.iter()) {
            if entry.kind == kind
                && entry.statement == statement
                && proof.is_none_or(|bytes| entry.proof.get(2..) == Some(bytes))
                && entry.root.as_ref() == root
            {
                budget.work(
                    size(entry.statement.len(), AT)?
                        .saturating_add(proof.map_or(0, |_| entry.proof.len() as u64)),
                    AT,
                )?;
                if cas.checked(&entry.judgment, budget).is_ok() {
                    return Ok(Some(entry.judgment.clone()));
                }
            }
        }
        Ok(None)
    }

    /// Share the graph-owned proof reference for a previously authenticated session hit.
    pub(crate) fn proof_for(&self, judgment: &Handle) -> Option<Arc<[u8]>> {
        self.entries
            .as_ref()?
            .iter()
            .find(|entry| &entry.judgment == judgment)
            .map(|entry| Arc::clone(&entry.proof))
    }

    /// Reserve all fallible local capacity before graph publication.
    /// Exact one-slot growth limits local over-allocation; relocation is uncharged real-time work.
    pub(crate) fn reserve(
        &mut self,
        statement: &[u8],
        budget: &mut Budget,
    ) -> Result<Proposal, Fault> {
        if self.next_serial == u64::MAX {
            return Err(Fault::capacity(BudgetKind::Work, AT));
        }
        let added = size_of::<Entry>() as u64 + statement.len() as u64;
        let next = self.bytes.saturating_add(added);
        budget.check(BudgetKind::PayloadBytes, next, AT)?;
        budget.check(BudgetKind::OutputBytes, added, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, added, AT)?;
        budget.work(added.saturating_add(1), AT)?;
        let mut key = Vec::new();
        key.try_reserve_exact(statement.len())
            .map_err(|_| Fault::allocation(AT))?;
        key.extend_from_slice(statement);
        self.entries
            .get_or_insert_with(Vec::new)
            .try_reserve_exact(1)
            .map_err(|_| Fault::allocation(AT))?;
        Ok(Proposal {
            statement: key,
            bytes: added,
        })
    }

    /// Insert a pre-reserved reference after successful graph publication.
    pub(crate) fn admit(
        &mut self,
        kind: Kind,
        proposal: Proposal,
        proof: Arc<[u8]>,
        root: Option<Handle>,
        judgment: Handle,
    ) {
        self.entries.get_or_insert_with(Vec::new).push(Entry {
            kind,
            statement: proposal.statement,
            proof,
            root,
            judgment,
            serial: self.next_serial,
        });
        self.next_serial += 1;
        self.bytes = self.bytes.saturating_add(proposal.bytes);
    }

    /// Admission order is stable and never refreshed by a hit.
    #[must_use]
    pub fn last_serial(&self) -> Option<u64> {
        self.entries
            .as_ref()
            .and_then(|entries| entries.last())
            .map(|entry| entry.serial)
    }

    /// Add unrelated entries for the lookup and admission locality control.
    #[cfg(test)]
    pub(crate) fn seed_unrelated(
        &mut self,
        cas: &Cas,
        judgment: &Handle,
        count: u8,
    ) -> Result<(), Fault> {
        self.prepare(cas)?;
        let entries = self.entries.get_or_insert_with(Vec::new);
        entries
            .try_reserve_exact(count as usize)
            .map_err(|_| Fault::allocation(AT))?;
        for index in 0..count {
            let substitution = index % 2 != 0;
            let statement = vec![index; if substitution { 256 } else { 1 }];
            let bytes = size_of::<Entry>() as u64 + statement.len() as u64;
            entries.push(Entry {
                kind: if substitution {
                    Kind::Substitution
                } else {
                    Kind::Root
                },
                statement,
                proof: Arc::from([0_u8, 0_u8]),
                root: None,
                judgment: judgment.clone(),
                serial: self.next_serial,
            });
            self.next_serial += 1;
            self.bytes = self.bytes.saturating_add(bytes);
        }
        Ok(())
    }
}
