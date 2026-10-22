// SPDX-License-Identifier: AGPL-3.0-only
//! Shared bounded payload encodings; all reference identities are supplied by the caller.
use super::sorts::{
    CertificateCoefficientEvaluation, CertificateRealCellExists, CertificateRealRootCount,
    CertificateRealSign, CoefficientEvaluationData, FamilyExpressionData, FibrePieceData,
    ParameterBaseData, RealCellData, ScalarPieceData, ScalarSpaceData, SpaceFamilyData,
    SpacePlacementData, StatementCoefficientEvaluation, StatementRealCellExists,
    StatementRealRootCount, StatementRealSign,
};

use super::model::{
    AdjunctionMode, BinomialMode, CertificateBinomialCoverage, CertificateCoefficientIdentity,
    CertificateComponentSplit, CertificateConjugationTransport, CertificateExactLinearEquivalence,
    CertificateExtensionUnit, CertificateImplicitFunctionBranch, CertificateMembership,
    CertificateMultiplicityTransfer, CertificateParameterImageCoverage,
    CertificatePermutationOrbitCoverage, CertificateProductCompleteness,
    CertificateRealIsolationAndOrder, CertificateRootAdjunction,
    CertificateRootSelectionRefinement, CertificateSolutionTransport,
    CertificateTotalPredicateSplit, CoefficientDomainData, CoefficientMapData,
    CoefficientValueData, CoefficientValueObject, ComponentSplitData, Coordinate, CoordinateRole,
    CoordinateSpaceData, ExclusionPolicy, ExtensionLevelData, FamilyMapData, GeneratorPair,
    IdentityMode, IsolationInterval, LocalBranchData, LocalMode, MapMode, MapTarget,
    MultiplicityMode, Neighborhood, PolynomialData, PolynomialTerm, PredicateSplitMode, PrimePower,
    RationalInterval, RefinementMode, RefinementTargetKind, RegularityKind, RelationMultiple,
    RootFactor, RootRelationData, RootSelectionData, RowOperation, ScopedCoefficientRelation,
    SelectionMode, SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveCertificate,
    SolveDraft, SolveForm, SolveGoal, SolveProblemData, SolveStatement, SolveSubstitution,
    SquareFreeRequest, StatementCoefficientEquality, StatementComponentCover, StatementCoverage,
    StatementEquivalent, StatementFamilyCoverage, StatementFamilyMembership, StatementIsolation,
    StatementLinearSystem, StatementLocalBranch, StatementLocalDerivative, StatementMember,
    StatementMultiplicity, StatementOrbit, StatementPresentation, StatementRank,
    StatementRealValues, StatementRootExistence, StatementRootOrder, StatementSelectionUnique,
    StatementUnit, TupleData,
};
use crate::encoding::{self, Encoder};
use crate::error::Fault;

impl CoordinateRole {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Coefficient => 0,
            Self::Unknown => 1,
            Self::Family => 2,
        }
    }
}

impl SquareFreeRequest {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::NotRequested => 0,
            Self::Required => 1,
        }
    }
}

impl MapMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Embedding => 0,
            Self::Specialization => 1,
        }
    }
}

impl SelectionMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Any => 0,
            Self::Constrained => 1,
            Self::Principal => 2,
            Self::Isolated => 3,
        }
    }
}

impl SolveGoal {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Candidates => 0,
            Self::Complete => 1,
        }
    }
}

impl SolveForm {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::AnyExact => 0,
            Self::AlgebraicRoots => 1,
            Self::Radicals => 2,
            Self::ExactFamily => 3,
            Self::LocalBranch => 4,
            Self::RealExact => 5,
        }
    }
}

impl ExclusionPolicy {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::PreserveOriginal => 0,
        }
    }
}

impl RegularityKind {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::RealC1 => 0,
            Self::Holomorphic => 1,
        }
    }
}

impl IdentityMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Normalize => 0,
            Self::RelationCombination => 1,
            Self::NonzeroPower(_) => 2,
        }
    }
}

impl AdjunctionMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::AlgebraicClosure => 0,
            Self::ComplexFTA => 1,
            Self::PrimitiveUnity => 2,
            Self::RealOddRoot => 3,
            Self::PositiveRealSquareRoot => 4,
        }
    }
}

impl PredicateSplitMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::ZeroNonzero => 0,
            Self::RealSigns => 1,
            Self::EliminateCover => 2,
        }
    }
}

impl BinomialMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Order => 0,
            Self::Coverage => 1,
            Self::ZeroRadicand => 2,
        }
    }
}

impl RefinementMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Existence => 0,
            Self::Unique => 1,
            Self::Presentation => 2,
        }
    }
}

impl RefinementTargetKind {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Selection => 0,
            Self::SolutionSet => 1,
        }
    }
}

impl MultiplicityMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Derivatives => 0,
            Self::SquareFreeLayers => 1,
        }
    }
}

impl LocalMode {
    /// Stable subtype byte, independent of Rust layout.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::PolynomialRegularity => 0,
            Self::OpenNeighborhood => 1,
            Self::ImplicitExistence => 2,
            Self::Derivative => 3,
        }
    }
}

impl Coordinate {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode<R>(
        &self,
        out: &mut Encoder<'_>,
        _write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.name(&self.key)?;
        encoding::field(&self.field, out)?;
        out.byte(self.role.tag())?;
        Ok(())
    }
}

impl RationalInterval {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode<R>(
        &self,
        out: &mut Encoder<'_>,
        _write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.number(&self.left)?;
        out.number(&self.right)?;
        Ok(())
    }
}

impl<R> CoordinateSpaceData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        out.count(self.scalar_spaces.len())?;
        for space in &self.scalar_spaces {
            out.byte(u8::from(space.is_some()))?;
            if let Some(space) = space {
                write(space, out)?;
            }
        }
        out.count(self.coordinates.len())?;
        for item0 in &self.coordinates {
            item0.encode(out, write)?;
        }
        Ok(())
    }
}

impl<R> TupleData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        write(&self.coordinates, out)?;
        out.count(self.entries.len())?;
        for item0 in &self.entries {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> SolutionSetData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        out.count(self.branches.len())?;
        for item0 in &self.branches {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> FamilyMapData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        write(&self.coefficients, out)?;
        write(&self.family, out)?;
        write(&self.region, out)?;
        write(&self.target, out)?;
        write(&self.body, out)?;
        Ok(())
    }
}

impl<R> PolynomialData<R> {
    /// Encode a fixed exponent width determined by the ordered variable frame.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.domain, out)?;
        out.count(self.variables.len())?;
        for field in &self.variables {
            encoding::field(field, out)?;
        }
        out.count(self.terms.len())?;
        for term in &self.terms {
            for exponent in &term.exponents {
                crate::wire::index(*exponent, out)?;
            }
            write(&term.coefficient, out)?;
        }
        Ok(())
    }
}

impl<R> ExtensionLevelData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.parent, out)?;
        write(&self.polynomial, out)?;
        out.count(self.generator.len())?;
        for item0 in &self.generator {
            encoding::atomic(item0, out)?;
        }
        out.byte(self.square_free.tag())?;
        Ok(())
    }
}

impl<R> CoefficientMapData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.source, out)?;
        self.target.encode(out, write)?;
        out.byte(self.mode.tag())?;
        write(&self.region, out)?;
        out.count(self.images.len())?;
        for item0 in &self.images {
            write(item0, out)?;
        }
        out.count(self.denominators.len())?;
        for item0 in &self.denominators {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> RootRelationData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        encoding::field(&self.ambient, out)?;
        write(&self.polynomial, out)?;
        out.count(self.binder.len())?;
        for item0 in &self.binder {
            encoding::atomic(item0, out)?;
        }
        write(&self.map, out)?;
        write(&self.region, out)?;
        out.byte(u8::from(self.multiplicity.is_some()))?;
        if let Some(item0) = &self.multiplicity {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> RootSelectionData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        out.count(self.relations.len())?;
        for item0 in &self.relations {
            write(item0, out)?;
        }
        write(&self.predicate, out)?;
        out.byte(self.mode.tag())?;
        out.byte(u8::from(self.convention.is_some()))?;
        if let Some(item0) = &self.convention {
            out.name(item0)?;
        }
        out.byte(u8::from(self.interval.is_some()))?;
        if let Some(item0) = &self.interval {
            item0.encode(out, write)?;
        }
        out.byte(u8::from(self.cell.is_some()))?;
        if let Some(cell) = &self.cell {
            write(cell, out)?;
        }
        Ok(())
    }
}

impl<R> ComponentSplitData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.parent, out)?;
        write(&self.left_factor, out)?;
        write(&self.right_factor, out)?;
        write(&self.left_domain, out)?;
        write(&self.right_domain, out)?;
        write(&self.left_map, out)?;
        write(&self.right_map, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CoefficientValueObject<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.domain, out)?;
        self.value.encode(out, write)?;
        Ok(())
    }
}

