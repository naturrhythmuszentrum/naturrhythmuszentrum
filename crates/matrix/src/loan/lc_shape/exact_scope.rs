// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed exact-owner observations are not another physical reservation.
//! No aggregate total is exposed: the named exclusions have no capacity inventory.

use super::{LcInputInventory, LcInputShape};
use nrz_bindings::{LcForwardCover, LcRelation, Registry};
use nrz_scalar::Scalar;

/// An original scope for which complete backing capacity is unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exclusion {
    /// Original symbolic stores, beyond their existing logical allowances.
    SymbolicStores,
    /// Original exact provenance and its retained dependencies.
    Provenance,
    /// Schema and symbol pools, including the input's binding symbols.
    SchemaAndSymbolPools,
    /// Original logical binding registries.
    LogicalBindings,
    /// Semantic Registry measure definitions, including `Registry.full_cycle`.
    RegistryMeasures,
    /// Semantic Registry name-to-dimension map.
    RegistryNames,
    /// Semantic Registry binary and power rules.
    RegistryRules,
    /// Semantic Registry scale roots; operation checks are not a whole-store inventory.
    RegistryScaleRoots,
}

/// Deliberate exact scopes plus the separately named semantic Registry exclusion.
const EXCLUSIONS: [Exclusion; 8] = [
    Exclusion::SymbolicStores,
    Exclusion::Provenance,
    Exclusion::SchemaAndSymbolPools,
    Exclusion::LogicalBindings,
    Exclusion::RegistryMeasures,
    Exclusion::RegistryNames,
    Exclusion::RegistryRules,
    Exclusion::RegistryScaleRoots,
];

/// Distinct units must not be added into a purported complete physical total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Measure {
    /// Existing owning-layer conservative logical retained allowance, when available.
    pub logical_allowance_bytes: Option<u64>,
    /// Size in bytes of the borrowed Rust record only, not its transitive backing allocations.
    /// An inventory contains its schema records. Question/relation sizes are per-use Rc
    /// handle sizes: the question handle lies in its schema, and the relation handle in
    /// the cover's shared allocation. These observations overlap their containing owners.
    /// Deduplicating shared-allocation identity does not deduplicate per-use handles.
    pub inline_record_size: usize,
    /// Complete physical backing capacity is unavailable for all these original scopes.
    pub backing_capacity_bytes: Option<usize>,
}

/// A live identity witness; no raw address escapes and no original owner is cloned.
#[derive(Debug)]
enum Owner<'i, P: Scalar, const N: usize> {
    /// Identity of the borrowed fixed inventory record, not a claimed heap allocation.
    Inventory(&'i LcInputInventory<P, N>),
    /// Identity of one inline schema record, already contained in its inventory.
    Schema(&'i LcInputShape<P>),
    /// Bindings supplies shared-allocation identity; the inline figure is one per-use
    /// Rc handle inside the schema record, not the shared allocation's size.
    /// Shared identity deduplication does not deduplicate these per-use handles.
    Question(&'i LcForwardCover),
    /// The original relation has independent shared-allocation identity; its inline
    /// figure is the per-use Rc handle inside the cover's shared allocation.
    /// Shared identity deduplication does not deduplicate this handle observation.
    Relation(&'i LcRelation),
    /// Borrowed Registry object identity, without a claim about its child allocations.
    Registry(&'i Registry),
}

/// One scoped observation keeps its identity valid for the original borrow lifetime.
#[derive(Debug)]
pub struct Observation<'i, P: Scalar, const N: usize> {
    /// Opaque identity distinguishes sharing from equal contents.
    owner: Owner<'i, P, N>,
    /// Separately named units; None means unavailable, never zero.
    pub measure: Measure,
}
impl<P: Scalar, const N: usize> Observation<'_, P, N> {
    /// Compare only the same owner kind, using original owning-layer identity interfaces.
    /// This is a fixed read-only comparison, not an allocation or a reservation handoff.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        match (&self.owner, &other.owner) {
            (Owner::Inventory(left), Owner::Inventory(right)) => std::ptr::eq(*left, *right),
            (Owner::Schema(left), Owner::Schema(right)) => std::ptr::eq(*left, *right),
            (Owner::Question(left), Owner::Question(right)) => left.shares_allocation(right),
            (Owner::Relation(left), Owner::Relation(right)) => left.shares_allocation(right),
            (Owner::Registry(left), Owner::Registry(right)) => std::ptr::eq(*left, *right),
            _ => false,
        }
    }
}

