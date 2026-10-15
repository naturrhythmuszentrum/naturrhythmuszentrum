// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit preflight capacities for the supported heap subset.

/// Capacity policy, checked before backend allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeapLimits {
    /// Maximum descriptor slots, including permanently exhausted slots.
    pub(crate) descriptors: usize,
    /// Maximum live scalar elements in each precision, including companions.
    pub(crate) elements: usize,
    /// Maximum immutable schemas retained for this heap's lifetime.
    pub(crate) schemas: usize,
}

impl HeapLimits {
    /// Set descriptor, per-precision element and immutable-schema capacities.
    /// Zero capacities are permitted and refuse the corresponding publication.
    #[must_use]
    pub const fn new(descriptors: usize, elements_per_precision: usize, schemas: usize) -> Self {
        Self {
            descriptors,
            elements: elements_per_precision,
            schemas,
        }
    }
}

impl Default for HeapLimits {
    fn default() -> Self {
        Self::new(4096, 1_048_576, 4096)
    }
}