impl<R> SolutionBranchData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.region, out)?;
        write(&self.domain, out)?;
        self.kind.encode(out, write)?;
        Ok(())
    }
}

impl<R> SolveProblemData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        write(&self.predicate, out)?;
        write(&self.coefficients, out)?;
        write(&self.unknowns, out)?;
        write(&self.region, out)?;
        write(&self.domain, out)?;
        write(&self.coefficient_domain, out)?;
        encoding::field(&self.ambient, out)?;
        out.count(self.denominators.len())?;
        for item0 in &self.denominators {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> LocalBranchData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.base_coefficients, out)?;
        write(&self.base_unknowns, out)?;
        self.neighborhood.encode(out, write)?;
        out.name(&self.family_symbol.key)?;
        out.name(&self.family_symbol.display_name)?;
        encoding::sort(&self.family_symbol.sort, out)?;
        write(&self.jacobian, out)?;
        write(&self.inverse, out)?;
        Ok(())
    }
}

impl IsolationInterval {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode<R>(
        &self,
        out: &mut Encoder<'_>,
        _write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.number(&self.left)?;
        out.number(&self.right)?;
        crate::wire::index(self.left_variations, out)?;
        crate::wire::index(self.right_variations, out)?;
        crate::wire::index(self.count, out)?;
        Ok(())
    }
}

impl<R> RelationMultiple<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.multiplier, out)?;
        write(&self.relation, out)?;
        out.byte(u8::from(self.hypothesis.is_some()))?;
        if let Some(hypothesis) = &self.hypothesis {
            hypothesis.encode(out, write)?;
        }
        Ok(())
    }
}

impl<R> ScopedCoefficientRelation<R> {
    /// Encode the exact premise slot, endpoints and ordered interpretation.
    fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        crate::wire::index(self.premise, out)?;
        write(&self.left, out)?;
        write(&self.right, out)?;
        out.count(self.roots.len())?;
        for root in &self.roots {
            write(root, out)?;
        }
        out.byte(u8::from(self.vieta.is_some()))?;
        if let Some(relation) = &self.vieta {
            write(relation, out)?;
        }
        Ok(())
    }
}

impl<R> SolveSubstitution<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.name(&self.key)?;
        write(&self.expression, out)?;
        Ok(())
    }
}

impl PrimePower {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode<R>(
        &self,
        out: &mut Encoder<'_>,
        _write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        crate::wire::index(self.prime, out)?;
        crate::wire::index(self.exponent, out)?;
        Ok(())
    }
}

impl<R> RootFactor<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.root, out)?;
        crate::wire::index(self.multiplicity, out)?;
        Ok(())
    }
}

impl<R> GeneratorPair<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.original, out)?;
        write(&self.conjugate, out)?;
        Ok(())
    }
}

impl<R> StatementMember<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.tuple, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementEquivalent<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.source, out)?;
        write(&self.target, out)?;
        write(&self.forward, out)?;
        write(&self.backward, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementCoverage<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.set, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementFamilyMembership<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.family, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementFamilyCoverage<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.family, out)?;
        write(&self.recovery, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementRank<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.matrix, out)?;
        write(&self.domain, out)?;
        crate::wire::index(self.rank, out)?;
        out.count(self.pivots.len())?;
        for item0 in &self.pivots {
            crate::wire::index(*item0, out)?;
        }
        write(&self.minor, out)?;
        write(&self.elimination, out)?;
        Ok(())
    }
}

impl<R> StatementLinearSystem<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.reduced, out)?;
        crate::wire::index(self.rank, out)?;
        crate::wire::index(self.augmented_rank, out)?;
        write(&self.region, out)?;
        out.count(self.free_columns.len())?;
        for item0 in &self.free_columns {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> StatementLocalBranch<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.branch, out)?;
        out.byte(self.regularity.tag())?;
        self.neighborhood.encode(out, write)?;
        Ok(())
    }
}

impl<R> StatementLocalDerivative<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.branch, out)?;
        write(&self.derivative, out)?;
        self.neighborhood.encode(out, write)?;
        Ok(())
    }
}

impl<R> StatementCoefficientEquality<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.left, out)?;
        write(&self.right, out)?;
        write(&self.map, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementUnit<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.value, out)?;
        write(&self.inverse, out)?;
        write(&self.domain, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementComponentCover<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.split, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementRootExistence<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementSelectionUnique<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.selection, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementRootOrder<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.value, out)?;
        crate::wire::index(self.order, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementOrbit<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.seed, out)?;
        out.count(self.generators.len())?;
        for item0 in &self.generators {
            out.count(item0.len())?;
            for item1 in item0 {
                crate::wire::index(*item1, out)?;
            }
        }
        out.count(self.members.len())?;
        for item0 in &self.members {
            write(item0, out)?;
        }
        out.count(self.actions.len())?;
        for item0 in &self.actions {
            out.count(item0.len())?;
            for item1 in item0 {
                crate::wire::index(*item1, out)?;
            }
        }
        Ok(())
    }
}

