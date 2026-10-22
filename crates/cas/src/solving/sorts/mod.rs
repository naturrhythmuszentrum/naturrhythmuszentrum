// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar-space registrations and exact structural admission, independent of producer scheduling.
//!
//! # Scalar spaces, routing and Real theorem surface
//!
//! The scalar-space registry admits versioned `scalar.real` and `scalar.complex` nodes. Real and Complex expression ambients and coordinate slots retain exact node references; `Field` remains declaration syntax. `Cas::scalar_ambient` exposes the checked identity. A Real value in its own space needs no placement case. The sole directed `placement.real-in-complex` definition has local incidence `Im(q)=0` and `Re(q)=t` with exact Complex-query and Real-value bindings. A placement supplies neither a reverse path nor inherited laws, membership or coverage. Missing registrations and missing edges have distinct named refusals.
//!
//! `ParameterBase`, `SpaceFamily`, `FamilyExpression`, `FibrePiece`, `RealCell` and `CoefficientEvaluation` have current structural codecs. A total `FamilyRegion` retains its exact earlier family and base stratum, and native arms pair every original local operand with its placement piece. Family argument roles preserve original declarations. A symbolic cell is separate from the retained rational interval on a root selection; endpoint syntax proves no order or root existence. Coefficient evaluation has complete same-sort slot bindings and explicit localization, separately from narrow exact-Free coefficient maps. Proof subtypes 18–21 and statement subtypes 20–23 encode complete evaluation, Real sign, Real root-count and cell-existence payloads. The finite evaluation checker and the three Real theorem checkers are available. Real signs replay finite ordered-field derivations with strict definedness. Real root counts check square-free reduction, signed Euclidean chains, exact endpoint and infinity signs, disjoint cells and both exterior counts; count-one cells supply exact isolated existence and uniqueness. Automatic Real production follows the degree ladder and all three signs of the computed depressed-cubic D=(q/2)²+(p/3)³. Repeated-root factors retain their multiplicities; the negative-discriminant quadratic returns an empty set, and the zero polynomial retains its full identity family. `RealExact` checks the full Real-only proof dependency closure. A cell selection is never `Radicals`; a Real radicals request on the three-root stratum retains checked coverage at `AnyExact` and reports `RequestedRealRadicalsUnavailable`. A reachable Complex original declaration, including one hidden by real-part syntax or cancellation, returns `RealProblemHasComplexSymbol` before orbit work.
//!
//! Before a solving executor runs, `sort_routing` traverses the complete original semantic graph, including guards, domains, denominators, attached root relations, coefficient maps, family links and inactive source clauses. A metered header index resolves used declarations by their introducing header; unused declarations do not enter support. The resulting request-local descriptor records the Real, Complex or family route without supplying proof authority. A Real unknown using an original Complex declaration returns `RealProblemHasComplexSymbol` before coefficient, GCD, orbit or tower work. Retained same-sort requests continue to their existing producers and precise capability boundaries. Symbol, binder and coordinate-slot contradictions name `SymbolSortMismatch`; stored ambient contradictions name `AmbientSpaceMismatch`.
//!
//! `resolve_scalar_ambient` checks an operator, its unchanged operands and an optional requested space. Real-only arithmetic resolves to Real, mixed Real/Complex arithmetic resolves to Complex, and every cross-space operand retains its exact direct registered placement requirement. An absent edge returns `SpacePlacementUnavailable`; an undecided ambient returns `AmbientSpaceAmbiguous`. The resolver supplies structural requirements for native family cases and never supplies a cover, transferred law or theorem.

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
