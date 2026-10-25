// SPDX-License-Identifier: AGPL-3.0-only
//! Exact descriptor maps cross to numerical clients only as bounded constant definitions.
//!
//! # Exact map definitions and exports
//!
//! `CheckedDeclarations::conversion_map` checks both descriptor schemas and constructs a private `CheckedMap` for exact measure conversion. `normalization_map` instead resolves a strictly positive exact difference reference from `Declaration::Constant`, and requires a compatible named exact point origin for a point value. Runtime inputs, derived observables and nonpositive references refuse; counts and unnamed relative values cannot enter normalization. Difference maps have zero shifts, and `inverse` restores the same reference and origin. Reference and origin identity remains distinct even when their exact coordinates coincide.
//!
//! A `MapIdentity` retains the frozen snapshot, source and target descriptors, map purpose/direction and exact named reference/origin definitions. `DescriptorConstant` retains that identity and its scale or shift role. Its bounded `export` method produces an integer or one literal fraction only when the exact coefficient is rational, with explicit output-byte and arithmetic/work refusals. `exact` exposes the retained CAS constant. Non-rational coefficients return `UnitError::UnsupportedExactConstantExport`; the same named refusal protects rational-only exports, native sensitivity schedules and the version-two docgen schema. `enclosure_export` and `ExactEnclosureExport` separately admit the bounded rational/pi grammar for numerical owners retaining the original typed definition. A numerical decoder encloses that spelling once per map, precision and arithmetic revision. There is no map evaluator accepting stored coordinates, no scalar or matrix dependency, and no numerical value sent to CAS. Exact scales and offsets, numerical conversion error and physical source uncertainty remain separate (see `docs/design/matrix-core-design.md`).
//!
//! `ReferenceDefinition` and `OriginDefinition` also carry immutable source-free schema definitions without adding declarations or changing a frozen snapshot. `ReferenceDefinition::unit` defines one unit of an exact difference measure; `new` permits an explicit checked positive finite exact coordinate, including a CAS constant expression, and refuses unchecked text or unproved positivity. `OriginDefinition::new` defines the zero coordinate of a point measure. Reuse these objects when adapting matrix reference and point-origin definitions: cloning preserves identity, while independently constructing even identical labels and coordinates creates distinct identities. `reference_definition` and `origin_definition` resolve existing frozen constants instead and retain their exact snapshot authority.
//!
//! `CheckedDeclarations::schema_normalization_map` exports a scalar map from those definitions with explicit arithmetic limits. For named definitions it returns exactly the same map as `normalization_map`. `quotient_normalization_map` accepts an entry's bindings `PairingWitness` plus its ordered row and column reference definitions, checks both original descriptors, and exports the coherent physical map with scale `column/row`. Both exact coordinates and measure scales participate. An optional compatible point origin is admitted only with ratio columns and affects centers; exporting the same quotient without an origin supplies its zero-shift difference map. The inverse restores the same definitions, and its shift supplies the coherent exact origin for a matrix RHS correction. Matrix retains correction placement, signs and operator multiplication; bindings performs none of those numerical operations.
//!
//! `MapIdentity::source` and `target` expose `MapCoordinates`: `descriptor` returns a scalar descriptor when present, and `quotient` returns the formal ordered pairing otherwise. Formal coordinates are coherent and retain their explicit point or difference role; they never acquire an invented scalar kind. Map identity includes both reference definitions, the origin and the frozen owner. A definition resolved in another snapshot refuses, even at identical coordinates. Schema coefficient arithmetic uses one caller-limited ledger; existing descriptor validation retains its own bounded checks, and exact string export retains its separate byte/work budget. No matrix dependency, heap data or numerical evaluation is introduced.

mod coordinates;
mod definitions;
mod enclosure;
mod maps;
mod model;
mod schema;

pub use coordinates::MapCoordinates;
pub use definitions::{OriginDefinition, ReferenceDefinition};
pub use enclosure::{ConstantEnclosureExport, ExactEnclosureExport, PiFactor};
pub use model::{CheckedMap, ConstantKind, DescriptorConstant, MapIdentity, MapKind};

use crate::{SnapshotError, UnitError};

/// Checked descriptor-map construction failures, without any numerical evaluation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MapError {
    /// Ordered formal quotient targets do not match the supplied exact definitions.
    #[error(transparent)]
    Pairing(#[from] crate::PairingError),
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