impl<R> StatementMultiplicity<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.selection, out)?;
        out.count(self.multiplicities.len())?;
        for item0 in &self.multiplicities {
            crate::wire::index(*item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementRealValues<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.count(self.values.len())?;
        for item0 in &self.values {
            write(item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementIsolation<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        out.count(self.intervals.len())?;
        for item0 in &self.intervals {
            item0.encode(out, write)?;
        }
        crate::wire::index(self.exterior_count, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> StatementPresentation<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.set, out)?;
        out.byte(self.form.tag())?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificateCoefficientIdentity<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        if let IdentityMode::NonzeroPower(exponent) = self.mode {
            crate::wire::index(exponent, out)?;
        }
        write(&self.left, out)?;
        write(&self.right, out)?;
        write(&self.map, out)?;
        write(&self.region, out)?;
        out.count(self.combinations.len())?;
        for item0 in &self.combinations {
            item0.encode(out, write)?;
        }
        out.count(self.denominators.len())?;
        for item0 in &self.denominators {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateRootAdjunction<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        write(&self.relation, out)?;
        out.byte(u8::from(self.selection.is_some()))?;
        if let Some(item0) = &self.selection {
            write(item0, out)?;
        }
        crate::wire::index(self.leading_unit, out)?;
        out.byte(u8::from(self.order.is_some()))?;
        if let Some(item0) = &self.order {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.splitting_chain.len())?;
        for item0 in &self.splitting_chain {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateExtensionUnit<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.element, out)?;
        write(&self.modulus, out)?;
        write(&self.gcd, out)?;
        out.count(self.quotients.len())?;
        for item0 in &self.quotients {
            write(item0, out)?;
        }
        out.count(self.bezout.len())?;
        for item0 in &self.bezout {
            write(item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificateComponentSplit<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.split, out)?;
        crate::wire::index(self.product, out)?;
        out.count(self.bezout.len())?;
        for item0 in &self.bezout {
            write(item0, out)?;
        }
        crate::wire::index(self.square_free, out)?;
        out.count(self.selections.len())?;
        for item0 in &self.selections {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateMembership<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.tuple, out)?;
        write(&self.region, out)?;
        out.count(self.substitutions.len())?;
        for item0 in &self.substitutions {
            item0.encode(out, write)?;
        }
        out.count(self.definedness.len())?;
        for item0 in &self.definedness {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateSolutionTransport<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.source, out)?;
        write(&self.target, out)?;
        write(&self.forward, out)?;
        write(&self.backward, out)?;
        write(&self.region, out)?;
        out.count(self.definedness.len())?;
        for item0 in &self.definedness {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.equations.len())?;
        for item0 in &self.equations {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.forward_inverse.len())?;
        for item0 in &self.forward_inverse {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.backward_inverse.len())?;
        for item0 in &self.backward_inverse {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateTotalPredicateSplit<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        write(&self.expression, out)?;
        write(&self.region, out)?;
        out.count(self.children.len())?;
        for item0 in &self.children {
            write(item0, out)?;
        }
        out.count(self.branches.len())?;
        for item0 in &self.branches {
            crate::wire::index(*item0, out)?;
        }
        out.byte(u8::from(self.cover.is_some()))?;
        if let Some(item0) = &self.cover {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateBinomialCoverage<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        crate::wire::index(self.order, out)?;
        out.byte(u8::from(self.unity.is_some()))?;
        if let Some(item0) = &self.unity {
            write(item0, out)?;
        }
        out.byte(u8::from(self.selection.is_some()))?;
        if let Some(item0) = &self.selection {
            write(item0, out)?;
        }
        out.count(self.factors.len())?;
        for item0 in &self.factors {
            item0.encode(out, write)?;
        }
        out.count(self.exclusions.len())?;
        for item0 in &self.exclusions {
            crate::wire::index(*item0, out)?;
        }
        out.byte(u8::from(self.product.is_some()))?;
        if let Some(item0) = &self.product {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateRootSelectionRefinement<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        out.byte(self.target_kind.tag())?;
        write(&self.target, out)?;
        out.count(self.witnesses.len())?;
        for item0 in &self.witnesses {
            write(item0, out)?;
        }
        out.count(self.compatibility.len())?;
        for item0 in &self.compatibility {
            crate::wire::index(*item0, out)?;
        }
        out.byte(u8::from(self.form.is_some()))?;
        if let Some(item0) = &self.form {
            out.byte(item0.tag())?;
        }
        Ok(())
    }
}

impl<R> CertificateExactLinearEquivalence<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.matrix, out)?;
        crate::wire::index(self.augmented_columns, out)?;
        out.count(self.operations.len())?;
        for item0 in &self.operations {
            item0.encode(out, write)?;
        }
        out.count(self.pivots.len())?;
        for item0 in &self.pivots {
            crate::wire::index(*item0, out)?;
        }
        write(&self.minor, out)?;
        write(&self.reduced, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificateProductCompleteness<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.set, out)?;
        write(&self.scalar, out)?;
        out.count(self.factors.len())?;
        for item0 in &self.factors {
            item0.encode(out, write)?;
        }
        crate::wire::index(self.product, out)?;
        out.count(self.existence.len())?;
        for item0 in &self.existence {
            crate::wire::index(*item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificateMultiplicityTransfer<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        write(&self.problem, out)?;
        write(&self.selection, out)?;
        out.count(self.multiplicities.len())?;
        for item0 in &self.multiplicities {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.values.len())?;
        for item0 in &self.values {
            write(item0, out)?;
        }
        crate::wire::index(self.product, out)?;
        out.count(self.nonzero.len())?;
        for item0 in &self.nonzero {
            crate::wire::index(*item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificateConjugationTransport<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.map, out)?;
        write(&self.conjugate_map, out)?;
        out.count(self.images.len())?;
        for item0 in &self.images {
            item0.encode(out, write)?;
        }
        out.count(self.values.len())?;
        for item0 in &self.values {
            write(item0, out)?;
        }
        out.count(self.relations.len())?;
        for item0 in &self.relations {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.fixed_field.len())?;
        for item0 in &self.fixed_field {
            crate::wire::index(*item0, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl<R> CertificatePermutationOrbitCoverage<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.seed, out)?;
        crate::wire::index(self.degree, out)?;
        out.count(self.generators.len())?;
        for item0 in &self.generators {
            out.count(item0.len())?;
            for item1 in item0 {
                crate::wire::index(*item1, out)?;
            }
        }
        out.count(self.members.len())?;
        for item0 in &self.members {
            write(item0, out)?;
        }
        out.count(self.actions.len())?;
        for item0 in &self.actions {
            out.count(item0.len())?;
            for item1 in item0 {
                crate::wire::index(*item1, out)?;
            }
        }
        out.count(self.words.len())?;
        for item0 in &self.words {
            out.count(item0.len())?;
            for item1 in item0 {
                crate::wire::index(*item1, out)?;
            }
        }
        Ok(())
    }
}

impl<R> CertificateRealIsolationAndOrder<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        out.count(self.sturm.len())?;
        for item0 in &self.sturm {
            write(item0, out)?;
        }
        out.count(self.intervals.len())?;
        for item0 in &self.intervals {
            item0.encode(out, write)?;
        }
        crate::wire::index(self.exterior_left, out)?;
        crate::wire::index(self.exterior_right, out)?;
        out.count(self.multiplicity.len())?;
        for item0 in &self.multiplicity {
            crate::wire::index(*item0, out)?;
        }
        out.byte(u8::from(self.ordered_index.is_some()))?;
        if let Some(item0) = &self.ordered_index {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateParameterImageCoverage<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.problem, out)?;
        write(&self.family, out)?;
        out.byte(u8::from(self.recovery.is_some()))?;
        if let Some(item0) = &self.recovery {
            write(item0, out)?;
        }
        write(&self.region, out)?;
        out.count(self.membership.len())?;
        for item0 in &self.membership {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.recovery_proofs.len())?;
        for item0 in &self.recovery_proofs {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.domain.len())?;
        for item0 in &self.domain {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.scope.len())?;
        for item0 in &self.scope {
            crate::wire::index(*item0, out)?;
        }
        Ok(())
    }
}

impl<R> CertificateImplicitFunctionBranch<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.mode.tag())?;
        write(&self.branch, out)?;
        out.byte(self.regularity.tag())?;
        out.count(self.regularity_proofs.len())?;
        for item0 in &self.regularity_proofs {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.base_equations.len())?;
        for item0 in &self.base_equations {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.inverse.len())?;
        for item0 in &self.inverse {
            crate::wire::index(*item0, out)?;
        }
        out.count(self.derivatives.len())?;
        for item0 in &self.derivatives {
            crate::wire::index(*item0, out)?;
        }
        out.byte(u8::from(self.derivative.is_some()))?;
        if let Some(item0) = &self.derivative {
            write(item0, out)?;
        }
        Ok(())
    }
}

impl<R> CoefficientDomainData<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Rational => {
                out.byte(0)?;
            }
            Self::Gaussian => {
                out.byte(1)?;
            }
            Self::Fraction { base, coordinates } => {
                out.byte(2)?;
                write(base, out)?;
                write(coordinates, out)?;
            }
            Self::Quotient { level } => {
                out.byte(3)?;
                write(level, out)?;
            }
        }
        Ok(())
    }
}

impl<R> MapTarget<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Family { family } => {
                out.byte(2)?;
                write(family, out)?;
            }
            Self::CoefficientDomain { domain } => {
                out.byte(0)?;
                write(domain, out)?;
            }
            Self::ScalarAmbient { header, field } => {
                out.byte(1)?;
                write(header, out)?;
                encoding::field(field, out)?;
            }
        }
        Ok(())
    }
}

impl<R> CoefficientValueData<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Rational { value } => {
                out.byte(0)?;
                out.number(value)?;
            }
            Self::Gaussian { real, imaginary } => {
                out.byte(1)?;
                out.number(real)?;
                out.number(imaginary)?;
            }
            Self::Fraction {
                numerator,
                denominator,
            } => {
                out.byte(2)?;
                write(numerator, out)?;
                write(denominator, out)?;
            }
            Self::Tower { coefficients } => {
                out.byte(3)?;
                out.count(coefficients.len())?;
                for item0 in coefficients {
                    write(item0, out)?;
                }
            }
        }
        Ok(())
    }
}

impl<R> SolutionBranchKind<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Finite { tuples } => {
                out.byte(0)?;
                out.count(tuples.len())?;
                for item0 in tuples {
                    write(item0, out)?;
                }
            }
            Self::Image { family } => {
                out.byte(1)?;
                write(family, out)?;
            }
            Self::Algebraic { selection } => {
                out.byte(2)?;
                write(selection, out)?;
            }
            Self::Empty => {
                out.byte(3)?;
            }
            Self::Local { branch } => {
                out.byte(4)?;
                write(branch, out)?;
            }
        }
        Ok(())
    }
}

impl<R> Neighborhood<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Existential {
                coefficient_binder,
                unknown_binder,
            } => {
                out.byte(0)?;
                out.count(coefficient_binder.len())?;
                for item0 in coefficient_binder {
                    encoding::atomic(item0, out)?;
                }
                out.count(unknown_binder.len())?;
                for item0 in unknown_binder {
                    encoding::atomic(item0, out)?;
                }
            }
            Self::Explicit {
                coefficient_region,
                unknown_region,
            } => {
                out.byte(1)?;
                write(coefficient_region, out)?;
                write(unknown_region, out)?;
            }
        }
        Ok(())
    }
}

impl<R> RowOperation<R> {
    /// Encode the closed subtype and its ordered fields.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::Swap { left, right } => {
                out.byte(0)?;
                crate::wire::index(*left, out)?;
                crate::wire::index(*right, out)?;
            }
            Self::Scale { row, value, unit } => {
                out.byte(1)?;
                crate::wire::index(*row, out)?;
                write(value, out)?;
                crate::wire::index(*unit, out)?;
            }
            Self::AddMultiple {
                destination,
                source,
                value,
            } => {
                out.byte(2)?;
                crate::wire::index(*destination, out)?;
                crate::wire::index(*source, out)?;
                write(value, out)?;
            }
        }
        Ok(())
    }
}

