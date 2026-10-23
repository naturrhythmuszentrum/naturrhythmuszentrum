// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-owned retained derivatives avoid any traversal of a previously proved exact key.

use super::{AT, SourceJacobian, SourceJacobianKey, storage};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, size};

/// One retained derivative per local exact key, independent of numerical operating points.
///
/// Build each `SourceJacobianKey` once. A cache hit borrows the retained result,
/// spends only key-comparison Work, and publishes no graph records. A fresh store
/// needs its own key and witness even when portable bytes match.
#[derive(Debug, Default)]
pub struct SourceJacobianCache {
    /// Sealed local results, never caller-supplied proof claims.
    entries: Vec<SourceJacobian>,
    /// Conservative retained result capacity, shared by all nested operations.
    bytes: u64,
}

impl SourceJacobianCache {
    /// Empty caller-owned retention, with no hidden default ledger or global state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
        }
    }

    /// Number of retained local exact keys.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no derivative has been retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the retained witness or derive and atomically retain one new key.
    ///
    /// A hit performs no expression, domain, derivative or witness traversal.
    /// Store ceilings still intersect caller limits, including retained payload.
    /// # Errors
    /// Refuses foreign keys, exhausted comparisons/storage and all ordinary
    /// source-Jacobian construction refusals without retaining a partial result.
    pub fn get_or_derive<'a>(
        &'a mut self,
        cas: &mut Cas,
        key: &SourceJacobianKey,
        budget: &mut Budget,
    ) -> Result<&'a SourceJacobian, Fault> {
        let previous = budget.restrict_storage(&cas.limits);
        let result = self.retained(cas, key, budget);
        budget.restore_limits(previous);
        result
    }

    /// Authenticate store ownership before comparing collision-free portable contents.
    fn position(
        &self,
        cas: &Cas,
        key: &SourceJacobianKey,
        budget: &mut Budget,
    ) -> Result<Option<usize>, Fault> {
        cas.checked(&key.context)?;
        cas.checked(&key.original)?;
        budget.check(BudgetKind::StagedPayloadBytes, self.bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, self.bytes, AT)?;
        for (index, value) in self.entries.iter().enumerate() {
            budget.work(
                size(value.key.bytes.len().max(key.bytes.len()), AT)?.saturating_add(1),
                AT,
            )?;
            let local = cas.checked(&value.key.context).is_ok();
            if local && value.key == *key {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// Publish the cache entry only after the complete graph transaction and payload checks succeed.
    fn retained<'a>(
        &'a mut self,
        cas: &mut Cas,
        key: &SourceJacobianKey,
        budget: &mut Budget,
    ) -> Result<&'a SourceJacobian, Fault> {
        let index = if let Some(index) = self.position(cas, key, budget)? {
            index
        } else {
            let result = cas.transaction(budget, |cas, budget| {
                storage::retaining(self.bytes, budget, |budget| {
                    let result = cas.source_jacobian(key, budget)?;
                    let total = self.bytes.saturating_add(payload(&result)?);
                    budget.check(BudgetKind::OutputBytes, total, AT)?;
                    budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
                    self.entries
                        .try_reserve(1)
                        .map_err(|_| Fault::allocation(AT))?;
                    Ok((result, total))
                })
            })?;
            let index = self.entries.len();
            self.bytes = result.1;
            self.entries.push(result.0);
            index
        };
        self.entries
            .get(index)
            .ok_or_else(|| super::invalid(crate::error::InvalidCode::Reference))
    }
}

/// Key and canonical proof payloads bound retained owned witnesses and typed views.
pub(super) fn payload(value: &SourceJacobian) -> Result<u64, Fault> {
    let mut bytes = value
        .key
        .payload()
        .saturating_add(size(value.witness.bytes.len(), AT)?.saturating_mul(8))
        .saturating_add(1024);
    if let Some(representation) = &value.representation {
        bytes = bytes.saturating_add(size(representation.bytes.len(), AT)?.saturating_mul(8));
    }
    Ok(bytes)
}
