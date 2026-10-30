// SPDX-License-Identifier: AGPL-3.0-only
//! Every citing ledger reserves store-owned records beside its later scratch.

use std::sync::Arc;

use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, size};

/// Conservative reservations survive later operations on the same cumulative ledger.
#[derive(Debug, Default)]
pub(crate) struct Accounting {
    /// Strong namespace identities prevent a dropped store from aliasing a later store.
    entries: Vec<(Arc<()>, u64)>,
    /// Complete retained judgments and their ledger registration metadata.
    pub(crate) bytes: u64,
}

impl Accounting {
    /// Remember the earlier reservation before an atomic receipt attempt.
    pub(super) fn snapshot(&self, owner: &Arc<()>) -> Option<u64> {
        self.entries
            .iter()
            .find(|(entry, _)| Arc::ptr_eq(entry, owner))
            .map(|(_, bytes)| *bytes)
    }

    /// Restore only this store's earlier registration; its previous slot is already allocated.
    pub(super) fn restore(&mut self, owner: &Arc<()>, bytes: Option<u64>) {
        self.forget(owner);
        if let Some(bytes) = bytes {
            self.entries.push((Arc::clone(owner), bytes));
            self.bytes = self.bytes.saturating_add(bytes).saturating_add(64);
        }
    }

    /// Rollback releases storage reservations without refunding any consumed Work.
    pub(crate) fn forget(&mut self, owner: &Arc<()>) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(entry, _)| Arc::ptr_eq(entry, owner))
        {
            let (_, bytes) = self.entries.remove(index);
            self.bytes -= bytes.saturating_add(64);
        }
    }
}

/// Reserve the complete current store retention, including all previous citing stores.
pub(super) fn retain(cas: &Cas, bytes: u64, budget: &mut Budget) -> Result<(), Fault> {
    let prior = budget
        .checked_records
        .entries
        .iter()
        .position(|(owner, _)| Arc::ptr_eq(owner, &cas.memo_owner));
    let old = prior.map_or(0, |index| {
        budget.checked_records.entries[index].1.saturating_add(64)
    });
    let new = if bytes == 0 {
        0
    } else {
        bytes.saturating_add(64)
    };
    budget.check(BudgetKind::StagedPayloadBytes, new.saturating_sub(old), AT)?;
    if new == 0 {
        budget.checked_records.forget(&cas.memo_owner);
        return Ok(());
    }
    let count = budget
        .checked_records
        .entries
        .len()
        .saturating_add(usize::from(prior.is_none()));
    budget.check(BudgetKind::Vertices, size(count, AT)?, AT)?;
    if let Some(index) = prior {
        budget.checked_records.entries[index].1 = bytes;
    } else {
        budget
            .checked_records
            .entries
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        budget
            .checked_records
            .entries
            .push((Arc::clone(&cas.memo_owner), bytes));
    }
    budget.checked_records.bytes = budget
        .checked_records
        .bytes
        .saturating_sub(old)
        .saturating_add(new);
    Ok(())
}