impl<R> SolveDraft<R> {
    /// Encode the closed subtype and its complete ordered payload.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self {
            Self::CoordinateSpace(value) => value.encode(out, write),
            Self::Tuple(value) => value.encode(out, write),
            Self::SolutionSet(value) => value.encode(out, write),
            Self::FamilyMap(value) => value.encode(out, write),
            Self::CoefficientDomain(value) => value.encode(out, write),
            Self::Polynomial(value) => value.encode(out, write),
            Self::ExtensionLevel(value) => value.encode(out, write),
            Self::CoefficientMap(value) => value.encode(out, write),
            Self::RootRelation(value) => value.encode(out, write),
            Self::RootSelection(value) => value.encode(out, write),
            Self::ComponentSplit(value) => value.encode(out, write),
            Self::CoefficientValue(value) => value.encode(out, write),
            Self::SolutionBranch(value) => value.encode(out, write),
            Self::SolveProblem(value) => value.encode(out, write),
            Self::LocalBranch(value) => value.encode(out, write),
            Self::ParameterBase(value) => value.encode(out, write),
            Self::SpaceFamily(value) => value.encode(out, write),
            Self::FamilyExpression(value) => value.encode(out, write),
            Self::FibrePiece(value) => value.encode(out, write),
            Self::RealCell(value) => value.encode(out, write),
            Self::CoefficientEvaluation(value) => value.encode(out, write),
            Self::ScalarSpace(value) => value.encode(out, write),
            Self::SpacePlacement(value) => value.encode(out, write),
            Self::ScalarPiece(value) => value.encode(out, write),
        }
    }

    /// Stable subtype discriminator.
    pub(crate) const fn tag(&self) -> u8 {
        match self {
            Self::CoordinateSpace(_) => 0,
            Self::Tuple(_) => 1,
            Self::SolutionSet(_) => 2,
            Self::FamilyMap(_) => 3,
            Self::CoefficientDomain(_) => 4,
            Self::Polynomial(_) => 5,
            Self::ExtensionLevel(_) => 6,
            Self::CoefficientMap(_) => 7,
            Self::RootRelation(_) => 8,
            Self::RootSelection(_) => 9,
            Self::ComponentSplit(_) => 10,
            Self::CoefficientValue(_) => 11,
            Self::SolutionBranch(_) => 12,
            Self::SolveProblem(_) => 13,
            Self::LocalBranch(_) => 14,
            Self::ParameterBase(_) => 15,
            Self::SpaceFamily(_) => 16,
            Self::FamilyExpression(_) => 17,
            Self::FibrePiece(_) => 18,
            Self::RealCell(_) => 19,
            Self::CoefficientEvaluation(_) => 20,
            Self::ScalarSpace(_) => 21,
            Self::SpacePlacement(_) => 22,
            Self::ScalarPiece(_) => 23,
        }
    }
}

impl<R> SolveStatement<R> {
    /// Encode the closed subtype and its complete ordered payload.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.tag())?;
        match self {
            Self::Member(value) => value.encode(out, write),
            Self::Equivalent(value) => value.encode(out, write),
            Self::Coverage(value) => value.encode(out, write),
            Self::FamilyMembership(value) => value.encode(out, write),
            Self::FamilyCoverage(value) => value.encode(out, write),
            Self::Rank(value) => value.encode(out, write),
            Self::LinearSystem(value) => value.encode(out, write),
            Self::LocalBranch(value) => value.encode(out, write),
            Self::LocalDerivative(value) => value.encode(out, write),
            Self::CoefficientEquality(value) => value.encode(out, write),
            Self::Unit(value) => value.encode(out, write),
            Self::ComponentCover(value) => value.encode(out, write),
            Self::RootExistence(value) => value.encode(out, write),
            Self::SelectionUnique(value) => value.encode(out, write),
            Self::RootOrder(value) => value.encode(out, write),
            Self::Orbit(value) => value.encode(out, write),
            Self::Multiplicity(value) => value.encode(out, write),
            Self::RealValues(value) => value.encode(out, write),
            Self::Isolation(value) => value.encode(out, write),
            Self::Presentation(value) => value.encode(out, write),
            Self::CoefficientEvaluation(value) => value.encode(out, write),
            Self::RealSign(value) => value.encode(out, write),
            Self::RealRootCount(value) => value.encode(out, write),
            Self::RealCellExists(value) => value.encode(out, write),
        }
    }

    /// Stable subtype discriminator.
    pub(crate) const fn tag(&self) -> u8 {
        match self {
            Self::Member(_) => 0,
            Self::Equivalent(_) => 1,
            Self::Coverage(_) => 2,
            Self::FamilyMembership(_) => 3,
            Self::FamilyCoverage(_) => 4,
            Self::Rank(_) => 5,
            Self::LinearSystem(_) => 6,
            Self::LocalBranch(_) => 7,
            Self::LocalDerivative(_) => 8,
            Self::CoefficientEquality(_) => 9,
            Self::Unit(_) => 10,
            Self::ComponentCover(_) => 11,
            Self::RootExistence(_) => 12,
            Self::SelectionUnique(_) => 13,
            Self::RootOrder(_) => 14,
            Self::Orbit(_) => 15,
            Self::Multiplicity(_) => 16,
            Self::RealValues(_) => 17,
            Self::Isolation(_) => 18,
            Self::Presentation(_) => 19,
            Self::CoefficientEvaluation(_) => 20,
            Self::RealSign(_) => 21,
            Self::RealRootCount(_) => 22,
            Self::RealCellExists(_) => 23,
        }
    }
}

impl<R> SolveCertificate<R> {
    /// Encode the closed subtype and its complete ordered payload.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        self.validate_modes()?;
        out.byte(self.tag())?;
        match self {
            Self::CoefficientIdentity(value) => value.encode(out, write),
            Self::RootAdjunction(value) => value.encode(out, write),
            Self::ExtensionUnit(value) => value.encode(out, write),
            Self::ComponentSplit(value) => value.encode(out, write),
            Self::Membership(value) => value.encode(out, write),
            Self::SolutionTransport(value) => value.encode(out, write),
            Self::TotalPredicateSplit(value) => value.encode(out, write),
            Self::BinomialCoverage(value) => value.encode(out, write),
            Self::RootSelectionRefinement(value) => value.encode(out, write),
            Self::ExactLinearEquivalence(value) => value.encode(out, write),
            Self::ProductCompleteness(value) => value.encode(out, write),
            Self::MultiplicityTransfer(value) => value.encode(out, write),
            Self::ConjugationTransport(value) => value.encode(out, write),
            Self::PermutationOrbitCoverage(value) => value.encode(out, write),
            Self::RealIsolationAndOrder(value) => value.encode(out, write),
            Self::ParameterImageCoverage(value) => value.encode(out, write),
            Self::ImplicitFunctionBranch(value) => value.encode(out, write),
            Self::CoefficientEvaluation(value) => value.encode(out, write),
            Self::RealSign(value) => value.encode(out, write),
            Self::RealRootCount(value) => value.encode(out, write),
            Self::RealCellExists(value) => value.encode(out, write),
        }
    }

    /// Stable subtype discriminator.
    pub(crate) const fn tag(&self) -> u8 {
        match self {
            Self::CoefficientIdentity(_) => 0,
            Self::RootAdjunction(_) => 1,
            Self::ExtensionUnit(_) => 2,
            Self::ComponentSplit(_) => 3,
            Self::Membership(_) => 4,
            Self::SolutionTransport(_) => 5,
            Self::TotalPredicateSplit(_) => 6,
            Self::BinomialCoverage(_) => 7,
            Self::RootSelectionRefinement(_) => 8,
            Self::ExactLinearEquivalence(_) => 9,
            Self::ProductCompleteness(_) => 10,
            Self::MultiplicityTransfer(_) => 11,
            Self::ConjugationTransport(_) => 12,
            Self::PermutationOrbitCoverage(_) => 14,
            Self::RealIsolationAndOrder(_) => 15,
            Self::ParameterImageCoverage(_) => 16,
            Self::ImplicitFunctionBranch(_) => 17,
            Self::CoefficientEvaluation(_) => 18,
            Self::RealSign(_) => 19,
            Self::RealRootCount(_) => 20,
            Self::RealCellExists(_) => 21,
        }
    }
}

use crate::budget::BudgetKind;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::wire_decode::Decoded;
use crate::wire_read::{Reader, invalid};

/// Check a vector's remaining bytes, semantic ceiling and retained size before allocation.
fn decode_vector<T>(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    kind: BudgetKind,
    minimum: u64,
    item_bytes: u64,
    mut read: impl FnMut(&mut Reader<'_, '_>, &[Decoded]) -> Result<T, Fault>,
) -> Result<Vec<T>, Fault> {
    let count = reader.count(kind, minimum, item_bytes)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(crate::error::Location::new("solving wire vector")))?;
    for _ in 0..count {
        result.push(read(reader, records)?);
    }
    Ok(result)
}

