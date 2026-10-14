// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic two-queue scheduling and conservative resource-aware dominance.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{LogicalId, size};

/// Fixed task rank before a structural theorem-key tie break.
pub(crate) type Rank = [u64; 4];
/// Queue records include a fresh serial on every resumed visit.
type QueueKey = (u64, Rank, Vec<u8>, u64, usize);
/// Progress order swaps the first two rank components.
type ProgressKey = (Rank, u64, Vec<u8>, u64, usize);

/// Every dimension needed for conservative path dominance.
#[derive(Clone, Debug)]
pub(crate) struct Metrics {
    /// Sum of positive logical transition weights.
    pub(crate) cost: u64,
    /// Peak unfolded expression size.
    pub(crate) peak: u64,
    /// Cumulative positive size increases, without refunds.
    pub(crate) credits: u64,
    /// Checked proof depth.
    pub(crate) depth: u64,
    /// Canonical witness footprint.
    pub(crate) bytes: u64,
    /// Required source, proof and rule-support identities.
    pub(crate) support: BTreeSet<LogicalId>,
    /// Canonical proof tie-break, never a runtime address.
    pub(crate) encoding: Vec<u8>,
}

impl Metrics {
    /// Different support sets or a cost/credit tradeoff prevent unsafe pruning.
    pub(crate) fn dominates(&self, other: &Self) -> bool {
        let own = [self.cost, self.peak, self.credits, self.depth, self.bytes];
        let peer = [
            other.cost,
            other.peak,
            other.credits,
            other.depth,
            other.bytes,
        ];
        own.iter().zip(peer).all(|(left, right)| *left <= right)
            && self.support.is_subset(&other.support)
            && (own != peer || self.support != other.support || self.encoding <= other.encoding)
    }
}

/// Scheduler keys kept separately from producer-owned proof payloads.
struct Entry {
    /// Full state key includes context, relation and residual class.
    key: Vec<u8>,
    /// Task-appropriate rank.
    rank: Rank,
    /// Path resources and dependency support.
    metrics: Metrics,
    /// Present exactly while queued.
    serial: Option<u64>,
    /// Dominated records cannot resume an abandoned cursor.
    discarded: bool,
}

/// Both queues refer to the same pending labels and share exact positive quotas.
#[derive(Default)]
pub(crate) struct Frontier {
    /// Cost-priority pending visits.
    cost: BTreeSet<QueueKey>,
    /// Progress-priority pending visits.
    progress: BTreeSet<ProgressKey>,
    /// All retained resource labels, including completed ones for dominance.
    entries: BTreeMap<usize, Entry>,
    /// Fresh deterministic resumption serial.
    next_serial: u64,
    /// Current epoch position.
    slot: u64,
    /// Actual queue classes selected, capped as diagnostic records.
    pub(crate) visits: Vec<bool>,
    /// Count of dominated pending or completed labels.
    pub(crate) dominated: u64,
}

/// Queue accounting location.
const AT: Location = Location::new("search frontier");

impl Frontier {
    /// Prepay all container/key comparisons and live queue payload.
    fn charge(&self, budget: &mut Budget) -> Result<(), Fault> {
        let mut bytes = 0_u64;
        for entry in self.entries.values() {
            bytes = bytes
                .checked_add(size(entry.key.len(), AT)?)
                .and_then(|n| n.checked_add(size(entry.metrics.encoding.len(), AT).ok()?))
                .and_then(|n| {
                    n.checked_add(
                        size(entry.metrics.support.len(), AT)
                            .ok()?
                            .checked_mul(16)?,
                    )
                })
                .and_then(|n| n.checked_add(256))
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes
                .checked_mul(4)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        budget.work(
            bytes
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )
    }

    /// Admit a nondominated label, removing both queue entries of dominated alternatives.
    pub(crate) fn insert(
        &mut self,
        id: usize,
        key: Vec<u8>,
        rank: Rank,
        metrics: Metrics,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        self.charge(budget)?;
        if self
            .entries
            .values()
            .any(|entry| !entry.discarded && entry.key == key && entry.metrics.dominates(&metrics))
        {
            return Ok(false);
        }
        let mut discarded = Vec::new();
        discarded
            .try_reserve(self.entries.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (previous, entry) in &self.entries {
            if !entry.discarded && entry.key == key && metrics.dominates(&entry.metrics) {
                discarded.push(*previous);
            }
        }
        for previous in discarded {
            self.remove(previous)?;
            self.entries
                .get_mut(&previous)
                .ok_or_else(invalid)?
                .discarded = true;
            self.dominated = self
                .dominated
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::FrontierLabels, AT))?;
        }
        self.entries.insert(
            id,
            Entry {
                key,
                rank,
                metrics,
                serial: None,
                discarded: false,
            },
        );
        self.resume(id, budget)?;
        Ok(true)
    }

