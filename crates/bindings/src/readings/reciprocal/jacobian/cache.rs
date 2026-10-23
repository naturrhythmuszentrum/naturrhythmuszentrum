// SPDX-License-Identifier: AGPL-3.0-only
//! Each exact derivative key is retained once without reopening a per-solve context.

use super::{ReadingError, ReciprocalJacobian, ReciprocalJacobianKey, support};
use nrz_cas::{
    Cas,
    limits::{Budget, BudgetKind},
};

/// Caller-owned derivative retention over one common exact graph and cumulative ledger.
#[derive(Debug, Default)]
pub struct ReciprocalJacobianCache {
    /// Shared sealed derivatives, independent of all numerical evaluations.
    entries: Vec<ReciprocalJacobian>,
    /// Complete retained evidence bound across every exact key.
    bytes: u64,
}

impl ReciprocalJacobianCache {
    /// Empty retention creates neither a graph nor a work allowance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
        }
    }

    /// Number of distinct original exact keys derived here.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no derivative is retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Complete retained payload, also reserved while deriving another exact key.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }

    /// Reuse a checked derivative or derive it once from its original exact graph.
    ///
    /// Hits authenticate the local context and compare complete keys without traversing
    /// expressions or replaying proofs. Failed attempts consume the caller ledger and
    /// publish no cache entry; retries never receive a new allowance.
    /// # Errors
    /// Refuses foreign keys and exhausted cumulative work or retained storage.
    pub fn get_or_derive(
        &mut self,
        cas: &mut Cas,
        key: &ReciprocalJacobianKey,
        budget: &mut Budget,
    ) -> Result<ReciprocalJacobian, ReadingError> {
        cas.context_header(key.definition().frame().context())?;
        budget.check(BudgetKind::StagedPayloadBytes, self.bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, self.bytes, support::AT)?;
        for entry in &self.entries {
            if entry.key().same(key, budget)? {
                return Ok(entry.clone());
            }
        }
        let value = budget.with_retained_payload(self.bytes, support::AT, |budget| {
            ReciprocalJacobian::derive(cas, key, budget)
        })?;
        let bytes = self
            .bytes
            .checked_add(value.retained_payload_bytes())
            .ok_or(ReadingError::Shape)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| nrz_cas::Fault::allocation(support::AT))?;
        self.entries.push(value.clone());
        self.bytes = bytes;
        Ok(value)
    }
}