impl CoordinateRole {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Coefficient,
            1 => Self::Unknown,
            2 => Self::Family,
            _ => return Err(invalid()),
        })
    }
}

impl SquareFreeRequest {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::NotRequested,
            1 => Self::Required,
            _ => return Err(invalid()),
        })
    }
}

impl MapMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Embedding,
            1 => Self::Specialization,
            _ => return Err(invalid()),
        })
    }
}

impl SelectionMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Any,
            1 => Self::Constrained,
            2 => Self::Principal,
            3 => Self::Isolated,
            _ => return Err(invalid()),
        })
    }
}

impl SolveForm {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::AnyExact,
            1 => Self::AlgebraicRoots,
            2 => Self::Radicals,
            3 => Self::ExactFamily,
            4 => Self::LocalBranch,
            5 => Self::RealExact,
            _ => return Err(invalid()),
        })
    }
}

impl RegularityKind {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::RealC1,
            1 => Self::Holomorphic,
            _ => return Err(invalid()),
        })
    }
}

impl IdentityMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Normalize,
            1 => Self::RelationCombination,
            2 => Self::NonzeroPower(reader.index()?),
            _ => return Err(invalid()),
        })
    }
}

impl AdjunctionMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::AlgebraicClosure,
            1 => Self::ComplexFTA,
            2 => Self::PrimitiveUnity,
            3 => Self::RealOddRoot,
            4 => Self::PositiveRealSquareRoot,
            _ => return Err(invalid()),
        })
    }
}

impl PredicateSplitMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::ZeroNonzero,
            1 => Self::RealSigns,
            2 => Self::EliminateCover,
            _ => return Err(invalid()),
        })
    }
}

impl BinomialMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Order,
            1 => Self::Coverage,
            2 => Self::ZeroRadicand,
            _ => return Err(invalid()),
        })
    }
}

impl RefinementMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Existence,
            1 => Self::Unique,
            2 => Self::Presentation,
            _ => return Err(invalid()),
        })
    }
}

impl RefinementTargetKind {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Selection,
            1 => Self::SolutionSet,
            _ => return Err(invalid()),
        })
    }
}

impl MultiplicityMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Derivatives,
            1 => Self::SquareFreeLayers,
            _ => return Err(invalid()),
        })
    }
}

impl LocalMode {
    /// Reject unknown or irrelevant subtype bytes.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::PolynomialRegularity,
            1 => Self::OpenNeighborhood,
            2 => Self::ImplicitExistence,
            3 => Self::Derivative,
            _ => return Err(invalid()),
        })
    }
}

impl Coordinate {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, _records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            key: reader.name()?,
            field: reader.field()?,
            role: CoordinateRole::decode(reader)?,
        })
    }
}

impl RationalInterval {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, _records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            left: reader.rational()?,
            right: reader.rational()?,
        })
    }
}

impl CoordinateSpaceData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scalar_spaces: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                1,
                64,
                crate::wire_decode::optional,
            )?,
            coordinates: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                6,
                256,
                Coordinate::decode,
            )?,
        })
    }
}

impl TupleData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scope: reader.scope()?,
            coordinates: crate::wire_decode::object(reader, records)?,
            entries: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl SolutionSetData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            branches: decode_vector(
                reader,
                records,
                BudgetKind::SolutionBranches,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl FamilyMapData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scope: reader.scope()?,
            coefficients: crate::wire_decode::object(reader, records)?,
            family: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            target: crate::wire_decode::object(reader, records)?,
            body: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl ExtensionLevelData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            parent: crate::wire_decode::object(reader, records)?,
            polynomial: crate::wire_decode::object(reader, records)?,
            generator: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                1,
                256,
                |reader, _records| reader.atomic(),
            )?,
            square_free: SquareFreeRequest::decode(reader)?,
        })
    }
}

impl CoefficientMapData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            source: crate::wire_decode::object(reader, records)?,
            target: MapTarget::decode(reader, records)?,
            mode: MapMode::decode(reader)?,
            region: crate::wire_decode::object(reader, records)?,
            images: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            denominators: decode_vector(
                reader,
                records,
                BudgetKind::DenominatorFactors,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl RootRelationData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scope: reader.scope()?,
            ambient: reader.field()?,
            polynomial: crate::wire_decode::object(reader, records)?,
            binder: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                1,
                256,
                |reader, _records| reader.atomic(),
            )?,
            map: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            multiplicity: if reader.boolean()? {
                Some(reader.index()?)
            } else {
                None
            },
        })
    }
}

impl RootSelectionData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scope: reader.scope()?,
            relations: decode_vector(
                reader,
                records,
                BudgetKind::SelectionSlots,
                4,
                64,
                crate::wire_decode::object,
            )?,
            predicate: crate::wire_decode::object(reader, records)?,
            mode: SelectionMode::decode(reader)?,
            convention: if reader.boolean()? {
                Some(reader.name()?)
            } else {
                None
            },
            interval: if reader.boolean()? {
                Some(RationalInterval::decode(reader, records)?)
            } else {
                None
            },
            cell: crate::wire_decode::optional(reader, records)?,
        })
    }
}

impl ComponentSplitData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            parent: crate::wire_decode::object(reader, records)?,
            left_factor: crate::wire_decode::object(reader, records)?,
            right_factor: crate::wire_decode::object(reader, records)?,
            left_domain: crate::wire_decode::object(reader, records)?,
            right_domain: crate::wire_decode::object(reader, records)?,
            left_map: crate::wire_decode::object(reader, records)?,
            right_map: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CoefficientValueObject<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            domain: crate::wire_decode::object(reader, records)?,
            value: CoefficientValueData::decode(reader, records)?,
        })
    }
}

impl SolutionBranchData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            domain: crate::wire_decode::object(reader, records)?,
            kind: SolutionBranchKind::decode(reader, records)?,
        })
    }
}

impl SolveProblemData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: crate::wire_decode::object(reader, records)?,
            scope: reader.scope()?,
            predicate: crate::wire_decode::object(reader, records)?,
            coefficients: crate::wire_decode::object(reader, records)?,
            unknowns: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            domain: crate::wire_decode::object(reader, records)?,
            coefficient_domain: crate::wire_decode::object(reader, records)?,
            ambient: reader.field()?,
            denominators: decode_vector(
                reader,
                records,
                BudgetKind::DenominatorFactors,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl LocalBranchData<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            base_coefficients: crate::wire_decode::object(reader, records)?,
            base_unknowns: crate::wire_decode::object(reader, records)?,
            neighborhood: Neighborhood::decode(reader, records)?,
            family_symbol: crate::model::Declaration {
                key: reader.name()?,
                display_name: reader.name()?,
                sort: reader.sort()?,
            },
            jacobian: crate::wire_decode::object(reader, records)?,
            inverse: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl IsolationInterval {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, _records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            left: reader.rational()?,
            right: reader.rational()?,
            left_variations: reader.index()?,
            right_variations: reader.index()?,
            count: reader.index()?,
        })
    }
}

impl RelationMultiple<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            multiplier: crate::wire_decode::object(reader, records)?,
            relation: crate::wire_decode::object(reader, records)?,
            hypothesis: if reader.boolean()? {
                Some(ScopedCoefficientRelation::decode(reader, records)?)
            } else {
                None
            },
        })
    }
}

impl ScopedCoefficientRelation<Handle> {
    /// Bound the root interpretation before allocating or reading any term handles.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            premise: reader.index()?,
            left: crate::wire_decode::object(reader, records)?,
            right: crate::wire_decode::object(reader, records)?,
            roots: decode_vector(
                reader,
                records,
                BudgetKind::PolynomialVariables,
                4,
                64,
                crate::wire_decode::object,
            )?,
            vieta: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
        })
    }
}

impl SolveSubstitution<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            key: reader.name()?,
            expression: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl PrimePower {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, _records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            prime: reader.index()?,
            exponent: reader.index()?,
        })
    }
}

impl RootFactor<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            root: crate::wire_decode::object(reader, records)?,
            multiplicity: reader.index()?,
        })
    }
}

impl GeneratorPair<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            original: crate::wire_decode::object(reader, records)?,
            conjugate: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementMember<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            tuple: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementEquivalent<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            source: crate::wire_decode::object(reader, records)?,
            target: crate::wire_decode::object(reader, records)?,
            forward: crate::wire_decode::object(reader, records)?,
            backward: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementCoverage<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            set: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementFamilyMembership<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            family: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementFamilyCoverage<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            family: crate::wire_decode::object(reader, records)?,
            recovery: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementRank<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            matrix: crate::wire_decode::object(reader, records)?,
            domain: crate::wire_decode::object(reader, records)?,
            rank: reader.index()?,
            pivots: decode_vector(
                reader,
                records,
                BudgetKind::MatrixEntries,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            minor: crate::wire_decode::object(reader, records)?,
            elimination: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementLinearSystem<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            reduced: crate::wire_decode::object(reader, records)?,
            rank: reader.index()?,
            augmented_rank: reader.index()?,
            region: crate::wire_decode::object(reader, records)?,
            free_columns: decode_vector(
                reader,
                records,
                BudgetKind::MatrixEntries,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
        })
    }
}

