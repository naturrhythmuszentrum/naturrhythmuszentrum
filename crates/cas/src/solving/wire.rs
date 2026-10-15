// SPDX-License-Identifier: AGPL-3.0-only
//! Shared bounded payload encodings; all reference identities are supplied by the caller.

use super::model::{
    AdjunctionMode, BinomialMode, CertificateBinomialCoverage, CertificateCoefficientIdentity,
    CertificateComponentSplit, CertificateConjugationTransport, CertificateExactLinearEquivalence,
    CertificateExtensionUnit, CertificateImplicitFunctionBranch, CertificateMembership,
    CertificateMultiplicityTransfer, CertificateParameterImageCoverage,
    CertificatePermutationOrbitCoverage, CertificateProductCompleteness,
    CertificateRealIsolationAndOrder, CertificateRealRestriction, CertificateRootAdjunction,
    CertificateRootSelectionRefinement, CertificateSolutionTransport,
    CertificateTotalPredicateSplit, CoefficientDomainData, CoefficientMapData,
    CoefficientValueData, CoefficientValueObject, ComponentSplitData, Coordinate, CoordinateRole,
    CoordinateSpaceData, ExclusionPolicy, ExtensionLevelData, FamilyMapData, GeneratorPair,
    IdentityMode, IsolationInterval, LocalBranchData, LocalMode, MapMode, MapTarget,
    MultiplicityMode, Neighborhood, PolynomialData, PolynomialTerm, PredicateSplitMode, PrimePower,
    RationalInterval, RefinementMode, RefinementTargetKind, RegularityKind, RelationMultiple,
    RootFactor, RootRelationData, RootSelectionData, RowOperation, SelectionMode,
    SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveCertificate, SolveDraft,
    SolveForm, SolveGoal, SolveProblemData, SolveStatement, SolveSubstitution, SquareFreeRequest,
    StatementCoefficientEquality, StatementComponentCover, StatementCoverage, StatementEquivalent,
    StatementFamilyCoverage, StatementFamilyMembership, StatementIsolation, StatementLinearSystem,
    StatementLocalBranch, StatementLocalDerivative, StatementMember, StatementMultiplicity,
    StatementOrbit, StatementPresentation, StatementRank, StatementRealValues,
    StatementRootExistence, StatementRootOrder, StatementSelectionUnique, StatementUnit, TupleData,
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

impl<R> PolynomialTerm<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.count(self.exponents.len())?;
        for item0 in &self.exponents {
            crate::wire::index(*item0, out)?;
        }
        write(&self.coefficient, out)?;
        Ok(())
    }
}

impl<R> PolynomialData<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.domain, out)?;
        out.count(self.variables.len())?;
        for item0 in &self.variables {
            encoding::field(item0, out)?;
        }
        out.count(self.terms.len())?;
        for item0 in &self.terms {
            item0.encode(out, write)?;
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

impl<R> CertificateRealRestriction<R> {
    /// Encode every payload field in its specified semantic order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        crate::wire::index(self.coverage, out)?;
        write(&self.problem, out)?;
        write(&self.set, out)?;
        out.count(self.retained.len())?;
        for item0 in &self.retained {
            write(item0, out)?;
        }
        out.count(self.rejected.len())?;
        for item0 in &self.rejected {
            write(item0, out)?;
        }
        out.count(self.classification.len())?;
        for item0 in &self.classification {
            crate::wire::index(*item0, out)?;
        }
        crate::wire::index(self.cover, out)?;
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
            Self::RealRestriction(value) => value.encode(out, write),
            Self::PermutationOrbitCoverage(value) => value.encode(out, write),
            Self::RealIsolationAndOrder(value) => value.encode(out, write),
            Self::ParameterImageCoverage(value) => value.encode(out, write),
            Self::ImplicitFunctionBranch(value) => value.encode(out, write),
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
            Self::RealRestriction(_) => 13,
            Self::PermutationOrbitCoverage(_) => 14,
            Self::RealIsolationAndOrder(_) => 15,
            Self::ParameterImageCoverage(_) => 16,
            Self::ImplicitFunctionBranch(_) => 17,
        }
    }
}
