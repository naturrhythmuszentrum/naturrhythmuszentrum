// SPDX-License-Identifier: AGPL-3.0-only
//! Actual structural-index comparisons, independent of admission budgets and rollback.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::LogicalId;
use super::retained_map::RetainedMap;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};

mod key;
pub(crate) use key::StoredKey;

/// Borrowed probes and retained keys expose the same ordered canonical byte sequence.
#[derive(Clone)]
enum KeyBytes<'a> {
    /// Existing contiguous probes need no allocation or retained owner.
    Flat(Cow<'a, [u8]>),
    /// An owned index key or a borrowed probe of that same segmented representation.
    Retained(Cow<'a, StoredKey>),
}

impl KeyBytes<'_> {
    /// Borrow each segment without copying the immutable environment.
    fn parts(&self) -> [&[u8]; 3] {
        match self {
            Self::Flat(bytes) => [bytes, &[], &[]],
            Self::Retained(key) => key.parts(),
        }
    }

    /// Canonical byte length, independent of how those bytes are retained.
    fn len(&self) -> usize {
        self.parts()
            .into_iter()
            .fold(0_usize, |length, part| length.saturating_add(part.len()))
    }
}

/// Cumulative diagnostic work at a quiescent store boundary, never a budget charge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct InterningSnapshot {
    /// Calls to the structural-key comparator, including unsuccessful operations.
    /// A value of `u64::MAX` denotes saturation rather than wrapping or refusal.
    pub(crate) comparisons: u64,
    /// Byte pairs examined, including the first unequal pair, counted once per position.
    /// Equal/prefix keys examine their common length; length ordering adds no bytes.
    /// A value of `u64::MAX` denotes saturation rather than wrapping or refusal.
    pub(crate) compared_bytes: u64,
}

/// Existing runtime identity allocation also owns the nonsemantic diagnostic counters.
/// Its address and counters never participate in key ordering or canonical encoding.
#[derive(Debug, Default)]
pub(crate) struct StoreToken {
    /// Logical identities belong to this token even when records are rolled back.
    pub(super) identities: super::identity::Identities,
    /// Monotone number of actual comparator calls.
    comparisons: AtomicU64,
    /// Monotone number of byte pairs examined by those calls.
    compared_bytes: AtomicU64,
}

impl StoreToken {
    /// Read without resetting; exact interval snapshots bracket completed store operations.
    pub(crate) fn snapshot(&self) -> InterningSnapshot {
        InterningSnapshot {
            comparisons: self.comparisons.load(AtomicOrdering::Relaxed),
            compared_bytes: self.compared_bytes.load(AtomicOrdering::Relaxed),
        }
    }

    /// Diagnostics saturate instead of changing an admission decision on overflow.
    fn record(&self, compared_bytes: u64) {
        for (counter, increment) in [
            (&self.comparisons, 1),
            (&self.compared_bytes, compared_bytes),
        ] {
            let _previous =
                counter.try_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |value| {
                    Some(value.saturating_add(increment))
                });
        }
    }
}

/// A retained key owns ordinary bytes or a shared signature, plus the existing store token.
/// A probe borrows bytes, allocating and cloning nothing. Only retained keys have an owner.
#[derive(Clone)]
struct Key<'a> {
    /// Unchanged structural bytes, borrowed for lookup and owned for retention/removal.
    bytes: KeyBytes<'a>,
    /// Diagnostic destination; ignored by every ordering/equality decision.
    owner: Option<Arc<StoreToken>>,
}

impl fmt::Debug for Key<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // State diagnostics describe retained keys, not cumulative work that survives rollback.
        formatter
            .debug_list()
            .entries(self.bytes.parts().into_iter().flatten())
            .finish()
    }
}

impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut compared_bytes = 0_u64;
        let mut order = Ordering::Equal;
        for (left, right) in self
            .bytes
            .parts()
            .into_iter()
            .flatten()
            .zip(other.bytes.parts().into_iter().flatten())
        {
            compared_bytes = compared_bytes.saturating_add(1);
            order = left.cmp(right);
            if order != Ordering::Equal {
                break;
            }
        }
        if order == Ordering::Equal {
            order = self.bytes.len().cmp(&other.bytes.len());
        }
        // Every index comparison has a retained key, including borrowed lookup probes.
        if let Some(owner) = self.owner.as_ref().or(other.owner.as_ref()) {
            owner.record(compared_bytes);
        }
        order
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

