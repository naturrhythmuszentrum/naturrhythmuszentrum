// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-owned anonymous lemmas retain one checked proof per complete canonical key.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, size};
use crate::proof_model::{Judgment, VerifiedWitness};

/// Complete-key construction and retention share one accounting location.
const AT: Location = Location::new("anonymous lemma cache");

/// Named schema for anonymous instance reuse, independent of any display name.
pub const ANONYMOUS_LEMMA_SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: "lemma.cache",
    label: "thm:cas:lemma-cache",
    version: 1,
    statement: "A sealed local theorem is reusable only under its complete canonical judgment and proof bytes.",
    premises: "Identical relation, declarations, context, stratum, ordered premises, representation, proof revisions and local proof owner.",
};

/// A caller-owned cache of sealed local lemmas across requests and retries.
///
/// The collision-free key qualifies the receiving-store owner by the complete canonical expected judgment and
/// the complete canonical proof bytes, including declarations, context, stratum,
/// ordered premises, representation and the used primitive revisions. A different
/// proof is a different key even when its theorem, shape, name or cost is equal.
/// A receiving store checks its own entry; portable equality never imports handles.
#[derive(Debug, Default)]
pub struct AnonymousLemmaCache {
    /// Sealed results with exact portable expected judgments.
    entries: Vec<Entry>,
    /// Conservative aggregate retained payload.
    bytes: u64,
}

/// A private entry can be minted only after independent replay succeeds.
#[derive(Debug)]
struct Entry {
    /// Collision-free canonical expected theorem.
    expected: Vec<u8>,
    /// Canonical proof bytes complete the key and seal its local result.
    witness: VerifiedWitness,
}

impl AnonymousLemmaCache {
    /// Empty caller-owned retention with no hidden ledger or global state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
        }
    }

    /// Number of completely checked local keys retained so far.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no complete checked key has been retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Conservative retained payload charged while this cache participates in a call.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }

    /// Borrow an exact retained lemma or independently replay and atomically retain it.
    ///
    /// Key construction, comparisons and retained storage remain metered. A hit
    /// performs no rule checking or publication. An error never creates an entry.
    /// # Errors
    /// Refuses invalid or foreign judgments, altered proof bytes, false rules and
    /// exhausted caller/store limits with the ordinary replay error.
    pub fn replay<'a>(
        &'a mut self,
        cas: &mut Cas,
        bytes: &[u8],
        expected: &Judgment,
        budget: &mut Budget,
    ) -> Result<&'a VerifiedWitness, Fault> {
        let previous = budget.restrict_storage(&cas.limits);
        let result = crate::proof_check::replay_budget(budget, |budget| {
            self.retained(cas, bytes, expected, budget)
        });
        budget.restore_limits(previous);
        result
    }

    /// Retained bytes remain reserved throughout key construction and any miss.
    fn retained<'a>(
        &'a mut self,
        cas: &mut Cas,
        bytes: &[u8],
        expected: &Judgment,
        budget: &mut Budget,
    ) -> Result<&'a VerifiedWitness, Fault> {
        budget.check(BudgetKind::OutputBytes, self.bytes, AT)?;
        let index = budget.with_retained_payload(self.bytes, AT, |budget| {
            crate::proof_view::validate_judgment(cas, expected, budget)?;
            let key = crate::wire_build::judgment_key(cas, expected, budget)?;
            budget.with_retained_payload(size(key.len(), AT)?, AT, |budget| {
                if let Some(index) = self.position(cas, &key, bytes, budget)? {
                    return Ok(index);
                }
                self.insert(cas, key, bytes, expected, budget)
            })
        })?;
        self.entries
            .get(index)
            .map(|entry| &entry.witness)
            .ok_or_else(invalid)
    }

    /// A complete byte comparison and store authentication precede every hit.
    fn position(
        &self,
        cas: &Cas,
        key: &[u8],
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Option<usize>, Fault> {
        for (index, entry) in self.entries.iter().enumerate() {
            let work = size(entry.expected.len().max(key.len()), AT)?
                .saturating_add(size(entry.witness.bytes.len().max(bytes.len()), AT)?)
                .saturating_add(1);
            budget.work(work, AT)?;
            let local = cas.checked(entry.witness.proof()).is_ok();
            if local && entry.expected == key && entry.witness.bytes == bytes {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// Reserve retention before replay, then publish the cache entry after the graph transaction.
    fn insert(
        &mut self,
        cas: &mut Cas,
        key: Vec<u8>,
        bytes: &[u8],
        expected: &Judgment,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        let amount = size(key.len(), AT)?
            .saturating_add(size(bytes.len(), AT)?)
            .saturating_mul(8)
            .saturating_add(1024);
        budget.check(
            BudgetKind::OutputBytes,
            self.bytes.saturating_add(amount),
            AT,
        )?;
        budget.check(BudgetKind::StagedPayloadBytes, amount, AT)?;
        budget.check(
            BudgetKind::Vertices,
            size(self.entries.len(), AT)?.saturating_add(1),
            AT,
        )?;
        budget.work(amount, AT)?;
        let witness = cas.transaction(budget, |cas, budget| {
            let witness = budget
                .with_retained_payload(amount, AT, |budget| cas.replay(bytes, expected, budget))?;
            self.entries
                .try_reserve_exact(1)
                .map_err(|_| Fault::allocation(AT))?;
            Ok(witness)
        })?;
        let index = self.entries.len();
        self.entries.push(Entry {
            expected: key,
            witness,
        });
        self.bytes = self.bytes.saturating_add(amount);
        Ok(index)
    }
}

/// A missing retained index is an error, never a fabricated theorem.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}

#[cfg(test)]
mod tests;
