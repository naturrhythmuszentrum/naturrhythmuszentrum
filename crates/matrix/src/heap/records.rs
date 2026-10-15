// SPDX-License-Identifier: AGPL-3.0-only
//! Region-level value and companion records.

use nrz_scalar::Scalar;

use super::identity::Key;
use super::{Companion, Handle, Value, ValueStamp};
use crate::Layout;
use crate::storage::Region;

/// Meaning of the import's deterministic coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    /// Exact represented coordinates only; no physical-input bound is asserted.
    RepresentedInputs,
    /// Caller-declared componentwise absolute bounds, retained as input assumptions.
    DeclaredBounds,
}

/// Category-specific dependency edges; companions are never scalar fields.
#[derive(Debug)]
pub(crate) enum Meaning {
    /// A value and its complete supported companion set.
    Value {
        /// Independent identity and content version.
        stamp: ValueStamp,
        /// Reverse dependency edge to the componentwise bound, when declared.
        bound: Option<Key>,
    },
    /// A selection that resolves directly through its surviving base occupation.
    Alias {
        /// Exact base value occupation; aliases never form chains.
        base: Key,
    },
    /// An absolute deterministic bound on the entire logical target region.
    Bound {
        /// Exact target descriptor occupation.
        target: Key,
        /// Captured target identity and version.
        stamp: ValueStamp,
    },
}

/// Physical ownership or indirection through the base value descriptor.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Location {
    /// An independently owned numerical allocation.
    Owned(Region),
    /// A checked alias of an owning value descriptor.
    Alias,
}

/// A single descriptor slot over its own storage region and schema.
#[derive(Debug)]
pub(crate) struct Record {
    /// Index in the immutable exact-equality schema pool.
    pub(crate) schema: usize,
    /// Physical indirection into an arena of the recorded format.
    pub(crate) location: Location,
    /// Logical entry map into the owning region.
    pub(crate) layout: Layout,
    /// Category, coverage and versioned dependency relation.
    pub(crate) meaning: Meaning,
}

/// Atomic import result; its handles resolve through the heap on every access.
#[derive(Clone, Copy, Debug)]
pub struct PublishedBundle<P: Scalar> {
    /// Published value occupation.
    pub(crate) value: Handle<Value, P>,
    /// Independently allocated and described bound occupation.
    pub(crate) bound: Option<Handle<Companion, P>>,
}

impl<P: Scalar> PublishedBundle<P> {
    /// The imported value handle.
    #[must_use]
    pub const fn value(self) -> Handle<Value, P> {
        self.value
    }

    /// The declared componentwise bound handle, if requested at import.
    #[must_use]
    pub const fn bound(self) -> Option<Handle<Companion, P>> {
        self.bound
    }
}
