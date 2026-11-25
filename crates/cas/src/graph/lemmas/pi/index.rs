// SPDX-License-Identifier: AGPL-3.0-only
//! Complete borrowed Machin byte keys index the authoritative graph without warm allocation.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::retained_map::RetainedMap;
use crate::graph::{Cas, LogicalId};
use crate::pi_bounds::PiBoundCertificate;

/// A retained key shares the node's canonical fields; a probe only borrows them.
#[derive(Clone, Debug)]
enum Certificate<'a> {
    /// Independently checked, immutable canonical fields shared with the graph node.
    Owned(Arc<PiBoundCertificate>),
    /// A caller's fields after fixed-size canonical length validation.
    Borrowed(&'a PiBoundCertificate),
}

/// Construction validates canonical lengths before the infallible ordered-index comparator.
#[derive(Clone, Debug)]
struct Key<'a>(Certificate<'a>);

impl Key<'_> {
    /// Both forms expose identical canonical fields without allocating an encoding buffer.
    fn certificate(&self) -> &PiBoundCertificate {
        match &self.0 {
            Certificate::Owned(value) => value,
            Certificate::Borrowed(value) => value,
        }
    }
}

impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self
            .certificate()
            .canonical_key()
            .expect("canonical key lengths were validated before indexing");
        let right = other
            .certificate()
            .canonical_key()
            .expect("canonical key lengths were validated before indexing");
        left.cmp(right)
    }
}

impl PartialOrd for Key<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key<'_> {}

/// Derived structural keys and node identifiers never substitute for checked graph evidence.
#[derive(Clone, Debug, Default)]
pub(crate) struct PiIndex {
    /// Only retained keys own the same immutable certificate allocation as their graph node.
    entries: RetainedMap<Key<'static>, LogicalId>,
    /// Namespace in which at least one entry has independently checked authority.
    owner: Option<Arc<()>>,
    /// A rollback causes the next citation to run the complete finite checker first.
    epoch: u64,
}

impl PiIndex {
    /// No stale namespace is consulted before independent full checking.
    pub(crate) fn current(&self, cas: &Cas) -> bool {
        let owned = self
            .owner
            .as_ref()
            .is_some_and(|owner| Arc::ptr_eq(owner, &cas.memo_owner));
        owned && self.epoch == cas.memo_epoch
    }

    /// Count index entries without walking unrelated graph records.
    pub(crate) const fn len(&self) -> usize {
        self.entries.len()
    }

    /// A borrowed probe returns the exact retained key allocation beside its node identifier.
    pub(crate) fn entry<'a>(
        &'a self,
        certificate: &'a PiBoundCertificate,
        floor: u64,
        budget: &mut Budget,
    ) -> Result<Option<(LogicalId, &'a Arc<PiBoundCertificate>)>, Fault> {
        let _validated = certificate.canonical_key()?;
        let probe = Key(Certificate::Borrowed(certificate));
        let Some(position) = self
            .entries
            .find_by(|key| key.cmp(&probe), floor, budget, super::AT)?
            .ok()
        else {
            return Ok(None);
        };
        let Some((key, id)) = self.entries.at(position) else {
            return Err(Fault::invalid(InvalidCode::Reference, super::AT));
        };
        let Certificate::Owned(fields) = &key.0 else {
            return Err(Fault::invalid(InvalidCode::Reference, super::AT));
        };
        Ok(Some((*id, fields)))
    }

    /// Tests inspect the identifier through the same complete canonical lookup.
    #[cfg(test)]
    pub(crate) fn get(
        &self,
        certificate: &PiBoundCertificate,
        budget: &mut Budget,
    ) -> Result<Option<LogicalId>, Fault> {
        Ok(self.entry(certificate, 0, budget)?.map(|(id, _)| id))
    }

    /// Plan a complete key after earlier unpublished keys in the same batch.
    pub(crate) fn prepare_virtual_insert<'a>(
        &mut self,
        certificate: &PiBoundCertificate,
        earlier: impl Iterator<Item = &'a PiBoundCertificate>,
        journaled: bool,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        let _validated = certificate.canonical_key()?;
        let probe = Key(Certificate::Borrowed(certificate));
        let Err(base) = self
            .entries
            .find_by(|key| key.cmp(&probe), 0, budget, super::AT)?
        else {
            return Err(Fault::invalid(InvalidCode::Reference, super::AT));
        };
        let mut before = 0_usize;
        let mut prior = 0_usize;
        for earlier_certificate in earlier {
            prior = prior
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, super::AT))?;
            let staged = Key(Certificate::Borrowed(earlier_certificate));
            match staged.cmp(&probe) {
                Ordering::Less => {
                    before = before
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, super::AT))?;
                }
                Ordering::Equal => return Err(Fault::invalid(InvalidCode::Reference, super::AT)),
                Ordering::Greater => {}
            }
        }
        let position = base
            .checked_add(before)
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, super::AT))?;
        self.entries.prepare_virtual_insert(
            self.entries
                .len()
                .checked_add(prior)
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, super::AT))?,
            position,
            journaled,
            budget,
            super::AT,
        )?;
        Ok(position)
    }

    /// Publication moves a validated certificate into the prepared sorted position.
    pub(crate) fn insert_prepared(
        &mut self,
        position: usize,
        certificate: Arc<PiBoundCertificate>,
        id: LogicalId,
    ) {
        self.entries
            .insert_prepared(position, Key(Certificate::Owned(certificate)), id);
    }

    /// Journal removal consumes its prepaid inverse and performs no new debit.
    pub(crate) fn remove_prepaid(&mut self, certificate: Arc<PiBoundCertificate>) {
        let probe = Key(Certificate::Owned(certificate));
        let _removed = self
            .entries
            .remove_by_prepaid(|candidate| candidate.cmp(&probe));
    }

    /// Successful full checking establishes the current namespace without changing key identity.
    pub(crate) fn refresh(&mut self, owner: &Arc<()>, epoch: u64) {
        self.owner = Some(Arc::clone(owner));
        self.epoch = epoch;
    }
}
