// SPDX-License-Identifier: AGPL-3.0-only
//! Sorted retained index with checked capacity and local-entry Work prepayment.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::mem::size_of;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};

/// (`inv:nrz:cas-backend-target-coverage`) requires an installed backing with n
/// live entries to have capacity at least target(n). An empty vector may have no backing.
/// Binary search preserves strict sorted uniqueness; every comparator runs only
/// after the caller's cumulative lookup tariff has been charged.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RetainedMap<K, V> {
    /// Sorted complete keys and their retained values.
    entries: Vec<(K, V)>,
}

impl<K: Clone, V: Clone> Clone for RetainedMap<K, V> {
    fn clone(&self) -> Self {
        let mut entries = Vec::with_capacity(self.entries.capacity());
        entries.extend(self.entries.iter().cloned());
        Self { entries }
    }
}

#[cfg(test)]
impl<K: Ord, V> FromIterator<(K, V)> for RetainedMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let source: Vec<_> = iter.into_iter().collect();
        let minimum = if source.len() <= 4 {
            4
        } else {
            source.len().next_power_of_two()
        };
        let mut entries = Vec::with_capacity(minimum);
        entries.extend(source);
        Self { entries }
    }
}

impl<K, V> Default for RetainedMap<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

/// Length-derived logical capacity, independent of allocator over-allocation.
fn target(n: usize, at: Location) -> Result<usize, Fault> {
    if n <= 4 {
        Ok(4)
    } else {
        n.checked_next_power_of_two()
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))
    }
}

impl<K, V> RetainedMap<K, V> {
    /// Number of live entries in complete-key order.
    pub(crate) const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Visit live entries in complete-key order.
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    /// Visit retained complete keys in sorted order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(key, _)| key)
    }

    #[cfg(test)]
    pub(crate) fn peek(&self, key: &K) -> Option<&V>
    where
        K: PartialEq,
    {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value)
    }

    #[cfg(test)]
    pub(crate) fn peek_mut(&mut self, key: &K) -> Option<&mut V>
    where
        K: PartialEq,
    {
        self.entries
            .iter_mut()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value)
    }

    /// Read an already searched position; this performs no new map comparison.
    pub(crate) fn at(&self, position: usize) -> Option<(&K, &V)> {
        self.entries.get(position).map(|(key, value)| (key, value))
    }

    /// Replace an already searched value after its two Work units were paid.
    pub(crate) fn replace_prepared(&mut self, position: usize, value: V) -> V {
        std::mem::replace(&mut self.entries[position].1, value)
    }

    /// A checkpoint clones the logical entries into a newly covered backing.
    pub(crate) fn checkpoint(&self, at: Location) -> Result<Self, Fault>
    where
        K: Clone,
        V: Clone,
    {
        let mut entries = Vec::new();
        if !self.entries.is_empty() {
            let required = target(self.len(), at)?;
            required
                .checked_mul(size_of::<(K, V)>())
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
            entries
                .try_reserve_exact(required)
                .map_err(|_| Fault::allocation(at))?;
            entries.extend(self.entries.iter().cloned());
        }
        Ok(Self { entries })
    }

    /// Reserve the next logical target before a prepared insertion.
    pub(crate) fn prepare_insert(
        &mut self,
        position: usize,
        journaled: bool,
        budget: &mut Budget,
        at: Location,
    ) -> Result<u64, Fault> {
        self.prepare_virtual_insert(self.len(), position, journaled, budget, at)
    }

    /// Prepay one insertion in a batch while earlier prepared insertions remain unpublished.
    pub(crate) fn prepare_virtual_insert(
        &mut self,
        n: usize,
        position: usize,
        journaled: bool,
        budget: &mut Budget,
        at: Location,
    ) -> Result<u64, Fault> {
        if n < self.len() {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
        }
        let required = n
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        let old_target = target(n, at)?;
        if n > 0 && self.entries.capacity() < old_target {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
        }
        let new_target = target(required, at)?;
        new_target
            .checked_mul(size_of::<(K, V)>())
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        if position > n {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
        }
        let inverse = u64::from(journaled);
        let charge = 1 + inverse;
        budget.work(charge, at)?;
        if self.entries.capacity() < new_target {
            self.entries
                .try_reserve_exact(new_target - self.len())
                .map_err(|_| Fault::allocation(at))?;
        }
        Ok(inverse)
    }

    /// A successful preflight has already reserved work and storage.
    pub(crate) fn insert_prepared(&mut self, position: usize, key: K, value: V) {
        self.entries.insert(position, (key, value));
    }

    /// Undo uses its journal entry's prepaid inverse entry credit.
    pub(crate) fn remove_prepaid<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.remove_by_prepaid(|candidate| candidate.borrow().cmp(key))
    }

    /// Remove through a borrowed comparator using its journaled entry credit.
    pub(crate) fn remove_by_prepaid(
        &mut self,
        mut compare: impl FnMut(&K) -> Ordering,
    ) -> Option<(K, V)> {
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare(&self.entries[middle].0) {
                Ordering::Less => low = middle + 1,
                Ordering::Equal => return Some(self.entries.remove(middle)),
                Ordering::Greater => high = middle,
            }
        }
        None
    }
}

