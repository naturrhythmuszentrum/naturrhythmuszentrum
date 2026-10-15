// SPDX-License-Identifier: AGPL-3.0-only
//! Exact descriptor maps cross to numerical clients only as bounded constant definitions.

mod maps;
mod model;

pub use model::{CheckedMap, ConstantKind, DescriptorConstant, MapIdentity, MapKind};

use crate::{SnapshotError, UnitError};

/// Checked descriptor-map construction failures, without any numerical evaluation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MapError {
    /// Source, target, origin or reference descriptors do not admit the map.
    #[error(transparent)]
    Units(#[from] UnitError),
    /// Exact descriptor arithmetic or constant export exhausted its bounded ledger.
    #[error(transparent)]
    Algebra(#[from] nrz_cas::Fault),
    /// Map identity belongs to a different frozen semantic state.
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    /// A reference or origin must be a source-free declared exact constant.
    #[error("map reference or origin is not a declared exact constant: {0}")]
    ConstantDeclaration(String),
    /// Difference references must be strictly positive and match the value kind.
    #[error("normalization requires a compatible strictly positive difference reference")]
    Reference,
    /// Points require a named point origin; differences refuse one.
    #[error("normalization has a missing, extra or incompatible named origin")]
    Origin,
}
