// SPDX-License-Identifier: AGPL-3.0-only
//! Closed homogeneous recipes and graph-backed value/companion declarations.
//!
//! # Homogeneous symbol admission and copy
//!
//! `HomogeneousRecipe::check` checks every original concrete entry under an explicit entry limit, retaining physical kind, coherent measure and difference role. Points, formal quotients and unequal kinds refuse even at equal dimensions; empty rectangles use ratio meaning. This is a closed copy recipe predicate, not a requirement on floating storage.
//!
//! `BindingSymbols::declare` prepares a homogeneous matrix-valued CAS symbol and optional deterministic-bound and numerical-error symbols before numerical allocation. Each `QuantitySymbol` retains its mathematical sort, physical descriptor, distinct `SymbolKind`, actual CAS node and graph owner. Both companions have the target's difference units, with separate `CompanionKind` values and checked ordered edges to the exact value symbol. These edges state obligations; they prove neither the supplied bound nor a numerical enclosure.
//!
//! The graph uses CAS free declarations and the closed `SymbolRelation` kinds `Copy`, `DeterministicBound` and `NumericalError`. Each association retains the new symbol followed by its exact source or target, even across independently admitted declaration headers. CAS checks distinct prior Free endpoints, closed scopes, Boolean result sort, exact result-header ownership, Real matrix sorts and companion extent agreement; bindings checks physical kinds, measures and relation inventory. These edges provide no equation, normalization, solving or numerical authority. Ordinary header-local function applications would accumulate and recompare every earlier declaration, so each bundle instead has a bounded independent header and fixed-width symbol names. The read-only `QuantitySymbol::cas` accessor permits inspection of the actual graph edges.
//!
//! `copy_result` derives fresh result and companion symbols before execution. `HomogeneousRecipe::check_copy` checks the input and result meanings, companion kinds and targets, and exact source identity. A copied selection retains the source symbol; the numerical owner must separately validate its ordered region selection, extents, live versions and complete companion inventory. Swapped companion roles, a foreign companion and a result derived from another source refuse. `check_copy` returns a sealed `CopyWitness` retaining both symbol inventories. `SymbolBinding<T>::check` joins that meaning to opaque numerical-region tickets and refuses missing or extra associations. Ticket equality supplies neither an entry-set judgment nor overlap compatibility; the numerical client checks actual coverage and the exact relations for every intersection. The numerical client must validate every ticket under its live execution lease. The symbols and tickets alone grant no numerical access, representation proof or source independence.
//!
//! Graph admission consumes one caller-supplied CAS `Budget` across all construction phases and uses its existing limits and charges. The fixed companion inventory has at most three symbols; homogeneous entry admission checks its full iterator length before and during traversal. Independent numerical imports require `check_import`; a derived result must be bound through its checked publication. Public tests in `tests/conformance/admission.rs` cover the exact entry and cumulative CAS work boundaries, with success at N and refusal at N minus one. This subset does not implement general shaped descriptors, covariance/source graphs or total ownership across a client's heap.

mod binding;
mod graph;
mod parameter_graph;
mod parameters;
mod recipe;
mod schema;
mod selected;
mod symbols;

pub use binding::SymbolBinding;
pub use parameters::{ParameterEntry, ParameterSymbol, ParameterTarget};
pub use recipe::{CopyWitness, HomogeneousRecipe};
pub use schema::SymbolSchema;
pub use symbols::{BindingSymbols, CompanionKind, QuantitySymbol, SymbolKind};

/// A refused exact symbol relation, independent of any numerical heap.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AdmissionError {
    /// The recipe requires concrete homogeneous coherent differences.
    #[error("recipe requires concrete homogeneous coherent differences")]
    UnsupportedRecipe,
    /// A declared value does not have the requested physical meaning.
    #[error("symbol meaning does not match the recipe")]
    ValueMismatch,
    /// A companion has the wrong kind, target, shape or difference measure.
    #[error("companion symbol does not match its declared target")]
    CompanionMismatch,
    /// The result does not derive from the specified source symbol.
    #[error("planned result does not match the copy source")]
    ResultMismatch,
    /// Required region associations are missing or extra.
    #[error("symbol binding region inventory is incomplete")]
    RegionInventory,
    /// The bounded entry traversal or symbol sequence cannot advance.
    #[error("binding admission resource limit exhausted")]
    ResourceLimit,
    /// Exact descriptor arithmetic refused the declaration.
    #[error("descriptor: {0}")]
    Units(#[from] crate::UnitError),
    /// CAS syntax admission or inspection refused the graph.
    #[error("symbol graph: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}