impl StatementLocalBranch<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            branch: crate::wire_decode::object(reader, records)?,
            regularity: RegularityKind::decode(reader)?,
            neighborhood: Neighborhood::decode(reader, records)?,
        })
    }
}

impl StatementLocalDerivative<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            branch: crate::wire_decode::object(reader, records)?,
            derivative: crate::wire_decode::object(reader, records)?,
            neighborhood: Neighborhood::decode(reader, records)?,
        })
    }
}

impl StatementCoefficientEquality<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            left: crate::wire_decode::object(reader, records)?,
            right: crate::wire_decode::object(reader, records)?,
            map: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementUnit<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            value: crate::wire_decode::object(reader, records)?,
            inverse: crate::wire_decode::object(reader, records)?,
            domain: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementComponentCover<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            split: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementRootExistence<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementSelectionUnique<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            selection: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementRootOrder<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            value: crate::wire_decode::object(reader, records)?,
            order: reader.index()?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementOrbit<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            seed: crate::wire_decode::object(reader, records)?,
            generators: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                256,
                |reader, records| {
                    decode_vector(
                        reader,
                        records,
                        BudgetKind::ProofNodes,
                        4,
                        4,
                        |reader, _records| reader.index(),
                    )
                },
            )?,
            members: decode_vector(
                reader,
                records,
                BudgetKind::OrbitMembers,
                4,
                64,
                crate::wire_decode::object,
            )?,
            actions: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                256,
                |reader, records| {
                    decode_vector(
                        reader,
                        records,
                        BudgetKind::ProofNodes,
                        4,
                        4,
                        |reader, _records| reader.index(),
                    )
                },
            )?,
        })
    }
}

impl StatementMultiplicity<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            selection: crate::wire_decode::object(reader, records)?,
            multiplicities: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementRealValues<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            values: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementIsolation<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: crate::wire_decode::object(reader, records)?,
            intervals: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                30,
                256,
                IsolationInterval::decode,
            )?,
            exterior_count: reader.index()?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl StatementPresentation<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            set: crate::wire_decode::object(reader, records)?,
            form: SolveForm::decode(reader)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificateCoefficientIdentity<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: IdentityMode::decode(reader)?,
            left: crate::wire_decode::object(reader, records)?,
            right: crate::wire_decode::object(reader, records)?,
            map: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            combinations: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                9,
                256,
                RelationMultiple::decode,
            )?,
            denominators: decode_vector(
                reader,
                records,
                BudgetKind::DenominatorFactors,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl CertificateRootAdjunction<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: AdjunctionMode::decode(reader)?,
            relation: crate::wire_decode::object(reader, records)?,
            selection: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
            leading_unit: reader.index()?,
            order: if reader.boolean()? {
                Some(reader.index()?)
            } else {
                None
            },
            splitting_chain: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl CertificateExtensionUnit<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            element: crate::wire_decode::object(reader, records)?,
            modulus: crate::wire_decode::object(reader, records)?,
            gcd: crate::wire_decode::object(reader, records)?,
            quotients: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            bezout: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificateComponentSplit<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            split: crate::wire_decode::object(reader, records)?,
            product: reader.index()?,
            bezout: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            square_free: reader.index()?,
            selections: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
        })
    }
}

impl CertificateMembership<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            tuple: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            substitutions: decode_vector(
                reader,
                records,
                BudgetKind::ConditionAtoms,
                8,
                256,
                SolveSubstitution::decode,
            )?,
            definedness: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
        })
    }
}

impl CertificateSolutionTransport<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            source: crate::wire_decode::object(reader, records)?,
            target: crate::wire_decode::object(reader, records)?,
            forward: crate::wire_decode::object(reader, records)?,
            backward: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            definedness: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            equations: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            forward_inverse: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            backward_inverse: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
        })
    }
}

impl CertificateTotalPredicateSplit<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: PredicateSplitMode::decode(reader)?,
            expression: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
            children: decode_vector(
                reader,
                records,
                BudgetKind::CaseArms,
                4,
                64,
                crate::wire_decode::object,
            )?,
            branches: decode_vector(
                reader,
                records,
                BudgetKind::SolutionBranches,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            cover: if reader.boolean()? {
                Some(reader.index()?)
            } else {
                None
            },
        })
    }
}

impl CertificateBinomialCoverage<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: BinomialMode::decode(reader)?,
            order: reader.index()?,
            unity: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
            selection: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
            factors: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                8,
                256,
                PrimePower::decode,
            )?,
            exclusions: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            product: if reader.boolean()? {
                Some(reader.index()?)
            } else {
                None
            },
        })
    }
}

impl CertificateRootSelectionRefinement<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: RefinementMode::decode(reader)?,
            target_kind: RefinementTargetKind::decode(reader)?,
            target: crate::wire_decode::object(reader, records)?,
            witnesses: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            compatibility: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            form: if reader.boolean()? {
                Some(SolveForm::decode(reader)?)
            } else {
                None
            },
        })
    }
}

impl CertificateExactLinearEquivalence<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            matrix: crate::wire_decode::object(reader, records)?,
            augmented_columns: reader.index()?,
            operations: decode_vector(
                reader,
                records,
                BudgetKind::MatrixEntries,
                9,
                256,
                RowOperation::decode,
            )?,
            pivots: decode_vector(
                reader,
                records,
                BudgetKind::MatrixEntries,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            minor: crate::wire_decode::object(reader, records)?,
            reduced: crate::wire_decode::object(reader, records)?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificateProductCompleteness<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            set: crate::wire_decode::object(reader, records)?,
            scalar: crate::wire_decode::object(reader, records)?,
            factors: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                8,
                256,
                RootFactor::decode,
            )?,
            product: reader.index()?,
            existence: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificateMultiplicityTransfer<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: MultiplicityMode::decode(reader)?,
            problem: crate::wire_decode::object(reader, records)?,
            selection: crate::wire_decode::object(reader, records)?,
            multiplicities: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            values: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            product: reader.index()?,
            nonzero: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificateConjugationTransport<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            map: crate::wire_decode::object(reader, records)?,
            conjugate_map: crate::wire_decode::object(reader, records)?,
            images: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                8,
                256,
                GeneratorPair::decode,
            )?,
            values: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            relations: decode_vector(
                reader,
                records,
                BudgetKind::SelectionSlots,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            fixed_field: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            region: crate::wire_decode::object(reader, records)?,
        })
    }
}

impl CertificatePermutationOrbitCoverage<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            seed: crate::wire_decode::object(reader, records)?,
            degree: reader.index()?,
            generators: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                256,
                |reader, records| {
                    decode_vector(
                        reader,
                        records,
                        BudgetKind::ProofNodes,
                        4,
                        4,
                        |reader, _records| reader.index(),
                    )
                },
            )?,
            members: decode_vector(
                reader,
                records,
                BudgetKind::OrbitMembers,
                4,
                64,
                crate::wire_decode::object,
            )?,
            actions: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                256,
                |reader, records| {
                    decode_vector(
                        reader,
                        records,
                        BudgetKind::ProofNodes,
                        4,
                        4,
                        |reader, _records| reader.index(),
                    )
                },
            )?,
            words: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                256,
                |reader, records| {
                    decode_vector(
                        reader,
                        records,
                        BudgetKind::ProofNodes,
                        4,
                        4,
                        |reader, _records| reader.index(),
                    )
                },
            )?,
        })
    }
}

impl CertificateRealIsolationAndOrder<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: crate::wire_decode::object(reader, records)?,
            sturm: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                4,
                64,
                crate::wire_decode::object,
            )?,
            intervals: decode_vector(
                reader,
                records,
                BudgetKind::Edges,
                30,
                256,
                IsolationInterval::decode,
            )?,
            exterior_left: reader.index()?,
            exterior_right: reader.index()?,
            multiplicity: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            ordered_index: if reader.boolean()? {
                Some(reader.index()?)
            } else {
                None
            },
        })
    }
}

impl CertificateParameterImageCoverage<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            problem: crate::wire_decode::object(reader, records)?,
            family: crate::wire_decode::object(reader, records)?,
            recovery: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
            region: crate::wire_decode::object(reader, records)?,
            membership: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            recovery_proofs: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            domain: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            scope: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
        })
    }
}