impl<K: Ord, V> RetainedMap<K, V> {
    /// Search [low,high), charging only the site's former lookup floor.
    pub(crate) fn find<Q>(
        &self,
        key: &Q,
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Result<usize, usize>, Fault>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.find_by(|candidate| candidate.borrow().cmp(key), floor, budget, at)
    }

    /// The comparator can borrow a complete structural key without encoding it.
    pub(crate) fn find_by(
        &self,
        mut compare: impl FnMut(&K) -> Ordering,
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Result<usize, usize>, Fault> {
        crate::work_observation::observe!(budget, at, { budget.work(floor, at) })?;
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare(&self.entries[middle].0) {
                Ordering::Less => low = middle + 1,
                Ordering::Equal => {
                    return Ok(Ok(middle));
                }
                Ordering::Greater => high = middle,
            }
        }
        Ok(Err(low))
    }

    /// Find a value by a borrowed complete key and charge its former floor.
    pub(crate) fn get<Q>(
        &self,
        key: &Q,
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<&V>, Fault>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        Ok(self
            .find(key, floor, budget, at)?
            .ok()
            .map(|position| &self.entries[position].1))
    }
}

#[cfg(test)]
mod tests {
    use super::RetainedMap;
    use crate::budget::{Budget, BudgetKind, Limits};
    use crate::error::{Fault, InvalidCode, Location};
    use crate::graph::{Cas, Draft, LogicalId, VertexKind};
    use crate::testing::common::TestResult;

    const AT: Location = Location::new("retained map test");

    fn fresh_budget(work: u64) -> Result<Budget, Fault> {
        Budget::new(Limits {
            work,
            ..Limits::default()
        })
    }

    fn words() -> RetainedMap<String, u64> {
        let mut entries = Vec::with_capacity(4);
        entries.extend([("axxx".into(), 1), ("cxxx".into(), 2), ("exxx".into(), 3)]);
        RetainedMap { entries }
    }

    /// ´test:unit:backend-sparse-identity-order´
    #[test]
    fn backend_sparse_identity_order() -> TestResult {
        let low = LogicalId {
            kind: VertexKind::Source,
            serial: 0,
        };
        let gap = LogicalId {
            kind: VertexKind::Source,
            serial: 1_u64 << 40,
        };
        let other_kind = LogicalId {
            kind: VertexKind::Proof,
            serial: 7,
        };
        let mut ids = RetainedMap::<LogicalId, u64>::default();
        let mut work = fresh_budget(1_000)?;
        for (id, value) in [(gap, 2), (other_kind, 3), (low, 1)] {
            let position = ids.find(&id, 0, &mut work, AT)?.err().ok_or("duplicate")?;
            ids.prepare_insert(position, true, &mut work, AT)?;
            ids.insert_prepared(position, id, value);
        }
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.entries.capacity(), 4);
        assert_eq!(
            ids.keys().copied().collect::<Vec<_>>(),
            vec![low, gap, other_kind]
        );
        assert_eq!(ids.get(&gap, 0, &mut work, AT)?, Some(&2));
        let mut zero_floor = fresh_budget(0)?;
        assert_eq!(ids.find(&gap, 0, &mut zero_floor, AT)?, Ok(1));
        assert_eq!(zero_floor.usage().work, 0);
        assert_eq!(
            ids.get(&LogicalId { serial: 1, ..low }, 0, &mut work, AT)?,
            None
        );

        // A parent's child claims need sorted order even when admitted backward.
        let mut parents = RetainedMap::<LogicalId, LogicalId>::default();
        for (child, parent) in [(gap, other_kind), (low, gap)] {
            let position = parents
                .find(&child, 0, &mut work, AT)?
                .err()
                .ok_or("duplicate child")?;
            parents.prepare_insert(position, true, &mut work, AT)?;
            parents.insert_prepared(position, child, parent);
        }
        assert_eq!(parents.keys().copied().collect::<Vec<_>>(), vec![low, gap]);
        assert_eq!(parents.get(&low, 0, &mut work, AT)?, Some(&gap));

