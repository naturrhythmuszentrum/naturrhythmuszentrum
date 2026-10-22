// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar-space registrations and exact structural admission, independent of producer scheduling.

mod mapping;
mod model;
mod proofs;
mod tags;
mod wire;

pub use model::{
    CoefficientEvaluationData, EvaluationBinding, EvaluationTarget, FamilyArgument,
    FamilyArgumentRole, FamilyExpressionData, FibrePieceData, LiesOver, ParameterBaseData,
    PlacementUse, RealCellData, RealEndpoint, ScalarPieceData, ScalarSpaceData, SpaceFamilyData,
    SpacePlacementData,
};
pub use proofs::{
    CertificateCoefficientEvaluation, CertificateRealCellExists, CertificateRealRootCount,
    CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep,
    StatementCoefficientEvaluation, StatementRealCellExists, StatementRealRootCount,
    StatementRealSign,
};

mod admission;
mod registry;
mod structure;
pub(crate) use admission::{selection_cell, validate};
pub(crate) use registry::{ambient, field};
pub(crate) use structure::{edges, metadata, statement};

mod incidence;
mod projection;
pub(crate) use projection::projection;

mod arms;
pub(crate) use arms::{arm, placement_operands};

mod ambient_resolution;
mod routing;
mod routing_index;
mod routing_shapes;
pub use ambient_resolution::{AmbientPlacement, AmbientResolution};
pub(crate) use routing::preflight;
pub use routing::{ScalarRoute, SortRouting};