impl CertificateImplicitFunctionBranch<Handle> {
    /// Decode exactly the specified ordered payload with bounded child vectors.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            mode: LocalMode::decode(reader)?,
            branch: crate::wire_decode::object(reader, records)?,
            regularity: RegularityKind::decode(reader)?,
            regularity_proofs: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            base_equations: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            inverse: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            derivatives: decode_vector(
                reader,
                records,
                BudgetKind::ProofNodes,
                4,
                4,
                |reader, _records| reader.index(),
            )?,
            derivative: if reader.boolean()? {
                Some(crate::wire_decode::object(reader, records)?)
            } else {
                None
            },
        })
    }
}

impl CoefficientDomainData<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Rational,
            1 => Self::Gaussian,
            2 => Self::Fraction {
                base: crate::wire_decode::object(reader, records)?,
                coordinates: crate::wire_decode::object(reader, records)?,
            },
            3 => Self::Quotient {
                level: crate::wire_decode::object(reader, records)?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl MapTarget<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            2 => Self::Family {
                family: crate::wire_decode::object(reader, records)?,
            },
            0 => Self::CoefficientDomain {
                domain: crate::wire_decode::object(reader, records)?,
            },
            1 => Self::ScalarAmbient {
                header: crate::wire_decode::object(reader, records)?,
                field: reader.field()?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl CoefficientValueData<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Rational {
                value: reader.rational()?,
            },
            1 => Self::Gaussian {
                real: reader.rational()?,
                imaginary: reader.rational()?,
            },
            2 => Self::Fraction {
                numerator: crate::wire_decode::object(reader, records)?,
                denominator: crate::wire_decode::object(reader, records)?,
            },
            3 => Self::Tower {
                coefficients: decode_vector(
                    reader,
                    records,
                    BudgetKind::Edges,
                    4,
                    64,
                    crate::wire_decode::object,
                )?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl SolutionBranchKind<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Finite {
                tuples: decode_vector(
                    reader,
                    records,
                    BudgetKind::CaseArms,
                    4,
                    64,
                    crate::wire_decode::object,
                )?,
            },
            1 => Self::Image {
                family: crate::wire_decode::object(reader, records)?,
            },
            2 => Self::Algebraic {
                selection: crate::wire_decode::object(reader, records)?,
            },
            3 => Self::Empty,
            4 => Self::Local {
                branch: crate::wire_decode::object(reader, records)?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl Neighborhood<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Existential {
                coefficient_binder: decode_vector(
                    reader,
                    records,
                    BudgetKind::Edges,
                    1,
                    256,
                    |reader, _records| reader.atomic(),
                )?,
                unknown_binder: decode_vector(
                    reader,
                    records,
                    BudgetKind::Edges,
                    1,
                    256,
                    |reader, _records| reader.atomic(),
                )?,
            },
            1 => Self::Explicit {
                coefficient_region: crate::wire_decode::object(reader, records)?,
                unknown_region: crate::wire_decode::object(reader, records)?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl RowOperation<Handle> {
    /// Decode the closed payload subtype without inferring any proof.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Swap {
                left: reader.index()?,
                right: reader.index()?,
            },
            1 => Self::Scale {
                row: reader.index()?,
                value: crate::wire_decode::object(reader, records)?,
                unit: reader.index()?,
            },
            2 => Self::AddMultiple {
                destination: reader.index()?,
                source: reader.index()?,
                value: crate::wire_decode::object(reader, records)?,
            },
            _ => return Err(invalid()),
        })
    }
}

impl SolveDraft<Handle> {
    /// Read one closed subtype; availability is still checked independently.
    pub(crate) fn decode(
        tag: u8,
        reader: &mut Reader<'_, '_>,
        records: &[Decoded],
    ) -> Result<Self, Fault> {
        Ok(match tag {
            13 => Self::CoordinateSpace(CoordinateSpaceData::decode(reader, records)?),
            14 => Self::Tuple(TupleData::decode(reader, records)?),
            15 => Self::SolutionSet(SolutionSetData::decode(reader, records)?),
            16 => Self::FamilyMap(FamilyMapData::decode(reader, records)?),
            17 => Self::CoefficientDomain(CoefficientDomainData::decode(reader, records)?),
            18 => Self::Polynomial(PolynomialData::decode(reader, records)?),
            19 => Self::ExtensionLevel(ExtensionLevelData::decode(reader, records)?),
            20 => Self::CoefficientMap(CoefficientMapData::decode(reader, records)?),
            21 => Self::RootRelation(RootRelationData::decode(reader, records)?),
            22 => Self::RootSelection(RootSelectionData::decode(reader, records)?),
            23 => Self::ComponentSplit(ComponentSplitData::decode(reader, records)?),
            24 => Self::CoefficientValue(CoefficientValueObject::decode(reader, records)?),
            25 => Self::SolutionBranch(SolutionBranchData::decode(reader, records)?),
            26 => Self::SolveProblem(SolveProblemData::decode(reader, records)?),
            27 => Self::LocalBranch(LocalBranchData::decode(reader, records)?),
            28 => Self::ParameterBase(ParameterBaseData::decode(reader, records)?),
            29 => Self::SpaceFamily(SpaceFamilyData::decode(reader, records)?),
            30 => Self::FamilyExpression(FamilyExpressionData::decode(reader, records)?),
            31 => Self::FibrePiece(FibrePieceData::decode(reader, records)?),
            32 => Self::RealCell(RealCellData::decode(reader, records)?),
            33 => Self::CoefficientEvaluation(CoefficientEvaluationData::decode(reader, records)?),
            34 => Self::ScalarSpace(ScalarSpaceData::decode(reader, records)?),
            35 => Self::SpacePlacement(SpacePlacementData::decode(reader, records)?),
            36 => Self::ScalarPiece(ScalarPieceData::decode(reader, records)?),
            _ => return Err(invalid()),
        })
    }
}

impl SolveStatement<Handle> {
    /// Read one closed subtype; availability is still checked independently.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(match reader.byte()? {
            0 => Self::Member(StatementMember::decode(reader, records)?),
            1 => Self::Equivalent(StatementEquivalent::decode(reader, records)?),
            2 => Self::Coverage(StatementCoverage::decode(reader, records)?),
            3 => Self::FamilyMembership(StatementFamilyMembership::decode(reader, records)?),
            4 => Self::FamilyCoverage(StatementFamilyCoverage::decode(reader, records)?),
            5 => Self::Rank(StatementRank::decode(reader, records)?),
            6 => Self::LinearSystem(StatementLinearSystem::decode(reader, records)?),
            7 => Self::LocalBranch(StatementLocalBranch::decode(reader, records)?),
            8 => Self::LocalDerivative(StatementLocalDerivative::decode(reader, records)?),
            9 => Self::CoefficientEquality(StatementCoefficientEquality::decode(reader, records)?),
            10 => Self::Unit(StatementUnit::decode(reader, records)?),
            11 => Self::ComponentCover(StatementComponentCover::decode(reader, records)?),
            12 => Self::RootExistence(StatementRootExistence::decode(reader, records)?),
            13 => Self::SelectionUnique(StatementSelectionUnique::decode(reader, records)?),
            14 => Self::RootOrder(StatementRootOrder::decode(reader, records)?),
            15 => Self::Orbit(StatementOrbit::decode(reader, records)?),
            16 => Self::Multiplicity(StatementMultiplicity::decode(reader, records)?),
            17 => Self::RealValues(StatementRealValues::decode(reader, records)?),
            18 => Self::Isolation(StatementIsolation::decode(reader, records)?),
            19 => Self::Presentation(StatementPresentation::decode(reader, records)?),
            20 => Self::CoefficientEvaluation(StatementCoefficientEvaluation::decode(
                reader, records,
            )?),
            21 => Self::RealSign(StatementRealSign::decode(reader, records)?),
            22 => Self::RealRootCount(StatementRealRootCount::decode(reader, records)?),
            23 => Self::RealCellExists(StatementRealCellExists::decode(reader, records)?),
            _ => return Err(invalid()),
        })
    }
}

