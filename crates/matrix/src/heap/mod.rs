// SPDX-License-Identifier: AGPL-3.0-only
//! Homogeneous descriptor ownership and checked publication.

mod aliases;
mod error;
mod extent;
pub(crate) mod identity;
pub(crate) mod lifecycle;
mod limits;
pub(crate) mod publication;
pub(crate) mod records;
mod relocation;
mod scatter;
pub(crate) mod schema;
pub(crate) mod slots;
mod views;

pub use error::HeapError;
pub use extent::Extent;
pub use identity::{
    Category, Companion, Factor, Handle, SchemaId, Value, ValueId, ValueStamp, ValueVersion,
    Workspace,
};
pub use limits::HeapLimits;
pub use records::{Coverage, PublishedBundle};
pub use views::{CompanionView, RegionView, ValueView};

use crate::storage::Storage;
use records::Record;
use schema::Schema;
use slots::Slots;

/// Single numerical owner of precision-separated faer arenas and descriptor slots.
///
/// Imports state represented-only or declared-bound coverage explicitly. This subset
/// stores coherent homogeneous regions, not checked linear operators or solve results.
#[derive(Debug)]
pub struct Heap {
    /// Unique process-local scope; a foreign handle is refused before indexing.
    pub(crate) scope: u64,
    /// Recoverable resource policy.
    pub(crate) limits: HeapLimits,
    /// Typed owned numerical allocations.
    pub(crate) storage: Storage,
    /// Category-bearing descriptor occupations.
    pub(crate) descriptors: Slots<Record>,
    /// Immutable schemas interned using exact descriptor equality.
    pub(crate) schemas: Vec<Schema>,
    /// Bounded operation records and source dependency graph.
    pub(crate) provenance: crate::provenance::Provenance,
    /// Immutable source relations and independent statistical companion regions.
    pub(crate) joint: crate::companions::JointStore,
    /// Source-preserving propagated statistical parameter relations.
    pub(crate) statistics: crate::statistics::Store,
}

impl Heap {
    /// Create an empty owner with explicit capacities.
    ///
    /// # Errors
    /// Returns `HeapError::CounterExhausted` if no new process-local scope exists.
    pub fn new(limits: HeapLimits) -> Result<Self, HeapError> {
        Ok(Self {
            scope: identity::identity()?,
            limits,
            storage: Storage::new(),
            descriptors: Slots::new(),
            schemas: Vec::new(),
            provenance: crate::provenance::Provenance::new(),
            joint: crate::companions::JointStore::default(),
            statistics: crate::statistics::Store::default(),
        })
    }
}
