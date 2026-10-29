// SPDX-License-Identifier: AGPL-3.0-only
//! One retained exact judgment per original mathematical expression and restoration map.

use super::DecisionError;
use crate::{decision::storage, readings::EntryMeaning};
use nrz_cas::{Cas, ExactConstant, Handle, LogicalId, Rational, limits::Budget};
use std::{collections::BTreeMap, rc::Rc};

/// Unique checked preparations retained by one physical question operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LatticePreparationUsage {
    /// Requests for original exact mathematical entries.
    pub requests: u64,
    /// Original entries answered by a retained normalization judgment.
    pub hits: u64,
    /// Separately retained exact graphs, including nonidentity restoration results.
    pub graphs: u64,
    /// Records in those independently owned exact graphs.
    pub records: u64,
    /// Conservative unique constant payload plus cache entry metadata.
    pub bytes: u64,
}

/// A cache never escapes its original question's context or definition owners.
#[derive(Debug, Default)]
pub(super) struct Preparations {
    /// Exact expression identities scoped by the single source CAS of this operation.
    normalized: BTreeMap<LogicalId, (Handle, Rc<ExactConstant>)>,
    /// Original expression, retained map definition identity and affine role.
    restored: BTreeMap<(LogicalId, usize, bool), Rc<ExactConstant>>,
    /// Unique retained payload and literal cache request counters.
    usage: LatticePreparationUsage,
}

impl Preparations {
    /// Borrow the measured unique ownership envelope.
    pub(super) const fn usage(&self) -> LatticePreparationUsage {
        self.usage
    }

    /// Normalize once within the existing preparation charge; a repeated handle pays one lookup Work.
    fn normalize(
        &mut self,
        cas: &Cas,
        node: &Handle,
        budget: &mut Budget,
    ) -> Result<Rc<ExactConstant>, DecisionError> {
        self.usage.requests = self.usage.requests.saturating_add(1);
        if let Some((original, value)) = self.normalized.get(&node.logical_id()) {
            budget.work(1, storage::AT)?;
            if original != node {
                return Err(DecisionError::LatticeMeaning);
            }
            self.usage.hits = self.usage.hits.saturating_add(1);
            return Ok(Rc::clone(value));
        }
        let value = Rc::new(budget.with_retained_payload(
            self.usage.bytes,
            storage::AT,
            |budget| ExactConstant::from_expression(cas, node, budget),
        )?);
        self.retain(&value, budget)?;
        self.normalized
            .insert(node.logical_id(), (node.clone(), Rc::clone(&value)));
        Ok(value)
    }

    /// Restore once per original map identity; trivial identities cite the normalized judgment.
    pub(super) fn restore(
        &mut self,
        cas: &Cas,
        node: &Handle,
        meaning: &EntryMeaning,
        difference: bool,
        budget: &mut Budget,
    ) -> Result<Rc<ExactConstant>, DecisionError> {
        let exact = self.normalize(cas, node, budget)?;
        let key = (node.logical_id(), meaning.cache_identity(), difference);
        if let Some(value) = self.restored.get(&key) {
            budget.work(1, storage::AT)?;
            return Ok(Rc::clone(value));
        }
        budget.work(2, storage::AT)?;
        let unit = meaning.magnitude().as_rational() == Some(&Rational::one());
        let scaled = if unit {
            exact
        } else {
            Rc::new(
                budget.with_retained_payload(self.usage.bytes, storage::AT, |budget| {
                    exact.mul(meaning.magnitude(), budget)
                })?,
            )
        };
        let shifted = !difference && !meaning.shift().is_zero();
        let restored = if shifted {
            Rc::new(
                budget.with_retained_payload(self.usage.bytes, storage::AT, |budget| {
                    scaled.add(meaning.shift(), budget)
                })?,
            )
        } else {
            scaled
        };
        if !unit || shifted {
            self.retain(&restored, budget)?;
        }
        self.usage.bytes = self.usage.bytes.saturating_add(128);
        storage::retained(self.usage.bytes, budget)?;
        self.restored.insert(key, Rc::clone(&restored));
        Ok(restored)
    }

    /// Account a newly owned exact judgment once before inserting it into the cache.
    fn retain(&mut self, value: &ExactConstant, budget: &Budget) -> Result<(), DecisionError> {
        self.usage.bytes = self
            .usage
            .bytes
            .saturating_add(storage::constant_bytes(value))
            .saturating_add(128);
        if let Some((graph, _)) = value.graph() {
            self.usage.graphs = self.usage.graphs.saturating_add(1);
            let counts = graph.counts();
            let records = u64::try_from(counts.vertices.saturating_add(counts.edges))
                .map_err(|_| DecisionError::Capacity)?;
            self.usage.records = self.usage.records.saturating_add(records);
        }
        storage::retained(self.usage.bytes, budget)
    }
}
