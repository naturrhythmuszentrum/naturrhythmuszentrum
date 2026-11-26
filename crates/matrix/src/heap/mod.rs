// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical region ownership, retained schema adapters and checked publication.
//!
//! # Numerical ownership and implemented facade
//!
//! `nrz-matrix` owns numerical regions in separate binary32 and binary64 faer arenas. A scalar is a 1×1 region. Values and declared absolute bounds have separate storage regions and bindings descriptors; every bound names the exact identity and version of its entire target region. Heap coordinates never enter CAS arithmetic. The descriptor facade retains separate homogeneous interpretations; `docs/design/descriptor-binding.md` places all meaning in checked bindings and specifies the remaining migration. See `docs/design/matrix-core-design.md` for numerical contracts.
//!
//! The prototype `DenseSystem`, bare `certified_solve` and mutable-field solver reports remain retired. Square solving is available through the checked operation and transaction boundary documented in `src/solve/` and `src/binding/`; historical feasibility records do not supply its certificates.
//!
//! The implemented homogeneous operation subset supports checked copy, homogeneous affine conversions and the deterministic kernels documented in `src/kernels/`, plus joint source declarations and covariance admission. Execution of mixed physical entries remains unavailable through this facade; float storage itself imposes no physical homogeneity principle. `Factor` and `Workspace` are category markers only. Numerical conversions use scalar outward enclosures and accuracy nonnegative bounds.

mod aliases;
pub(crate) mod descriptors;
mod error;
mod extent;
pub(crate) mod identity;
pub(crate) mod lifecycle;
mod limits;
pub(crate) mod publication;
pub(crate) mod records;
mod relocation;
pub(crate) mod resources;
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
use descriptors::Descriptors;
use schema::Schema;

/// Single numerical owner of precision-separated faer arenas and descriptor slots.
///
/// Imports state represented-only or declared-bound coverage explicitly. This subset
/// admits coherent homogeneous imports and publishes the supported checked operation results.
/// Descriptor-side interpretation records are opaque to numerical storage.
pub struct Heap {
    /// Temporary bounded graph observer; ordinary native calls retain their existing scope.
    pub(crate) graph_gate: Option<std::rc::Rc<crate::loan::graph::adapter::gate::Gate>>,
    /// Unique process-local scope; a foreign handle is refused before indexing.
    pub(crate) scope: u64,
    /// Recoverable resource policy.
    pub(crate) limits: HeapLimits,
    /// Typed owned numerical allocations.
    pub(crate) storage: Storage,
    /// Independently owned packed masks and their immutable native cell layouts.
    pub(crate) masks: crate::mask::Store,
    /// Managed owners, conservative debits and weak use indexes.
    pub(crate) loans: crate::loan::Registry,
    /// Category-bearing descriptor occupations.
    pub(crate) descriptors: Descriptors,
    /// Immutable numerical shapes; descriptor-side equality selects their opaque identities.
    pub(crate) schemas: Vec<Schema>,
    /// Opaque descriptor-side interpretations, never inspected by numerical storage.
    pub(crate) meanings: Vec<crate::binding::schema::BindingSchema>,
    /// Bounded operation records and source dependency graph.
    pub(crate) provenance: crate::provenance::Provenance,
    /// Opaque symbol inventory for migrated value and deterministic companion regions.
    pub(crate) symbols: crate::binding::symbols::Inventory,
    /// Immutable source relations and independent statistical companion regions.
    pub(crate) joint: crate::companions::JointStore,
    /// Source-preserving propagated statistical parameter relations.
    pub(crate) statistics: crate::statistics::Store,
    /// Completed execution resource leases, separate from immutable operation history.
    pub(crate) executions: crate::execution::Registry,
}

impl std::fmt::Debug for Heap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Heap")
            .field("scope", &self.scope)
            .field("limits", &self.limits)
            .field("epoch", &self.storage.epoch())
            .finish_non_exhaustive()
    }
}

impl Heap {
    /// Create an empty owner with explicit capacities.
    ///
    /// # Errors
    /// Returns `HeapError::CounterExhausted` if no new process-local scope exists.
    pub fn new(limits: HeapLimits) -> Result<Self, HeapError> {
        Ok(Self {
            graph_gate: None,
            scope: identity::identity()?,
            limits,
            storage: Storage::new(),
            masks: crate::mask::Store::new(),
            loans: crate::loan::Registry::new(),
            descriptors: Descriptors::new(),
            schemas: Vec::new(),
            meanings: Vec::new(),
            provenance: crate::provenance::Provenance::new(),
            symbols: crate::binding::symbols::Inventory::default(),
            joint: crate::companions::JointStore::default(),
            statistics: crate::statistics::Store::default(),
            executions: crate::execution::Registry::default(),
        })
    }
}
