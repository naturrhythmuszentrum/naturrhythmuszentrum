// SPDX-License-Identifier: AGPL-3.0-only
//! Closed homogeneous recipes and graph-backed value/companion declarations.

mod binding;
mod graph;
mod recipe;
mod symbols;

pub use binding::SymbolBinding;
pub use recipe::{CopyWitness, HomogeneousRecipe};
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
    /// Required region tickets are missing, extra or multiply owned.
    #[error("symbol binding region inventory is incomplete or repeats a region")]
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
