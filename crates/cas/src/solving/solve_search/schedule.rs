// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic cost/progress scheduling with structural ties and explicit continuation payloads.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;
use crate::solving::coefficients::storage::{invalid, reserve, vector};

/// One pending intent; no queue entry is a mathematical theorem or a dominance shortcut.
#[derive(Debug)]
struct Entry<T> {
    /// Accumulated accepted primitive cost, separate from arithmetic work.
    cost: u64,
    /// Unclosed branches, remaining degree, selection constraints and inverse equations.
    progress: [u64; 4],
    /// Exact structural tie, never an insertion serial or store token.
    key: Vec<u8>,
    /// Private continuation data owned by the requesting producer.
    payload: T,
}

/// No dominance pruning occurs until complete semantic state equality can be established.
#[derive(Debug)]
pub(crate) struct Schedule<T> {
    /// Live intents, each present in both scheduling views.
    entries: Vec<Entry<T>>,
    /// Next quota position, retained across cursor resumptions.
    epoch: u64,
}

impl<T> Schedule<T> {
    /// The initial queue and cache are empty.
    pub(crate) fn new(budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            entries: vector(0, budget)?,
            epoch: 0,
        })
    }

    /// Admit one bounded intent without merging different assumptions or parameter strata.
    pub(crate) fn push(
        &mut self,
        cost: u64,
        progress: [u64; 4],
        key: Vec<u8>,
        payload: T,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let count = size(self.entries.len(), super::AT)?
            .checked_add(1)
            .ok_or_else(invalid)?;
        budget.check(BudgetKind::FrontierLabels, count, super::AT)?;
        budget.check(
            BudgetKind::QueueEntries,
            count.checked_mul(2).ok_or_else(invalid)?,
            super::AT,
        )?;
        reserve(&mut self.entries, 1, budget)?;
        self.entries.push(Entry {
            cost,
            progress,
            key,
            payload,
        });
        Ok(())
    }

    /// Both views use the same structural tie; quota selection cannot drop a remaining continuation.
    pub(crate) fn pop(&mut self, budget: &mut Budget) -> Result<Option<T>, Fault> {
        if self.entries.is_empty() {
            return Ok(None);
        }
        let cost = budget.limits().cost_quota;
        let total = cost
            .checked_add(budget.limits().progress_quota)
            .ok_or_else(invalid)?;
        let progress = self.epoch >= cost;
        self.epoch = self.epoch.checked_add(1).ok_or_else(invalid)? % total;
        let mut selected = 0;
        for index in 1..self.entries.len() {
            let candidate = self.entries.get(index).ok_or_else(invalid)?;
            let current = self.entries.get(selected).ok_or_else(invalid)?;
            budget.work(
                size(candidate.key.len().max(current.key.len()), super::AT)?.max(1),
                super::AT,
            )?;
            let earlier = if progress {
                (candidate.progress, candidate.cost, &candidate.key)
                    < (current.progress, current.cost, &current.key)
            } else {
                (candidate.cost, candidate.progress, &candidate.key)
                    < (current.cost, current.progress, &current.key)
            };
            if earlier {
                selected = index;
            }
        }
        budget.work(size(self.entries.len(), super::AT)?, super::AT)?;
        Ok(Some(self.entries.remove(selected).payload))
    }
}

#[cfg(test)]
mod tests {
    use super::Schedule;
    use crate::limits::{Budget, Limits};

    /// Four low-cost visits are followed by a progress visit, with structural ties in both views.
    /// ´test:unit:blind-scheduler-quota-and-structural-ties´
    #[test]
    fn blind_scheduler_quota_and_structural_ties() -> Result<(), crate::Fault> {
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let mut schedule = Schedule::new(&mut budget)?;
        schedule.push(9, [0; 4], vec![0], 9, &mut budget)?;
        for index in (0..5).rev() {
            schedule.push(1, [1; 4], vec![index], index, &mut budget)?;
        }
        for expected in [0, 1, 2, 3, 9, 4] {
            assert_eq!(schedule.pop(&mut budget)?, Some(expected));
        }
        assert_eq!(schedule.pop(&mut budget)?, None);
        Ok(())
    }
}
