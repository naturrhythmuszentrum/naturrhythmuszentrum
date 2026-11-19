// SPDX-License-Identifier: AGPL-3.0-only
//! Fallible key-ordered scratch with charged comparisons and element movement.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::size;
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Upper bound on bytes inspected by one comparison with this query.
pub(crate) trait Key: Ord {
    /// Fixed-width keys cost one comparison; textual queries also charge their bytes.
    fn comparison_work(&self) -> u64;
}

impl Key for u64 {
    fn comparison_work(&self) -> u64 {
        1
    }
}

impl Key for petgraph::graph::NodeIndex<u32> {
    fn comparison_work(&self) -> u64 {
        1
    }
}

impl Key for str {
    fn comparison_work(&self) -> u64 {
        u64::try_from(self.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1)
    }
}

impl Key for String {
    fn comparison_work(&self) -> u64 {
        self.as_str().comparison_work()
    }
}

impl<T: Key + ?Sized> Key for &T {
    fn comparison_work(&self) -> u64 {
        (*self).comparison_work()
    }
}

impl Key for (u64, String) {
    fn comparison_work(&self) -> u64 {
        self.1.comparison_work().saturating_add(1)
    }
}

/// Sorted unique entries; no allocation or key comparison is hidden in a tree API.
pub(crate) struct Map<K, V> {
    /// Strictly increasing keys, retaining values in the same slots.
    entries: Vec<(K, V)>,
    /// Explicit reservation target; allocator over-allocation never changes the tariff.
    reserved: usize,
}

impl<K: Key, V> Map<K, V> {
    /// Construct empty scratch without allocating.
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
            reserved: 0,
        }
    }

    /// Number of retained keys.
    pub(crate) const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Visit retained entries in key order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.entries.iter()
    }

    /// Visit values in key order.
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }

    /// Visit keys in increasing order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(key, _)| key)
    }

    /// Prepay live-entry relocation at explicit doubling boundaries, then reserve fallibly.
    pub(crate) fn reserve(
        &mut self,
        additional: usize,
        budget: &mut Budget,
        at: Location,
    ) -> Result<(), Fault> {
        let count = self
            .entries
            .len()
            .checked_add(additional)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            size(count, at)?.saturating_mul(size(std::mem::size_of::<(K, V)>(), at)?),
            at,
        )?;
        if count > self.reserved {
            let doubled = self
                .reserved
                .checked_mul(2)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
            let target = count.max(doubled).max(4);
            budget.work(size(self.entries.len(), at)?, at)?;
            self.entries
                .try_reserve_exact(target - self.entries.len())
                .map_err(|_| Fault::allocation(at))?;
            self.reserved = target;
        }
        Ok(())
    }

    /// Binary search charges before each comparison, including unsuccessful lookup.
    fn locate<Q: Key + ?Sized>(
        &self,
        key: &Q,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Result<usize, usize>, Fault>
    where
        K: Borrow<Q>,
    {
        let mut low = 0;
        let mut high = self.entries.len();
        while low < high {
            let middle = low + (high - low) / 2;
            budget.work(key.comparison_work(), at)?;
            match self.entries[middle].0.borrow().cmp(key) {
                Ordering::Less => low = middle + 1,
                Ordering::Equal => return Ok(Ok(middle)),
                Ordering::Greater => high = middle,
            }
        }
        Ok(Err(low))
    }

    /// Look up a value without changing the collection.
    pub(crate) fn get<Q: Key + ?Sized>(
        &self,
        key: &Q,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<&V>, Fault>
    where
        K: Borrow<Q>,
    {
        Ok(self
            .locate(key, budget, at)?
            .ok()
            .map(|index| &self.entries[index].1))
    }

    /// Look up a value for mutation without changing key order.
    pub(crate) fn get_mut<Q: Key + ?Sized>(
        &mut self,
        key: &Q,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<&mut V>, Fault>
    where
        K: Borrow<Q>,
    {
        Ok(self
            .locate(key, budget, at)?
            .ok()
            .map(|index| &mut self.entries[index].1))
    }

    /// Insert or replace after prepaying every shifted element and reserving fallibly.
    pub(crate) fn insert(
        &mut self,
        key: K,
        value: V,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<V>, Fault> {
        match self.locate(&key, budget, at)? {
            Ok(index) => {
                budget.work(2, at)?;
                Ok(Some(std::mem::replace(&mut self.entries[index].1, value)))
            }
            Err(index) => {
                budget.work(size(self.entries.len() - index, at)?.saturating_add(1), at)?;
                self.reserve(1, budget, at)?;
                self.entries.insert(index, (key, value));
                Ok(None)
            }
        }
    }

    /// Remove after prepaying the returned element and every element shifted left.
    pub(crate) fn remove<Q: Key + ?Sized>(
        &mut self,
        key: &Q,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<V>, Fault>
    where
        K: Borrow<Q>,
    {
        let Ok(index) = self.locate(key, budget, at)? else {
            return Ok(None);
        };
        budget.work(size(self.entries.len() - index, at)?, at)?;
        Ok(Some(self.entries.remove(index).1))
    }
}

#[cfg(test)]
mod tests;