/// The production sorted index; all probes use the same instrumented total order.
#[derive(Default)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct InternIndex {
    /// Retained keys are always owned; borrowed probes use the same complete order.
    entries: RetainedMap<Key<'static>, LogicalId>,
}

impl fmt::Debug for InternIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.entries.fmt(formatter)
    }
}

impl InternIndex {
    /// Count retained complete keys without probing the map.
    #[cfg(test)]
    pub(crate) const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Test the actual retained allocation, not an equivalent local reconstruction.
    #[cfg(test)]
    pub(crate) fn shares_key(&self, id: LogicalId, bytes: &[u8]) -> bool {
        self.entries
            .iter()
            .any(|(key, value)| *value == id && std::ptr::eq(key.bytes.parts()[0], bytes))
    }

    /// Locate a complete retained key using its prepaid comparison floor.
    fn search(
        &self,
        probe: &Key<'_>,
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Result<usize, usize>, Fault> {
        self.entries
            .find_by(|candidate| candidate.cmp(probe), floor, budget, at)
    }

    /// Probe without copying key bytes or allocating a diagnostic owner.
    pub(crate) fn get<'a>(
        &'a self,
        bytes: &'a [u8],
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Option<&'a LogicalId>, Fault> {
        let probe = Key {
            bytes: KeyBytes::Flat(Cow::Borrowed(bytes)),
            owner: None,
        };
        Ok(self
            .search(&probe, floor, budget, at)?
            .ok()
            .and_then(|position| self.entries.at(position).map(|(_, value)| value)))
    }

    /// Return the first search position so admission can insert without comparing again.
    pub(crate) fn find_key(
        &self,
        bytes: &StoredKey,
        floor: u64,
        budget: &mut Budget,
        at: Location,
    ) -> Result<Result<usize, usize>, Fault> {
        let probe = Key {
            bytes: KeyBytes::Retained(Cow::Borrowed(bytes)),
            owner: None,
        };
        self.search(&probe, floor, budget, at)
    }

    /// Return the retained logical id at a searched position.
    pub(crate) fn value_at(&self, position: usize) -> Option<LogicalId> {
        self.entries.at(position).map(|(_, id)| *id)
    }

    /// Reserve insertion and any journaled inverse at a known position.
    pub(crate) fn prepare_insert_at(
        &mut self,
        position: usize,
        journaled: bool,
        budget: &mut Budget,
        at: Location,
    ) -> Result<u64, Fault> {
        self.entries.prepare_insert(position, journaled, budget, at)
    }

    /// Reserve the former lookup floor and local entry and inverse Work before graph publication.
    pub(crate) fn prepare_insert_key(
        &mut self,
        bytes: &StoredKey,
        journaled: bool,
        budget: &mut Budget,
        at: Location,
    ) -> Result<(usize, u64), Fault> {
        let probe = Key {
            bytes: KeyBytes::Retained(Cow::Borrowed(bytes)),
            owner: None,
        };
        let Err(position) = self.search(&probe, 0, budget, at)? else {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
        };
        let inverse = self.prepare_insert_at(position, journaled, budget, at)?;
        Ok((position, inverse))
    }