impl SolveCertificate<Handle> {
    /// Read one closed subtype; availability is still checked independently.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        let value = match reader.byte()? {
            0 => {
                Self::CoefficientIdentity(CertificateCoefficientIdentity::decode(reader, records)?)
            }
            1 => Self::RootAdjunction(CertificateRootAdjunction::decode(reader, records)?),
            2 => Self::ExtensionUnit(CertificateExtensionUnit::decode(reader, records)?),
            3 => Self::ComponentSplit(CertificateComponentSplit::decode(reader, records)?),
            4 => Self::Membership(CertificateMembership::decode(reader, records)?),
            5 => Self::SolutionTransport(CertificateSolutionTransport::decode(reader, records)?),
            6 => {
                Self::TotalPredicateSplit(CertificateTotalPredicateSplit::decode(reader, records)?)
            }
            7 => Self::BinomialCoverage(CertificateBinomialCoverage::decode(reader, records)?),
            8 => Self::RootSelectionRefinement(CertificateRootSelectionRefinement::decode(
                reader, records,
            )?),
            9 => Self::ExactLinearEquivalence(CertificateExactLinearEquivalence::decode(
                reader, records,
            )?),
            10 => {
                Self::ProductCompleteness(CertificateProductCompleteness::decode(reader, records)?)
            }
            11 => Self::MultiplicityTransfer(CertificateMultiplicityTransfer::decode(
                reader, records,
            )?),
            12 => Self::ConjugationTransport(CertificateConjugationTransport::decode(
                reader, records,
            )?),
            14 => Self::PermutationOrbitCoverage(CertificatePermutationOrbitCoverage::decode(
                reader, records,
            )?),
            15 => Self::RealIsolationAndOrder(CertificateRealIsolationAndOrder::decode(
                reader, records,
            )?),
            16 => Self::ParameterImageCoverage(CertificateParameterImageCoverage::decode(
                reader, records,
            )?),
            17 => Self::ImplicitFunctionBranch(CertificateImplicitFunctionBranch::decode(
                reader, records,
            )?),
            18 => Self::CoefficientEvaluation(CertificateCoefficientEvaluation::decode(
                reader, records,
            )?),
            19 => Self::RealSign(CertificateRealSign::decode(reader, records)?),
            20 => Self::RealRootCount(CertificateRealRootCount::decode(reader, records)?),
            21 => Self::RealCellExists(CertificateRealCellExists::decode(reader, records)?),
            _ => return Err(invalid()),
        };
        value.validate_modes()?;
        Ok(value)
    }
}

impl PolynomialData<Handle> {
    /// Exponent vectors have exactly the previously declared variable-frame width.
    fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        let domain = crate::wire_decode::object(reader, records)?;
        let variables = decode_vector(
            reader,
            records,
            BudgetKind::PolynomialVariables,
            1,
            128,
            |reader, _records| reader.field(),
        )?;
        let width = crate::graph::size(
            variables.len(),
            crate::error::Location::new("polynomial wire"),
        )?;
        let minimum = width
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or_else(invalid)?;
        let terms = decode_vector(
            reader,
            records,
            BudgetKind::PolynomialTerms,
            minimum,
            minimum.checked_mul(16).ok_or_else(invalid)?,
            |reader, records| {
                let mut exponents = Vec::new();
                exponents.try_reserve_exact(variables.len()).map_err(|_| {
                    Fault::allocation(crate::error::Location::new("polynomial exponents"))
                })?;
                for _ in 0..variables.len() {
                    exponents.push(reader.index()?);
                }
                Ok(PolynomialTerm {
                    exponents,
                    coefficient: crate::wire_decode::object(reader, records)?,
                })
            },
        )?;
        Ok(Self {
            domain,
            variables,
            terms,
        })
    }
}

/// Decode and admit one solving record in the unpublished replay transaction.
pub(crate) fn decode_object(
    cas: &mut Cas,
    tag: u8,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Handle, Fault> {
    let value = SolveDraft::decode(tag, reader, records)?;
    if let SolveDraft::CoordinateSpace(frame) = &value
        && frame.scalar_spaces.len() != frame.coordinates.len()
    {
        return Err(invalid());
    }
    let value = value.resolve(&mut |handle| Ok(Reference::Existing(handle.clone())))?;
    cas.admit(&[Draft::Solve(value)], reader.budget)?
        .into_iter()
        .next()
        .ok_or_else(invalid)
}

impl<R> SolveDraft<R> {
    /// Stable wire record tags, independent of graph-kind tags.
    pub(crate) const fn record_tag(&self) -> u8 {
        match self {
            Self::CoordinateSpace(_) => 13,
            Self::Tuple(_) => 14,
            Self::SolutionSet(_) => 15,
            Self::FamilyMap(_) => 16,
            Self::CoefficientDomain(_) => 17,
            Self::Polynomial(_) => 18,
            Self::ExtensionLevel(_) => 19,
            Self::CoefficientMap(_) => 20,
            Self::RootRelation(_) => 21,
            Self::RootSelection(_) => 22,
            Self::ComponentSplit(_) => 23,
            Self::CoefficientValue(_) => 24,
            Self::SolutionBranch(_) => 25,
            Self::SolveProblem(_) => 26,
            Self::LocalBranch(_) => 27,
            Self::ParameterBase(_) => 28,
            Self::SpaceFamily(_) => 29,
            Self::FamilyExpression(_) => 30,
            Self::FibrePiece(_) => 31,
            Self::RealCell(_) => 32,
            Self::CoefficientEvaluation(_) => 33,
            Self::ScalarSpace(_) => 34,
            Self::SpacePlacement(_) => 35,
            Self::ScalarPiece(_) => 36,
        }
    }
}

impl<R> SolveCertificate<R> {
    /// Mode-specific option fields are syntax, never substitute truth flags.
    fn validate_modes(&self) -> Result<(), Fault> {
        let valid = match self {
            Self::CoefficientIdentity(value) => match value.mode {
                IdentityMode::Normalize => value.combinations.is_empty(),
                IdentityMode::RelationCombination => true,
                IdentityMode::NonzeroPower(exponent) => exponent > 0,
            },
            Self::RootAdjunction(value) => {
                matches!(
                    value.mode,
                    AdjunctionMode::PrimitiveUnity
                        | AdjunctionMode::RealOddRoot
                        | AdjunctionMode::PositiveRealSquareRoot
                ) == value.order.is_some()
                    && value.order != Some(0)
            }
            Self::TotalPredicateSplit(value) => {
                (value.mode == PredicateSplitMode::EliminateCover) == value.cover.is_some()
                    && match value.mode {
                        PredicateSplitMode::ZeroNonzero => value.children.len() == 2,
                        PredicateSplitMode::RealSigns => value.children.len() == 3,
                        PredicateSplitMode::EliminateCover => {
                            value.branches.len() == value.children.len()
                        }
                    }
            }
            Self::BinomialCoverage(value) => value.valid_mode(),
            Self::RootSelectionRefinement(value) => {
                (value.mode == RefinementMode::Presentation) == value.form.is_some()
                    && (value.mode == RefinementMode::Presentation)
                        == (value.target_kind == RefinementTargetKind::SolutionSet)
            }
            Self::ExactLinearEquivalence(value) => {
                value.operations.iter().all(|operation| match operation {
                    RowOperation::Swap { left, right } => left != right,
                    RowOperation::AddMultiple {
                        destination,
                        source,
                        ..
                    } => destination != source,
                    RowOperation::Scale { .. } => true,
                })
            }
            Self::ProductCompleteness(value) => {
                value.factors.iter().all(|factor| factor.multiplicity > 0)
            }
            Self::MultiplicityTransfer(value) => !value.multiplicities.contains(&0),
            Self::ImplicitFunctionBranch(value) => value.valid_mode(),
            _ => true,
        };
        if valid { Ok(()) } else { Err(invalid()) }
    }
}

impl<R> CertificateBinomialCoverage<R> {
    /// Optional unity, selection and product fields exactly follow the closed binomial mode.
    fn valid_mode(&self) -> bool {
        if self.order == 0
            || self
                .factors
                .iter()
                .any(|factor| factor.prime < 2 || factor.exponent == 0)
        {
            return false;
        }
        match self.mode {
            BinomialMode::Order => {
                self.unity.is_some() && self.selection.is_none() && self.product.is_none()
            }
            BinomialMode::Coverage => {
                self.unity.is_some() && self.selection.is_some() && self.product.is_some()
            }
            BinomialMode::ZeroRadicand => {
                self.unity.is_none()
                    && self.selection.is_some()
                    && self.product.is_some()
                    && self.factors.is_empty()
                    && self.exclusions.is_empty()
            }
        }
    }
}

impl<R> CertificateImplicitFunctionBranch<R> {
    /// Finite regularity subcases cannot smuggle in theorem-premise payloads.
    const fn valid_mode(&self) -> bool {
        match self.mode {
            LocalMode::PolynomialRegularity | LocalMode::OpenNeighborhood => {
                self.regularity_proofs.is_empty()
                    && self.base_equations.is_empty()
                    && self.inverse.is_empty()
                    && self.derivatives.is_empty()
                    && self.derivative.is_none()
            }
            LocalMode::ImplicitExistence => {
                self.derivative.is_none() && self.derivatives.is_empty()
            }
            LocalMode::Derivative => self.derivative.is_some(),
        }
    }
}
