// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical region ownership, retained schema adapters and checked publication.
//!
//! # Numerical ownership and implemented facade
//!
//! `nrz-matrix` owns numerical regions in separate binary32 and binary64 faer arenas. A scalar is a 1×1 region. Values and declared absolute bounds have separate storage regions and bindings descriptors; every bound names the exact identity and version of its entire target region. Heap coordinates never enter CAS arithmetic. The facade still retains homogeneous descriptor schemas; `docs/design/descriptor-binding.md` places all meaning in checked bindings and specifies the remaining migration. See `docs/design/matrix-core-design.md` for numerical contracts.
//!
//! The prototype `DenseSystem`, bare `certified_solve` and mutable-field solver reports remain retired. Square solving is available through the checked operation and transaction boundary documented in `src/solve/` and `src/binding/`; historical feasibility records do not supply its certificates.
//!
//! The implemented homogeneous operation subset supports checked copy, homogeneous affine conversions and the deterministic kernels documented in `src/kernels/`, plus joint source declarations and covariance admission. Execution of mixed physical entries remains unavailable through this facade; float storage itself imposes no physical homogeneity principle. `Factor` and `Workspace` are category markers only. Numerical conversions use scalar outward enclosures and accuracy nonnegative bounds.

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
/// admits coherent homogeneous imports and publishes the supported checked operation results.
/// Descriptor records are retained facade metadata pending the binding migration.
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
    /// Opaque symbol inventory for migrated value and deterministic companion regions.
    pub(crate) symbols: crate::binding::symbols::Owners,
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
            symbols: crate::binding::symbols::Owners::default(),
            joint: crate::companions::JointStore::default(),
            statistics: crate::statistics::Store::default(),
        })
    }
}