    /// Plan a staged contiguous key after earlier keys in the same unpublished batch.
    pub(crate) fn prepare_virtual_insert_bytes<'a>(
        &mut self,
        bytes: &[u8],
        earlier: impl Iterator<Item = &'a [u8]>,
        journaled: bool,
        budget: &mut Budget,
        at: Location,
    ) -> Result<usize, Fault> {
        let probe = Key {
            bytes: KeyBytes::Flat(Cow::Borrowed(bytes)),
            owner: None,
        };
        let Err(base) = self.search(&probe, 0, budget, at)? else {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
        };
        let mut before = 0_usize;
        let mut prior = 0_usize;
        for key in earlier {
            prior = prior
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
            let staged = Key {
                bytes: KeyBytes::Flat(Cow::Borrowed(key)),
                owner: None,
            };
            match staged.cmp(&probe) {
                Ordering::Less => {
                    before = before
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
                }
                Ordering::Equal => {
                    return Err(Fault::invalid(crate::error::InvalidCode::Reference, at));
                }
                Ordering::Greater => {}
            }
        }
        let position = base
            .checked_add(before)
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        self.entries.prepare_virtual_insert(
            self.entries
                .len()
                .checked_add(prior)
                .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?,
            position,
            journaled,
            budget,
            at,
        )?;
        Ok(position)
    }

    /// Move the preflighted key into its prepared sorted slot without another comparison.
    pub(crate) fn insert_prepared_key(
        &mut self,
        position: usize,
        bytes: StoredKey,
        id: LogicalId,
        owner: &Arc<StoreToken>,
    ) {
        self.entries.insert_prepared(
            position,
            Key {
                bytes: KeyBytes::Retained(Cow::Owned(bytes)),
                owner: Some(Arc::clone(owner)),
            },
            id,
        );
    }

    /// Consume the journal's existing key using its prepaid inverse entry credit.
    pub(crate) fn remove_prepaid(&mut self, bytes: StoredKey) {
        let probe = Key {
            bytes: KeyBytes::Retained(Cow::Owned(bytes)),
            owner: None,
        };
        let _removed = self
            .entries
            .remove_by_prepaid(|candidate| candidate.cmp(&probe));
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicU64, Cow, InterningSnapshot, Key, KeyBytes, StoreToken};
    use std::sync::Arc;

    /// Empty, equal, differing and prefix keys retain byte order and count only examined pairs.
    /// ´test:unit:structural-key-order-and-prefix-counts´
    /// ´claim:cas:structural-key-order-and-prefix-counts´ Structural keys preserve ordinary byte ordering and count each compared byte pair once.
    #[test]
    fn structural_key_order_and_prefix_counts() {
        let examples: &[(&[u8], &[u8], u64)] = &[
            (&[], &[], 0),
            (&[], &[1], 0),
            (&[1], &[1], 1),
            (&[0, 2], &[1, 2], 1),
            (&[1, 2, 0], &[1, 3, 0], 2),
            (&[1, 2], &[1, 2, 0], 2),
        ];
        for &(left, right, compared_bytes) in examples {
            let owner = Arc::new(StoreToken::default());
            let retained = Key {
                bytes: KeyBytes::Flat(Cow::Owned(left.to_vec())),
                owner: Some(Arc::clone(&owner)),
            };
            let probe = Key {
                bytes: KeyBytes::Flat(Cow::Borrowed(right)),
                owner: None,
            };
            assert_eq!(retained.cmp(&probe), left.cmp(right));
            assert_eq!(
                owner.snapshot(),
                InterningSnapshot {
                    comparisons: 1,
                    compared_bytes
                }
            );
            assert_eq!(probe.cmp(&retained), right.cmp(left));
            assert_eq!(
                owner.snapshot(),
                InterningSnapshot {
                    comparisons: 2,
                    compared_bytes: 2 * compared_bytes,
                }
            );
        }
    }

    /// Counter overflow neither wraps nor affects the total byte order.
    /// ´test:unit:interning-diagnostics-saturate-without-changing-order´
    /// ´claim:cas:interning-diagnostics-saturate-without-changing-order´ Saturated diagnostic counters preserve the same structural key ordering.
    #[test]
    fn interning_diagnostics_saturate_without_changing_order() {
        let owner = Arc::new(StoreToken {
            comparisons: AtomicU64::new(u64::MAX - 1),
            compared_bytes: AtomicU64::new(u64::MAX - 1),
            ..StoreToken::default()
        });
        let retained = Key {
            bytes: KeyBytes::Flat(Cow::Owned(vec![1, 2])),
            owner: Some(Arc::clone(&owner)),
        };
        let probe = Key {
            bytes: KeyBytes::Flat(Cow::Borrowed(&[1, 3])),
            owner: None,
        };
        assert!(retained < probe);
        assert!(probe > retained);
        assert_eq!(
            owner.snapshot(),
            InterningSnapshot {
                comparisons: u64::MAX,
                compared_bytes: u64::MAX,
            }
        );
    }
}