/// Every positional use remains discoverable even when its question/relation is shared.
#[derive(Debug)]
pub struct Input<'i, P: Scalar, const N: usize> {
    /// Original supplied-constraint index, not an index after deduplication.
    pub position: usize,
    /// Inline schema and its unmeasured symbol/provenance dependencies.
    pub schema: Observation<'i, P, N>,
    /// New forward-cover logical allowance excludes the shared original relation.
    pub question: Observation<'i, P, N>,
    /// Original relation logical allowance remains separate from the forward cover.
    pub relation: Observation<'i, P, N>,
}

/// Allocation-free view of the caller's originals beside, never inside, the physical plan.
///
/// Fixed observations use existing uncharged identity/retained-byte accessors and sizeof;
/// there is no hidden traversal, serialization, reservation or mutation of a caller ledger.
/// A later graph traversal retains its own record-visit charges. Borrowing this view adds
/// no retaining edge, capacity or cleanup obligation. No complete total is available.
#[derive(Debug)]
pub struct ExactScope<'i, P: Scalar, const N: usize> {
    /// The caller keeps every schema and its shared exact authority alive for this borrow.
    inventory: &'i LcInputInventory<P, N>,
    /// The semantic Registry is borrowed for that same lifetime, never reconstructed.
    registry: &'i Registry,
}
impl<'i, P: Scalar, const N: usize> ExactScope<'i, P, N> {
    /// Bind the report to existing originals, without taking ownership or allocating.
    pub(crate) const fn new(inventory: &'i LcInputInventory<P, N>, registry: &'i Registry) -> Self {
        Self {
            inventory,
            registry,
        }
    }

    /// Named unavailable scopes belong to this report, not a separately imported inventory.
    #[must_use]
    pub const fn exclusions() -> [Exclusion; 8] {
        EXCLUSIONS
    }

    /// Finite positional enumeration uses the original fixed inventory extent.
    #[must_use]
    pub const fn input_count() -> usize {
        N
    }

    /// The complete inline array is one borrowed record, with unknown symbolic backing.
    #[must_use]
    pub const fn inventory(&self) -> Observation<'i, P, N> {
        Observation {
            owner: Owner::Inventory(self.inventory),
            measure: measure(self.inventory, None),
        }
    }

    /// Object/header observation does not count any Registry child as physically covered.
    #[must_use]
    pub const fn registry(&self) -> Observation<'i, P, N> {
        Observation {
            owner: Owner::Registry(self.registry),
            measure: measure(self.registry, None),
        }
    }

    /// Return one original use; out-of-range does not fabricate an empty owner.
    #[must_use]
    pub fn input(&self, index: usize) -> Option<Input<'i, P, N>> {
        let schema = self.inventory.input(index)?;
        let question = schema.question();
        let relation = question.relation();
        Some(Input {
            position: schema.input_index(),
            schema: Observation {
                owner: Owner::Schema(schema),
                measure: measure(schema, None),
            },
            question: Observation {
                owner: Owner::Question(question),
                measure: measure(question, Some(question.retained_payload_bytes())),
            },
            relation: Observation {
                owner: Owner::Relation(relation),
                measure: measure(relation, Some(relation.retained_payload_bytes())),
            },
        })
    }
}

/// A known inline record is not evidence of a complete transitive capacity inventory.
const fn measure<T>(record: &T, logical_allowance_bytes: Option<u64>) -> Measure {
    Measure {
        logical_allowance_bytes,
        inline_record_size: size_of_val(record),
        backing_capacity_bytes: None,
    }
}