    /// Resume a bounded matching cursor behind earlier equal-priority visits.
    pub(crate) fn resume(&mut self, id: usize, budget: &mut Budget) -> Result<(), Fault> {
        self.charge(budget)?;
        let live = size(self.cost.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::FrontierLabels, AT))?;
        budget.check(BudgetKind::FrontierLabels, live, AT)?;
        budget.check(
            BudgetKind::QueueEntries,
            live.checked_mul(3)
                .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?,
            AT,
        )?;
        let entry = self.entries.get_mut(&id).ok_or_else(invalid)?;
        if entry.discarded {
            return Ok(());
        }
        if entry.serial.is_some() {
            return Err(invalid());
        }
        let serial = self.next_serial;
        self.next_serial = serial
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?;
        entry.serial = Some(serial);
        self.cost.insert((
            entry.metrics.cost,
            entry.rank,
            entry.key.clone(),
            serial,
            id,
        ));
        self.progress.insert((
            entry.rank,
            entry.metrics.cost,
            entry.key.clone(),
            serial,
            id,
        ));
        Ok(())
    }

    /// Pop one live label from the scheduled queue and remove its companion entry.
    pub(crate) fn pop(&mut self, budget: &mut Budget) -> Result<Option<usize>, Fault> {
        self.charge(budget)?;
        if self.cost.is_empty() && self.progress.is_empty() {
            return Ok(None);
        }
        let epoch = budget
            .limits()
            .cost_quota
            .checked_add(budget.limits().progress_quota)
            .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?;
        let progress = (self.slot >= budget.limits().cost_quota && !self.progress.is_empty())
            || self.cost.is_empty();
        self.slot = self
            .slot
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::QueueEntries, AT))?
            % epoch;
        let id = if progress {
            self.progress.first().map(|entry| entry.4)
        } else {
            self.cost.first().map(|entry| entry.4)
        }
        .ok_or_else(invalid)?;
        self.remove(id)?;
        if size(self.visits.len(), AT)? < budget.limits().diagnostic_records {
            self.visits
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            self.visits.push(progress);
        }
        Ok(Some(id))
    }

    /// Remove both references without removing the completed dominance record.
    fn remove(&mut self, id: usize) -> Result<(), Fault> {
        let entry = self.entries.get_mut(&id).ok_or_else(invalid)?;
        if let Some(serial) = entry.serial.take() {
            self.cost.remove(&(
                entry.metrics.cost,
                entry.rank,
                entry.key.clone(),
                serial,
                id,
            ));
            self.progress.remove(&(
                entry.rank,
                entry.metrics.cost,
                entry.key.clone(),
                serial,
                id,
            ));
        }
        Ok(())
    }
}

/// Internal bookkeeping errors remain values.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}

/// Scheduling tests use independent numbered labels, without the rewrite implementation.
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Frontier, Metrics};
    use crate::{Budget, Fault, Limits, LogicalId, VertexKind};

    /// If one scheduling view has no eligible entry, its slot is donated to the other view.
    /// ´test:unit:queue-empty-donation´
    #[test]
    fn queue_empty_donation() -> Result<(), Fault> {
        let mut frontier = Frontier::default();
        let mut budget = Budget::new(Limits::default())?;
        let metrics = Metrics {
            cost: 1,
            credits: 0,
            peak: 8,
            depth: 2,
            bytes: 10,
            support: BTreeSet::from([LogicalId {
                kind: VertexKind::Proof,
                serial: 0,
            }]),
            encoding: vec![1],
        };
        frontier.insert(0, vec![0], [0; 4], metrics, &mut budget)?;
        frontier.cost.clear();
        assert_eq!(frontier.pop(&mut budget)?, Some(0));
        assert_eq!(frontier.visits, vec![true]);
        Ok(())
    }
}
