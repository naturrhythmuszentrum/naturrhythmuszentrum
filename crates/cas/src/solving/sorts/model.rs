// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit scalar spaces, placements, parameter families and finite evaluation syntax.

use crate::graph::{Handle, Reference};
use crate::model::{Operator, Scope, Sort};

/// A closed specification identity; its name never supplies caller-defined laws.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarSpaceData {
    /// Registered grammar, signatures, predicates and law inventory.
    pub specification: String,
    /// Exact specification version.
    pub version: u32,
}

/// One explicitly registered directed placement, with no inferred path composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpacePlacementData<R = Reference> {
    /// Original local scalar space.
    pub source: R,
    /// Exact ambient scalar space.
    pub ambient: R,
    /// Closed piece and operation-agreement definition.
    pub specification: String,
    /// Exact placement-definition version.
    pub version: u32,
}

/// A structural instance of a placement piece, without membership or cover evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarPieceData<R = Reference> {
    /// Earlier placement definition.
    pub placement: R,
    /// Piece in that definition's finite vocabulary.
    pub piece: u32,
    /// Exact region of ambient query locations.
    pub region: R,
    /// Earlier incidence predicate carrying the exact local binding scope.
    pub local_frame: R,
}

/// An original Real parameter product and its exact domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterBaseData<R = Reference> {
    /// Original immutable declaration header.
    pub header: R,
    /// Ordered original Real coefficient coordinates, possibly empty.
    pub coordinates: R,
    /// Exact parameter domain, without a nonemptiness claim.
    pub domain: R,
}

/// A fibre over the original parameter base; the problem owns its later total region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpaceFamilyData<R = Reference> {
    /// Exact parameter base.
    pub base: R,
    /// Registered fibre scalar space.
    pub fibre_space: R,
    /// Ordered fibre coefficient declarations.
    pub coefficients: R,
    /// Ordered original unknown declarations.
    pub unknowns: R,
}

/// Exact dependency role in a local family operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyArgumentRole {
    /// A value in the original Real base.
    Base,
    /// A value in the declared fibre space.
    Fibre,
    /// An earlier local expression in this same family.
    Family,
}

/// One role stays paired with its exact original operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyArgument<R = Reference> {
    /// Checked local role, never a conversion instruction.
    pub role: FamilyArgumentRole,
    /// Earlier original operand or local family body.
    pub value: R,
}

/// A local operator body; native cases retain its placement support.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyExpressionData<R = Reference> {
    /// Exact parameter/fibre family.
    pub family: R,
    /// Checked scalar result ambient.
    pub ambient: R,
    /// Active total coefficient piece.
    pub region: R,
    /// Explicit local binder scope.
    pub scope: Scope,
    /// Registered local operator signature.
    pub operator: Operator,
    /// One exact result category and shape.
    pub result: Sort,
    /// Ordered role/operand pairs.
    pub arguments: Vec<FamilyArgument<R>>,
}

/// A regional solution proposal; completeness and enclosing covers are separate applications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FibrePieceData<R = Reference> {
    /// Exact family.
    pub family: R,
    /// Original Real base stratum.
    pub stratum: R,
    /// Exact total coefficient region lying over the stratum.
    pub region: R,
    /// Original problem, retaining all source dependencies.
    pub problem: R,
    /// Regional solution proposal.
    pub set: R,
}

/// A finite Real endpoint or signed infinity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealEndpoint<R = Reference> {
    /// Unbounded below, always open.
    NegativeInfinity,
    /// Earlier independently defined Real expression.
    RealExpression(R),
    /// Unbounded above, always open.
    PositiveInfinity,
}

/// Structural cell syntax; order, endpoint definedness and root counts require evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealCellData<R = Reference> {
    /// Original header.
    pub header: R,
    /// Exact binder scope.
    pub scope: Scope,
    /// Coefficient stratum on which endpoints are interpreted.
    pub region: R,
    /// Left endpoint.
    pub left: RealEndpoint<R>,
    /// Right endpoint.
    pub right: RealEndpoint<R>,
    /// Whether the finite left endpoint is excluded.
    pub left_open: bool,
    /// Whether the finite right endpoint is excluded.
    pub right_open: bool,
}

/// Exact target of a finite, same-sort coefficient evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationTarget<R = Reference> {
    /// Ordinary expressions in one declared scalar space.
    Scalar {
        /// Exact immutable target header.
        header: R,
        /// Registered target space.
        space: R,
    },
    /// Local expressions retaining their original base and fibre roles.
    Family {
        /// Exact family.
        family: R,
    },
}

/// One ordered source slot and its same-sort image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationBinding<R = Reference> {
    /// Position in the original coordinate frame.
    pub slot: u32,
    /// Earlier expression in exactly that slot's declared space.
    pub image: R,
}

/// Finite specialization syntax, separate from narrow exact-Free interpretation maps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientEvaluationData<R = Reference> {
    /// Original ordered coordinate space.
    pub source: R,
    /// Exact scalar or family target.
    pub target: EvaluationTarget<R>,
    /// Exact evaluation region.
    pub region: R,
    /// Complete ordered source-slot bindings.
    pub bindings: Vec<EvaluationBinding<R>>,
    /// Finite localization polynomials; no structural nonvanishing assertion.
    pub localization: Vec<R>,
}

/// Original operand and placement instance remain one inseparable association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementUse<R = Reference> {
    /// Original local operand, retaining its declaration space.
    pub operand: R,
    /// Exact placement piece and incidence frame.
    pub piece: R,
}

/// The exact family and base stratum of a total region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiesOver<R = Handle> {
    /// Earlier family, which does not point to this total region.
    pub family: R,
    /// Earlier base stratum.
    pub stratum: R,
}
