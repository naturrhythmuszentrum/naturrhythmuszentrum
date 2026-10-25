// SPDX-License-Identifier: AGPL-3.0-only
//! Physical readings over exact CAS coordinate families, without numerical payloads.
//!
//! Definitions retain selected copies and all expansion rows. Relations are checked
//! on the full free-coordinate family and replayed against a reconstructed judgment.
//! Physical meaning is checked before zero coefficients can discard coordinates.

mod companions;
mod family;
mod grid;
mod maps;
mod meaning;
mod normalization;
mod operations;
mod phasor;
mod projection;
mod reciprocal;
mod relation;
mod representation;
mod roots;
mod shape;
mod sources;
mod spectral;
mod support;
mod value;

pub use companions::{
    CompanionParameters, CompanionReading, CompanionReadingError, CompanionReadingKind,
    CompanionRelation, ParameterErrorReading,
};
pub use family::CoordinateFamily;
pub use grid::IndexedReading;
pub use meaning::EntryMeaning;
pub use normalization::{CoordinateDirection, CoordinateImage, CoordinateNormalization};
/// Exact algebra construction API used by these physical adapters.
pub use nrz_cas as algebra;
pub use operations::{
    AffineOperation, LinearOperator, OperatorApplication, OperatorCoefficient,
    ReadingOperationError,
};
pub use phasor::{
    PhasorBlockCorrespondence, PhasorConjugation, PhasorConvention, PhasorCovarianceLaw,
    PhasorResidualNorm, PhasorResidualTransfer, PhasorResultDefinition, PhasorSolveDefinition,
};
pub use projection::{ProjectionSpace, ReadingProjection};
pub use reciprocal::{
    PositiveReciprocalMap, ReciprocalAllocation, ReciprocalComponent, ReciprocalEntry,
    ReciprocalJacobian, ReciprocalJacobianCache, ReciprocalJacobianKey, ReciprocalMapDefinition,
    ReciprocalStage, SourceJacobianUsage,
};
pub use relation::ReadingRelation;
pub use representation::{Representation, RepresentationDefinition, RepresentationKind};
pub use roots::{RootCoverage, RootProblem, RootReplayUsage};
pub use shape::{ReadingAxis, ReadingShape};
pub use sources::{JointSources, ReadingSource, SourceEdge, SourceError, SourceKnowledge};
pub use spectral::{LcRootJacobian, LcSpectralLaw, LcSpectralResultDefinition, SpectralLawError};
pub use value::{PhysicalReading, ReadingDefinition};

/// A refused physical reading or exact relation.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ReadingError {
    /// A required declaration, original definition or scope is missing.
    #[error("physical reading requires its exact declaration and context")]
    Definition,
    /// Ordered coordinate counts or literal shapes disagree.
    #[error("physical reading shape mismatch")]
    Shape,
    /// Kind, measure, role, reference or origin disagrees.
    #[error("physical reading meaning mismatch")]
    Meaning,
    /// Affine point and difference roles disagree.
    #[error("physical reading role mismatch")]
    Role,
    /// Exact entry measures disagree.
    #[error("physical reading measure mismatch")]
    Measure,
    /// Exact source-free reference identities disagree.
    #[error("physical reading reference mismatch")]
    Reference,
    /// Exact affine origin identities disagree.
    #[error("physical reading origin mismatch")]
    Origin,
    /// Ordered partitions omit, repeat or reorder coordinates.
    #[error("physical reading partition mismatch")]
    Partition,
    /// The endpoints do not have the same exact family and scope.
    #[error("unrelated physical readings")]
    Unrelated,
    /// A claimed overlap has different targets or ordered projections.
    #[error("physical reading relation target mismatch")]
    Relation,
    /// An unsupported representation needs its own checked definition.
    #[error("unsupported physical reading contract")]
    Unsupported,
    /// Exact descriptor admission refused.
    #[error("physical descriptor: {0}")]
    Units(#[from] crate::UnitError),
    /// Exact reference or origin construction refused.
    #[error("physical coordinate map: {0}")]
    Map(#[from] crate::MapError),
    /// CAS construction, checking or resource accounting refused.
    #[error("reading algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}
