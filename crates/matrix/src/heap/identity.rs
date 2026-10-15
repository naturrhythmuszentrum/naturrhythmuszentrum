// SPDX-License-Identifier: AGPL-3.0-only
//! Process-local identities and sealed, opaque descriptor locators.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use nrz_scalar::Scalar;

use super::HeapError;

/// Next unallocated process-local identity; exhaustion never wraps.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a scope or value identity with no persistence claim.
pub(crate) fn identity() -> Result<u64, HeapError> {
    allocate_identity(&NEXT_ID)
}

/// The isolated counter parameter permits exhaustion tests without global mutation.
fn allocate_identity(counter: &AtomicU64) -> Result<u64, HeapError> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .map_err(|_| HeapError::CounterExhausted)
}

/// Closed set of descriptor categories.
pub trait Category: sealed::Sealed + Copy + std::fmt::Debug {}

/// Prevent caller-defined categories.
mod sealed {
    /// Private marker implemented only by this crate's categories.
    pub trait Sealed {}
}

/// Define one opaque category marker.
macro_rules! category {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[expect(
            clippy::empty_enums,
            reason = "an uninhabited marker exists only at the type level"
        )]
        pub enum $name {}
        impl sealed::Sealed for $name {}
        impl Category for $name {}
    };
}

category!(Value, "A published value region.");
category!(Companion, "An independently described companion region.");
category!(
    Factor,
    "A factor resource category, reserved for the solve lanes."
);
category!(
    Workspace,
    "An unpublished workspace category, reserved for operation lanes."
);

/// Slot and occupation generation, independent of physical storage location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Key {
    /// Position in the owner's descriptor or arena table.
    pub(crate) slot: usize,
    /// Occupation number, never reused in that slot.
    pub(crate) generation: u64,
}

/// Copyable locator; it neither lends a permanent borrow nor captures current evidence.
#[derive(Clone, Copy, Debug)]
pub struct Handle<C: Category, P: Scalar> {
    /// Allocating heap identity.
    pub(crate) scope: u64,
    /// Descriptor occupation.
    pub(crate) key: Key,
    /// Sealed category and scalar format.
    marker: PhantomData<(C, P)>,
}

impl<C: Category, P: Scalar> PartialEq for Handle<C, P> {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope && self.key == other.key
    }
}

impl<C: Category, P: Scalar> Eq for Handle<C, P> {}

impl<C: Category, P: Scalar> Handle<C, P> {
    /// Construct only after the heap has prepared a complete publication.
    pub(crate) const fn new(scope: u64, key: Key) -> Self {
        Self {
            scope,
            key,
            marker: PhantomData,
        }
    }
}

/// Identity of a value, independent of schema equality and descriptor occupation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueId(
    /// Checked process-local allocation, never reused by the identity allocator.
    pub(crate) u64,
);

/// Content version within one surviving value occupation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueVersion(
    /// Monotonic counter within one surviving value occupation.
    pub(crate) u64,
);

impl ValueVersion {
    /// Monotonic content version, starting at one for a fresh value.
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }
}

/// Immutable dependency snapshot; every companion access rechecks it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueStamp {
    /// Mathematical value identity.
    pub(crate) id: ValueId,
    /// Content version.
    pub(crate) version: ValueVersion,
}

impl ValueStamp {
    /// Value identity; equal bytes and schemas do not merge it.
    #[must_use]
    pub const fn id(self) -> ValueId {
        self.id
    }

    /// Version captured by this snapshot.
    #[must_use]
    pub const fn version(self) -> ValueVersion {
        self.version
    }
}

/// Immutable schema identity scoped to one heap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaId {
    /// Pool owner.
    pub(crate) scope: u64,
    /// Index in the immutable pool.
    pub(crate) index: usize,
}

/// Inaccessible identity exhaustion invariants.
#[cfg(test)]
mod tests {
    use super::{AtomicU64, HeapError, Ordering, allocate_identity};

    /// The final identity is followed by explicit exhaustion, never a reused scope.
    /// ´test:unit:identity-exhaustion-never-wraps´
    #[test]
    fn identity_exhaustion_never_wraps() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate_identity(&counter), Ok(u64::MAX - 1));
        assert_eq!(
            allocate_identity(&counter),
            Err(HeapError::CounterExhausted)
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
