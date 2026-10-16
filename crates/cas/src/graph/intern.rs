// SPDX-License-Identifier: AGPL-3.0-only
//! Actual structural-index comparisons, independent of admission budgets and rollback.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::LogicalId;

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

/// A retained key owns its existing bytes and shares the existing store-token allocation.
/// A probe borrows bytes, allocating and cloning nothing. Only retained keys have an owner.
#[derive(Clone)]
struct Key<'a> {
    /// Unchanged structural bytes, borrowed for lookup and owned for retention/removal.
    bytes: Cow<'a, [u8]>,
    /// Diagnostic destination; ignored by every ordering/equality decision.
    owner: Option<Arc<StoreToken>>,
}

impl fmt::Debug for Key<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // State diagnostics describe retained keys, not cumulative work that survives rollback.
        self.bytes.fmt(formatter)
    }
}

impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut compared_bytes = 0_u64;
        let mut order = Ordering::Equal;
        for (left, right) in self.bytes.iter().zip(other.bytes.iter()) {
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

/// The production ordered index; all probes use the same instrumented total order.
#[derive(Default)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct InternIndex {
    /// Retained keys are always owned; borrowing the map can shorten the probe lifetime.
    entries: BTreeMap<Key<'static>, LogicalId>,
}

impl fmt::Debug for InternIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.entries.fmt(formatter)
    }
}

impl InternIndex {
    /// Probe without copying key bytes or allocating a diagnostic owner.
    pub(crate) fn get<'a>(&'a self, bytes: &'a [u8]) -> Option<&'a LogicalId> {
        self.entries.get(&Key {
            bytes: Cow::Borrowed(bytes),
            owner: None,
        })
    }

    /// Move the existing key buffer into the map and share the existing token allocation.
    pub(crate) fn insert(&mut self, bytes: Vec<u8>, id: LogicalId, owner: &Arc<StoreToken>) {
        self.entries.insert(
            Key {
                bytes: Cow::Owned(bytes),
                owner: Some(Arc::clone(owner)),
            },
            id,
        );
    }

    /// Consume the journal's existing buffer; removal comparisons remain diagnostic work.
    pub(crate) fn remove(&mut self, bytes: Vec<u8>) {
        self.entries.remove(&Key {
            bytes: Cow::Owned(bytes),
            owner: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicU64, Cow, InterningSnapshot, Key, StoreToken};
    use std::sync::Arc;

    /// Empty, equal, differing and prefix keys retain byte order and count only examined pairs.
    /// ´test:unit:structural-key-order-and-prefix-counts´
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
                bytes: Cow::Owned(left.to_vec()),
                owner: Some(Arc::clone(&owner)),
            };
            let probe = Key {
                bytes: Cow::Borrowed(right),
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
    #[test]
    fn interning_diagnostics_saturate_without_changing_order() {
        let owner = Arc::new(StoreToken {
            comparisons: AtomicU64::new(u64::MAX - 1),
            compared_bytes: AtomicU64::new(u64::MAX - 1),
        });
        let retained = Key {
            bytes: Cow::Owned(vec![1, 2]),
            owner: Some(Arc::clone(&owner)),
        };
        let probe = Key {
            bytes: Cow::Borrowed(&[1, 3]),
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