        // The complete policy key includes its version; the same identity has one value.
        let mut cas = Cas::new(Limits::default())?;
        let mut ledger = Budget::new(Limits::default())?;
        let mut policy = crate::cost::CostPolicy {
            key: "backend-policy".into(),
            version: 1,
            ..crate::cost::CostPolicy::default()
        };
        let first = cas.pin_policy(&policy, &mut ledger)?;
        assert_eq!(cas.pin_policy(&policy, &mut ledger)?, first);
        let canonical = cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        policy.weights[0] += 1;
        assert_eq!(
            cas.pin_policy(&policy, &mut ledger),
            Err(Fault::invalid(
                InvalidCode::Policy,
                Location::new("cost policy")
            ))
        );
        assert_eq!(
            cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
            canonical
        );
        policy.version = 2;
        let second = cas.pin_policy(&policy, &mut ledger)?;
        assert_ne!(first, second);
        assert_eq!(cas.state.policies.len(), 2);
        Ok(())
    }

    /// Search depth does not change the former lookup floor.
    /// ´test:unit:backend-maintenance-boundaries´
    #[test]
    fn backend_maintenance_boundaries() -> TestResult {
        let map = words();
        let mut exact = fresh_budget(5)?;
        assert_eq!(map.find("dxxx", 5, &mut exact, AT)?, Err(2));
        assert_eq!(exact.usage().work, 5);
        let mut one_below = fresh_budget(4)?;
        assert_eq!(
            map.find("dxxx", 5, &mut one_below, AT),
            Err(Fault::exhausted(BudgetKind::Work, 4, 5, AT))
        );
        assert_eq!(one_below.usage().work, 0);
        let mut midpoint = fresh_budget(5)?;
        assert_eq!(map.find("cxxx", 5, &mut midpoint, AT)?, Ok(1));
        assert_eq!(midpoint.usage().work, 5);
        let mut scalar_floor = fresh_budget(8)?;
        assert_eq!(map.find("dxxx", 8, &mut scalar_floor, AT)?, Err(2));
        assert_eq!(scalar_floor.usage().work, 8);
        let empty = RetainedMap::<String, u64>::default();
        let mut empty_budget = fresh_budget(5)?;
        assert_eq!(empty.find("dxxx", 5, &mut empty_budget, AT)?, Err(0));
        assert_eq!(empty_budget.usage().work, 5);

        // n=3, p=0: one forward entry and one inverse entry.
        let mut parent = words();
        let mut insertion = fresh_budget(2)?;
        let inverse = parent.prepare_insert(0, true, &mut insertion, AT)?;
        assert_eq!(inverse, 1);
        assert_eq!(insertion.usage().work, 2);
        parent.insert_prepared(0, "0xxx".into(), 0);
        assert_eq!(parent.len(), 4);
        // n=4 crosses the logical target to 8 without a global Work debit.
        let mut growth = fresh_budget(2)?;
        assert_eq!(parent.prepare_insert(0, true, &mut growth, AT)?, 1);
        assert_eq!(growth.usage().work, 2);
        let mut growth_below = fresh_budget(1)?;
        let before = parent.clone();
        assert_eq!(
            parent.prepare_insert(0, true, &mut growth_below, AT),
            Err(Fault::exhausted(BudgetKind::Work, 1, 2, AT))
        );
        assert_eq!(parent, before);

        // Every possible insertion position pays the same local entry tariff.
        for (position, key) in [(0, "0xxx"), (1, "bxxx"), (2, "dxxx"), (3, "zxxx")] {
            let mut entries = words();
            let mut exact = fresh_budget(3)?;
            exact.work(1, AT)?; // existing parent-child visit
            assert_eq!(entries.find(key, 0, &mut exact, AT)?, Err(position));
            entries.prepare_insert(position, true, &mut exact, AT)?;
            assert_eq!(exact.usage().work, 3); // 1 + 0 + 1 + 1
            entries.insert_prepared(position, key.into(), 7);
            assert_eq!(entries.keys().map(String::as_str).nth(position), Some(key));

            let mut refused = words();
            let mut short = fresh_budget(2)?;
            short.work(1, AT)?;
            assert_eq!(refused.find(key, 0, &mut short, AT)?, Err(position));
            assert_eq!(
                refused.prepare_insert(position, true, &mut short, AT),
                Err(Fault::exhausted(BudgetKind::Work, 2, 3, AT))
            );
            assert_eq!(refused, words());
        }

        let prefix = "a".repeat(256);
        let long_map: RetainedMap<String, u64> =
            [(format!("{prefix}x"), 1), (format!("{prefix}z"), 2)]
                .into_iter()
                .collect();
        let probe = format!("{prefix}y");
        let mut long_budget = fresh_budget(258)?;
        assert_eq!(long_map.find(&probe, 258, &mut long_budget, AT)?, Err(1));
        assert_eq!(long_budget.usage().work, 258);

        // The backing retained after undo can be reused with the same local tariff.
        assert_eq!(parent.remove_prepaid("0xxx"), Some(("0xxx".into(), 0)));
        let mut reuse = fresh_budget(2)?;
        assert_eq!(parent.find("0xxx", 0, &mut reuse, AT)?, Err(0));
        assert_eq!(parent.prepare_insert(0, true, &mut reuse, AT)?, 1);
        assert_eq!(reuse.usage().work, 2);
        Ok(())
    }

    /// Reverse work has already been charged and remains spent across commit or undo.
    /// ´test:unit:backend-rollback-work´
    #[test]
    fn backend_rollback_work() -> TestResult {
        let mut map = words();
        let mut exact = fresh_budget(2)?;
        let credit = map.prepare_insert(0, true, &mut exact, AT)?;
        assert_eq!(credit, 1);
        map.insert_prepared(0, "0xxx".into(), 0);
        assert_eq!(exact.usage().work, 2);
        assert_eq!(map.remove_prepaid("0xxx"), Some(("0xxx".into(), 0)));
        assert_eq!(exact.usage().work, 2);
        // Checkpoint-restored counters pay only their forward entry cost.
        let mut counter = words();
        let mut forward = fresh_budget(1)?;
        assert_eq!(counter.prepare_insert(0, false, &mut forward, AT)?, 0);
        assert_eq!(forward.usage().work, 1);

        let mut cas = Cas::new(Limits::default())?;
        let mut ledger = Budget::new(Limits::default())?;
        cas.transaction(&mut ledger, |_, _| Ok(()))?;
        assert_eq!(ledger.usage().work, 256);
        let prefix = cas
            .admit(
                &[Draft::Source {
                    key: "prefix".into(),
                    text: "first".into(),
                }],
                &mut ledger,
            )?
            .remove(0);
        let snapshot = cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        let fault = Fault::invalid(InvalidCode::Rule, AT);
        let before = ledger.usage().work;
        let mut discarded = None;
        let result: Result<(), Fault> = cas.transaction(&mut ledger, |cas, ledger| {
            cas.transaction(ledger, |cas, ledger| {
                discarded = cas
                    .admit(
                        &[Draft::Source {
                            key: "discarded".into(),
                            text: "inner".into(),
                        }],
                        ledger,
                    )?
                    .into_iter()
                    .next();
                Ok(())
            })?;
            Err(fault.clone())
        });
        assert_eq!(result, Err(fault.clone()));
        // Two 256-Work checkpoints and 666 Work of inner admission, including inverse credit.
        assert_eq!(ledger.usage().work - before, 1_178);
        assert_eq!(
            cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
            snapshot
        );
        assert_eq!(cas.source(&prefix, &mut ledger)?, ("prefix", "first"));
        let gap = discarded.ok_or("rolled-back identity")?;
        let before_commit = ledger.usage().work;
        let next = cas
            .admit(
                &[Draft::Source {
                    key: "next".into(),
                    text: "next".into(),
                }],
                &mut ledger,
            )?
            .remove(0);
        assert!(ledger.usage().work > before_commit);
        assert!(next.logical_id().serial > gap.logical_id().serial);
        assert_eq!(
            cas.state.indices.keys().copied().collect::<Vec<_>>(),
            vec![prefix.logical_id(), next.logical_id()]
        );

        let stable = cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        let spent = ledger.usage().work;
        let result: Result<(), Fault> = cas.transaction(&mut ledger, |cas, ledger| {
            let inner: Result<(), Fault> = cas.transaction(ledger, |cas, ledger| {
                cas.admit(
                    &[Draft::Source {
                        key: "inner-refusal".into(),
                        text: "temporary".into(),
                    }],
                    ledger,
                )?;
                Err(fault.clone())
            });
            assert_eq!(inner, Err(fault.clone()));
            Ok(())
        });
        assert_eq!(result, Ok(()));
        // Two checkpoints and 718 Work of the refused inner admission remain spent.
        assert_eq!(ledger.usage().work - spent, 1_230);
        assert_eq!(
            cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
            stable
        );
        Ok(())
    }
}
