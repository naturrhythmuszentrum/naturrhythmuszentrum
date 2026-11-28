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
