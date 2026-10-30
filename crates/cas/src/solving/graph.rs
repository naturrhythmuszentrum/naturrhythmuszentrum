// SPDX-License-Identifier: AGPL-3.0-only
//! Capture-safe resolution, graph admission and borrowed solving views.
//!
//! # Exact object admission
//!
//! The `solving` facade adds exact coordinate products, tuples, coefficient algebras and values, sparse polynomials, extension levels, coefficient maps, root relations and joint selections, solution branches and sets, original problems and local germs. `Draft::Solve` admits syntax atomically; `Cas::solve_view` borrows graph-owned payloads and checked handles. Constructors never prove membership, existence, field structure or completeness. Product regions retain coordinate order; the existing interval checkers refuse them. Rational-function admission independently checks coprime multivariate numerator and denominator polynomials over Q or Q(i), with a monic denominator. Recursive content and primitive-part gcd calculations restore the original formal variable order before returning; coprimality does not assert a multivariate Bézout identity to one. When an operand is a monomial, exact per-variable exponent minima bound gcd normalization; both input factorizations are still independently reconstructed. Noncanonical pairs refuse atomically.

use super::model::{
    CertificateBinomialCoverage, CertificateCoefficientIdentity, CertificateComponentSplit,
    CertificateConjugationTransport, CertificateExactLinearEquivalence, CertificateExtensionUnit,
    CertificateImplicitFunctionBranch, CertificateMembership, CertificateMultiplicityTransfer,
    CertificateParameterImageCoverage, CertificatePermutationOrbitCoverage,
    CertificateProductCompleteness, CertificateRealIsolationAndOrder, CertificateRootAdjunction,
    CertificateRootSelectionRefinement, CertificateSolutionTransport,
    CertificateTotalPredicateSplit, CoefficientDomainData, CoefficientMapData,
    CoefficientValueData, CoefficientValueObject, ComponentSplitData, CoordinateRole,
    CoordinateSpaceData, ExtensionLevelData, FamilyMapData, GeneratorPair, LocalBranchData,
    MapMode, MapTarget, Neighborhood, PolynomialData, PolynomialTerm, RelationMultiple, RootFactor,
    RootRelationData, RootSelectionData, RowOperation, ScopedCoefficientRelation, SelectionMode,
    SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveCertificate, SolveDraft,
    SolveProblemData, SolveSpec, SolveStatement, SolveSubstitution, SolveView,
    StatementCoefficientEquality, StatementComponentCover, StatementCoverage, StatementEquivalent,
    StatementFamilyCoverage, StatementFamilyMembership, StatementIsolation, StatementLinearSystem,
    StatementLocalBranch, StatementLocalDerivative, StatementMember, StatementMultiplicity,
    StatementOrbit, StatementPresentation, StatementRank, StatementRealValues,
    StatementRootExistence, StatementRootOrder, StatementSelectionUnique, StatementUnit, TupleData,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, LogicalId, State, Vertex, VertexKind, size};
use crate::model::{AtomicSort, Field, Scope, Sort};

/// Location for bounded solving object admission.
const AT: Location = Location::new("solving object admission");

/// Refuse malformed mathematical syntax without producing evidence.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Sort, AT)
}

/// Resolve a bounded ordered sequence without hiding an allocation failure.
fn map_vector<R, S>(
    values: &[R],
    write: &mut impl FnMut(&R) -> Result<S, Fault>,
) -> Result<Vec<S>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| Fault::allocation(AT))?;
    for value in values {
        result.push(write(value)?);
    }
    Ok(result)
}

impl<R> CoordinateSpaceData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoordinateSpaceData<S>, Fault> {
        Ok(CoordinateSpaceData {
            scalar_spaces: self
                .scalar_spaces
                .iter()
                .map(|value| value.as_ref().map(&mut *write).transpose())
                .collect::<Result<_, _>>()?,
            header: write(&self.header)?,
            coordinates: self.coordinates.clone(),
        })
    }
}

impl<R> TupleData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<TupleData<S>, Fault> {
        Ok(TupleData {
            header: write(&self.header)?,
            scope: self.scope.clone(),
            coordinates: write(&self.coordinates)?,
            entries: map_vector(&self.entries, write)?,
        })
    }
}

impl<R> SolutionSetData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SolutionSetData<S>, Fault> {
        Ok(SolutionSetData {
            problem: write(&self.problem)?,
            branches: map_vector(&self.branches, write)?,
        })
    }
}

impl<R> FamilyMapData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<FamilyMapData<S>, Fault> {
        Ok(FamilyMapData {
            header: write(&self.header)?,
            scope: self.scope.clone(),
            coefficients: write(&self.coefficients)?,
            family: write(&self.family)?,
            region: write(&self.region)?,
            target: write(&self.target)?,
            body: write(&self.body)?,
        })
    }
}

impl<R> PolynomialTerm<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<PolynomialTerm<S>, Fault> {
        Ok(PolynomialTerm {
            exponents: self.exponents.clone(),
            coefficient: write(&self.coefficient)?,
        })
    }
}

impl<R> PolynomialData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<PolynomialData<S>, Fault> {
        Ok(PolynomialData {
            domain: write(&self.domain)?,
            variables: self.variables.clone(),
            terms: map_vector(&self.terms, &mut |value| value.resolve(write))?,
        })
    }
}

impl<R> ExtensionLevelData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<ExtensionLevelData<S>, Fault> {
        Ok(ExtensionLevelData {
            parent: write(&self.parent)?,
            polynomial: write(&self.polynomial)?,
            generator: self.generator.clone(),
            square_free: self.square_free,
        })
    }
}

impl<R> CoefficientMapData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoefficientMapData<S>, Fault> {
        Ok(CoefficientMapData {
            source: write(&self.source)?,
            target: self.target.resolve(write)?,
            mode: self.mode,
            region: write(&self.region)?,
            images: map_vector(&self.images, write)?,
            denominators: map_vector(&self.denominators, write)?,
        })
    }
}

impl<R> RootRelationData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<RootRelationData<S>, Fault> {
        Ok(RootRelationData {
            header: write(&self.header)?,
            scope: self.scope.clone(),
            ambient: self.ambient.clone(),
            polynomial: write(&self.polynomial)?,
            binder: self.binder.clone(),
            map: write(&self.map)?,
            region: write(&self.region)?,
            multiplicity: self.multiplicity,
        })
    }
}

impl<R> RootSelectionData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<RootSelectionData<S>, Fault> {
        Ok(RootSelectionData {
            cell: self.cell.as_ref().map(&mut *write).transpose()?,
            header: write(&self.header)?,
            scope: self.scope.clone(),
            relations: map_vector(&self.relations, write)?,
            predicate: write(&self.predicate)?,
            mode: self.mode,
            convention: self.convention.clone(),
            interval: self.interval.clone(),
        })
    }
}

impl<R> ComponentSplitData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<ComponentSplitData<S>, Fault> {
        Ok(ComponentSplitData {
            parent: write(&self.parent)?,
            left_factor: write(&self.left_factor)?,
            right_factor: write(&self.right_factor)?,
            left_domain: write(&self.left_domain)?,
            right_domain: write(&self.right_domain)?,
            left_map: write(&self.left_map)?,
            right_map: write(&self.right_map)?,
            region: write(&self.region)?,
        })
    }
}

impl<R> CoefficientValueObject<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoefficientValueObject<S>, Fault> {
        Ok(CoefficientValueObject {
            domain: write(&self.domain)?,
            value: self.value.resolve(write)?,
        })
    }
}

impl<R> SolutionBranchData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SolutionBranchData<S>, Fault> {
        Ok(SolutionBranchData {
            problem: write(&self.problem)?,
            region: write(&self.region)?,
            domain: write(&self.domain)?,
            kind: self.kind.resolve(write)?,
        })
    }
}

impl<R> SolveProblemData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SolveProblemData<S>, Fault> {
        Ok(SolveProblemData {
            header: write(&self.header)?,
            scope: self.scope.clone(),
            predicate: write(&self.predicate)?,
            coefficients: write(&self.coefficients)?,
            unknowns: write(&self.unknowns)?,
            region: write(&self.region)?,
            domain: write(&self.domain)?,
            coefficient_domain: write(&self.coefficient_domain)?,
            ambient: self.ambient.clone(),
            denominators: map_vector(&self.denominators, write)?,
        })
    }
}

impl<R> LocalBranchData<R> {
    /// Resolve every child through the current transaction.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<LocalBranchData<S>, Fault> {
        Ok(LocalBranchData {
            problem: write(&self.problem)?,
            base_coefficients: write(&self.base_coefficients)?,
            base_unknowns: write(&self.base_unknowns)?,
            neighborhood: self.neighborhood.resolve(write)?,
            family_symbol: self.family_symbol.clone(),
            jacobian: write(&self.jacobian)?,
            inverse: write(&self.inverse)?,
        })
    }
}

impl<R> CoefficientDomainData<R> {
    /// Resolve each typed variant without introducing forward references.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoefficientDomainData<S>, Fault> {
        Ok(match self {
            Self::Rational => CoefficientDomainData::Rational,
            Self::Gaussian => CoefficientDomainData::Gaussian,
            Self::Fraction { base, coordinates } => CoefficientDomainData::Fraction {
                base: write(base)?,
                coordinates: write(coordinates)?,
            },
            Self::Quotient { level } => CoefficientDomainData::Quotient {
                level: write(level)?,
            },
        })
    }
}

impl<R> MapTarget<R> {
    /// Resolve each typed variant without introducing forward references.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<MapTarget<S>, Fault> {
        Ok(match self {
            Self::Family { family } => MapTarget::Family {
                family: write(family)?,
            },
            Self::CoefficientDomain { domain } => MapTarget::CoefficientDomain {
                domain: write(domain)?,
            },
            Self::ScalarAmbient { header, field } => MapTarget::ScalarAmbient {
                header: write(header)?,
                field: field.clone(),
            },
        })
    }
}

impl<R> CoefficientValueData<R> {
    /// Resolve each typed variant without introducing forward references.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoefficientValueData<S>, Fault> {
        Ok(match self {
            Self::Rational { value } => CoefficientValueData::Rational {
                value: value.clone(),
            },
            Self::Gaussian { real, imaginary } => CoefficientValueData::Gaussian {
                real: real.clone(),
                imaginary: imaginary.clone(),
            },
            Self::Fraction {
                numerator,
                denominator,
            } => CoefficientValueData::Fraction {
                numerator: write(numerator)?,
                denominator: write(denominator)?,
            },
            Self::Tower { coefficients } => CoefficientValueData::Tower {
                coefficients: map_vector(coefficients, write)?,
            },
        })
    }
}

impl<R> SolutionBranchKind<R> {
    /// Resolve each typed variant without introducing forward references.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SolutionBranchKind<S>, Fault> {
        Ok(match self {
            Self::Finite { tuples } => SolutionBranchKind::Finite {
                tuples: map_vector(tuples, write)?,
            },
            Self::Image { family } => SolutionBranchKind::Image {
                family: write(family)?,
            },
            Self::Algebraic { selection } => SolutionBranchKind::Algebraic {
                selection: write(selection)?,
            },
            Self::Empty => SolutionBranchKind::Empty,
            Self::Local { branch } => SolutionBranchKind::Local {
                branch: write(branch)?,
            },
        })
    }
}

impl<R> Neighborhood<R> {
    /// Resolve each typed variant without introducing forward references.
    fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<Neighborhood<S>, Fault> {
        Ok(match self {
            Self::Existential {
                coefficient_binder,
                unknown_binder,
            } => Neighborhood::Existential {
                coefficient_binder: coefficient_binder.clone(),
                unknown_binder: unknown_binder.clone(),
            },
            Self::Explicit {
                coefficient_region,
                unknown_region,
            } => Neighborhood::Explicit {
                coefficient_region: write(coefficient_region)?,
                unknown_region: write(unknown_region)?,
            },
        })
    }
}

impl<R> SolveDraft<R> {
    /// Resolve all earlier typed references before publication.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SolveDraft<S>, Fault> {
        Ok(match self {
            Self::CoordinateSpace(value) => SolveDraft::CoordinateSpace(value.resolve(write)?),
            Self::Tuple(value) => SolveDraft::Tuple(value.resolve(write)?),
            Self::SolutionSet(value) => SolveDraft::SolutionSet(value.resolve(write)?),
            Self::FamilyMap(value) => SolveDraft::FamilyMap(value.resolve(write)?),
            Self::CoefficientDomain(value) => SolveDraft::CoefficientDomain(value.resolve(write)?),
            Self::Polynomial(value) => SolveDraft::Polynomial(value.resolve(write)?),
            Self::ExtensionLevel(value) => SolveDraft::ExtensionLevel(value.resolve(write)?),
            Self::CoefficientMap(value) => SolveDraft::CoefficientMap(value.resolve(write)?),
            Self::RootRelation(value) => SolveDraft::RootRelation(value.resolve(write)?),
            Self::RootSelection(value) => SolveDraft::RootSelection(value.resolve(write)?),
            Self::ComponentSplit(value) => SolveDraft::ComponentSplit(value.resolve(write)?),
            Self::CoefficientValue(value) => SolveDraft::CoefficientValue(value.resolve(write)?),
            Self::SolutionBranch(value) => SolveDraft::SolutionBranch(value.resolve(write)?),
            Self::SolveProblem(value) => SolveDraft::SolveProblem(value.resolve(write)?),
            Self::LocalBranch(value) => SolveDraft::LocalBranch(value.resolve(write)?),
            Self::ParameterBase(value) => SolveDraft::ParameterBase(value.resolve(write)?),
            Self::SpaceFamily(value) => SolveDraft::SpaceFamily(value.resolve(write)?),
            Self::FamilyExpression(value) => SolveDraft::FamilyExpression(value.resolve(write)?),
            Self::FibrePiece(value) => SolveDraft::FibrePiece(value.resolve(write)?),
            Self::RealCell(value) => SolveDraft::RealCell(value.resolve(write)?),
            Self::CoefficientEvaluation(value) => {
                SolveDraft::CoefficientEvaluation(value.resolve(write)?)
            }
            Self::ScalarSpace(value) => SolveDraft::ScalarSpace(value.clone()),
            Self::SpacePlacement(value) => SolveDraft::SpacePlacement(value.resolve(write)?),
            Self::ScalarPiece(value) => SolveDraft::ScalarPiece(value.resolve(write)?),
        })
    }
}

impl<R> CoordinateSpaceData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.extend(self.scalar_spaces.iter().filter_map(Option::as_ref));
    }
}

impl<R> TupleData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.coordinates);
        for item in &self.entries {
            result.push(item);
        }
    }
}

impl<R> SolutionSetData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        for item in &self.branches {
            result.push(item);
        }
    }
}

impl<R> FamilyMapData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.coefficients);
        result.push(&self.family);
        result.push(&self.region);
        result.push(&self.target);
        result.push(&self.body);
    }
}

impl<R> PolynomialTerm<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.coefficient);
    }
}

impl<R> PolynomialData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.domain);
        for item in &self.terms {
            item.references(result);
        }
    }
}

impl<R> ExtensionLevelData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.parent);
        result.push(&self.polynomial);
    }
}

impl<R> CoefficientMapData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.source);
        self.target.references(result);
        result.push(&self.region);
        for item in &self.images {
            result.push(item);
        }
        for item in &self.denominators {
            result.push(item);
        }
    }
}

impl<R> RootRelationData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.polynomial);
        result.push(&self.map);
        result.push(&self.region);
    }
}

impl<R> RootSelectionData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        if let Some(cell) = &self.cell {
            result.push(cell);
        }
        result.push(&self.header);
        for item in &self.relations {
            result.push(item);
        }
        result.push(&self.predicate);
    }
}

impl<R> ComponentSplitData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.parent);
        result.push(&self.left_factor);
        result.push(&self.right_factor);
        result.push(&self.left_domain);
        result.push(&self.right_domain);
        result.push(&self.left_map);
        result.push(&self.right_map);
        result.push(&self.region);
    }
}

impl<R> CoefficientValueObject<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.domain);
        self.value.references(result);
    }
}

impl<R> SolutionBranchData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.region);
        result.push(&self.domain);
        self.kind.references(result);
    }
}

impl<R> SolveProblemData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.predicate);
        result.push(&self.coefficients);
        result.push(&self.unknowns);
        result.push(&self.region);
        result.push(&self.domain);
        result.push(&self.coefficient_domain);
        for item in &self.denominators {
            result.push(item);
        }
    }
}

impl<R> LocalBranchData<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.base_coefficients);
        result.push(&self.base_unknowns);
        self.neighborhood.references(result);
        result.push(&self.jacobian);
        result.push(&self.inverse);
    }
}

impl<R> RelationMultiple<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.multiplier);
        result.push(&self.relation);
        if let Some(hypothesis) = &self.hypothesis {
            hypothesis.references(result);
        }
    }
}

impl<R> ScopedCoefficientRelation<R> {
    /// Preserve both polynomial endpoints and the explicit scalar root order as dependencies.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.left);
        result.push(&self.right);
        result.extend(&self.roots);
        result.extend(self.vieta.as_ref());
    }
}

impl<R> SolveSubstitution<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.expression);
    }
}

impl<R> RootFactor<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.root);
    }
}

impl<R> GeneratorPair<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.original);
        result.push(&self.conjugate);
    }
}

impl<R> StatementMember<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.tuple);
        result.push(&self.region);
    }
}

impl<R> StatementEquivalent<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.source);
        result.push(&self.target);
        result.push(&self.forward);
        result.push(&self.backward);
        result.push(&self.region);
    }
}

impl<R> StatementCoverage<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.set);
        result.push(&self.region);
    }
}

impl<R> StatementFamilyMembership<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.family);
        result.push(&self.region);
    }
}

impl<R> StatementFamilyCoverage<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.family);
        result.push(&self.recovery);
        result.push(&self.region);
    }
}

impl<R> StatementRank<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.matrix);
        result.push(&self.domain);
        result.push(&self.minor);
        result.push(&self.elimination);
    }
}

impl<R> StatementLinearSystem<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.reduced);
        result.push(&self.region);
    }
}

impl<R> StatementLocalBranch<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.branch);
        self.neighborhood.references(result);
    }
}

impl<R> StatementLocalDerivative<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.branch);
        result.push(&self.derivative);
        self.neighborhood.references(result);
    }
}

impl<R> StatementCoefficientEquality<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.left);
        result.push(&self.right);
        result.push(&self.map);
        result.push(&self.region);
    }
}

impl<R> StatementUnit<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.value);
        result.push(&self.inverse);
        result.push(&self.domain);
        result.push(&self.region);
    }
}

impl<R> StatementComponentCover<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.split);
        result.push(&self.region);
    }
}

impl<R> StatementRootExistence<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.region);
    }
}

impl<R> StatementSelectionUnique<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.selection);
        result.push(&self.region);
    }
}

impl<R> StatementRootOrder<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.value);
        result.push(&self.region);
    }
}

impl<R> StatementOrbit<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.seed);
        for item in &self.members {
            result.push(item);
        }
    }
}

impl<R> StatementMultiplicity<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.selection);
        result.push(&self.region);
    }
}

impl<R> StatementRealValues<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        for item in &self.values {
            result.push(item);
        }
        result.push(&self.region);
    }
}

impl<R> StatementIsolation<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.region);
    }
}

impl<R> StatementPresentation<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.set);
        result.push(&self.region);
    }
}

impl<R> CertificateCoefficientIdentity<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.left);
        result.push(&self.right);
        result.push(&self.map);
        result.push(&self.region);
        for item in &self.combinations {
            item.references(result);
        }
        for item in &self.denominators {
            result.push(item);
        }
    }
}

impl<R> CertificateRootAdjunction<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        if let Some(item) = &self.selection {
            result.push(item);
        }
        for item in &self.splitting_chain {
            result.push(item);
        }
    }
}

impl<R> CertificateExtensionUnit<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.element);
        result.push(&self.modulus);
        result.push(&self.gcd);
        for item in &self.quotients {
            result.push(item);
        }
        for item in &self.bezout {
            result.push(item);
        }
        result.push(&self.region);
    }
}

impl<R> CertificateComponentSplit<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.split);
        for item in &self.bezout {
            result.push(item);
        }
        for item in &self.selections {
            result.push(item);
        }
    }
}

impl<R> CertificateMembership<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.tuple);
        result.push(&self.region);
        for item in &self.substitutions {
            item.references(result);
        }
    }
}

impl<R> CertificateSolutionTransport<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.source);
        result.push(&self.target);
        result.push(&self.forward);
        result.push(&self.backward);
        result.push(&self.region);
    }
}

impl<R> CertificateTotalPredicateSplit<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.expression);
        result.push(&self.region);
        for item in &self.children {
            result.push(item);
        }
    }
}

impl<R> CertificateBinomialCoverage<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        if let Some(item) = &self.unity {
            result.push(item);
        }
        if let Some(item) = &self.selection {
            result.push(item);
        }
    }
}

impl<R> CertificateRootSelectionRefinement<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.target);
        for item in &self.witnesses {
            result.push(item);
        }
    }
}

impl<R> CertificateExactLinearEquivalence<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.matrix);
        for item in &self.operations {
            item.references(result);
        }
        result.push(&self.minor);
        result.push(&self.reduced);
        result.push(&self.region);
    }
}

impl<R> CertificateProductCompleteness<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.set);
        result.push(&self.scalar);
        for item in &self.factors {
            item.references(result);
        }
        result.push(&self.region);
    }
}

impl<R> CertificateMultiplicityTransfer<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.selection);
        for item in &self.values {
            result.push(item);
        }
        result.push(&self.region);
    }
}

impl<R> CertificateConjugationTransport<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.map);
        result.push(&self.conjugate_map);
        for item in &self.images {
            item.references(result);
        }
        for item in &self.values {
            result.push(item);
        }
        result.push(&self.region);
    }
}

impl<R> CertificatePermutationOrbitCoverage<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.seed);
        for item in &self.members {
            result.push(item);
        }
    }
}

impl<R> CertificateRealIsolationAndOrder<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        for item in &self.sturm {
            result.push(item);
        }
    }
}

impl<R> CertificateParameterImageCoverage<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.problem);
        result.push(&self.family);
        if let Some(item) = &self.recovery {
            result.push(item);
        }
        result.push(&self.region);
    }
}

impl<R> CertificateImplicitFunctionBranch<R> {
    /// Enumerate every referenced object in semantic field order.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.branch);
        if let Some(item) = &self.derivative {
            result.push(item);
        }
    }
}

impl<R> CoefficientDomainData<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Rational | Self::Gaussian => {}
            Self::Fraction { base, coordinates } => {
                result.push(base);
                result.push(coordinates);
            }
            Self::Quotient { level } => {
                result.push(level);
            }
        }
    }
}

impl<R> MapTarget<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Family { family } => result.push(family),
            Self::CoefficientDomain { domain } => {
                result.push(domain);
            }
            Self::ScalarAmbient { header, .. } => {
                result.push(header);
            }
        }
    }
}

impl<R> CoefficientValueData<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Rational { .. } | Self::Gaussian { .. } => {}
            Self::Fraction {
                numerator,
                denominator,
            } => {
                result.push(numerator);
                result.push(denominator);
            }
            Self::Tower { coefficients } => {
                for item in coefficients {
                    result.push(item);
                }
            }
        }
    }
}

impl<R> SolutionBranchKind<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Finite { tuples } => {
                for item in tuples {
                    result.push(item);
                }
            }
            Self::Image { family } => {
                result.push(family);
            }
            Self::Algebraic { selection } => {
                result.push(selection);
            }
            Self::Empty => {}
            Self::Local { branch } => {
                result.push(branch);
            }
        }
    }
}

impl<R> Neighborhood<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Existential { .. } => {}
            Self::Explicit {
                coefficient_region,
                unknown_region,
            } => {
                result.push(coefficient_region);
                result.push(unknown_region);
            }
        }
    }
}

impl<R> RowOperation<R> {
    /// Enumerate every typed child without copying its payload.
    fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        match self {
            Self::Swap { .. } => {}
            Self::Scale { value, .. } | Self::AddMultiple { value, .. } => {
                result.push(value);
            }
        }
    }
}

impl<R> SolveDraft<R> {
    /// Borrow every earlier semantic object reference.
    pub(crate) fn objects(&self) -> Vec<&R> {
        let mut result = Vec::new();
        match self {
            Self::CoordinateSpace(value) => value.references(&mut result),
            Self::Tuple(value) => value.references(&mut result),
            Self::SolutionSet(value) => value.references(&mut result),
            Self::FamilyMap(value) => value.references(&mut result),
            Self::CoefficientDomain(value) => value.references(&mut result),
            Self::Polynomial(value) => value.references(&mut result),
            Self::ExtensionLevel(value) => value.references(&mut result),
            Self::CoefficientMap(value) => value.references(&mut result),
            Self::RootRelation(value) => value.references(&mut result),
            Self::RootSelection(value) => value.references(&mut result),
            Self::ComponentSplit(value) => value.references(&mut result),
            Self::CoefficientValue(value) => value.references(&mut result),
            Self::SolutionBranch(value) => value.references(&mut result),
            Self::SolveProblem(value) => value.references(&mut result),
            Self::LocalBranch(value) => value.references(&mut result),
            Self::ParameterBase(value) => value.references(&mut result),
            Self::SpaceFamily(value) => value.references(&mut result),
            Self::FamilyExpression(value) => value.references(&mut result),
            Self::FibrePiece(value) => value.references(&mut result),
            Self::RealCell(value) => value.references(&mut result),
            Self::CoefficientEvaluation(value) => value.references(&mut result),
            Self::ScalarSpace(_) => {}
            Self::SpacePlacement(value) => value.references(&mut result),
            Self::ScalarPiece(value) => value.references(&mut result),
        }
        result
    }
}

impl<R> SolveStatement<R> {
    /// Borrow every earlier semantic object reference.
    pub(crate) fn objects(&self) -> Vec<&R> {
        let mut result = Vec::new();
        match self {
            Self::Member(value) => value.references(&mut result),
            Self::Equivalent(value) => value.references(&mut result),
            Self::Coverage(value) => value.references(&mut result),
            Self::FamilyMembership(value) => value.references(&mut result),
            Self::FamilyCoverage(value) => value.references(&mut result),
            Self::Rank(value) => value.references(&mut result),
            Self::LinearSystem(value) => value.references(&mut result),
            Self::LocalBranch(value) => value.references(&mut result),
            Self::LocalDerivative(value) => value.references(&mut result),
            Self::CoefficientEquality(value) => value.references(&mut result),
            Self::Unit(value) => value.references(&mut result),
            Self::ComponentCover(value) => value.references(&mut result),
            Self::RootExistence(value) => value.references(&mut result),
            Self::SelectionUnique(value) => value.references(&mut result),
            Self::RootOrder(value) => value.references(&mut result),
            Self::Orbit(value) => value.references(&mut result),
            Self::Multiplicity(value) => value.references(&mut result),
            Self::RealValues(value) => value.references(&mut result),
            Self::Isolation(value) => value.references(&mut result),
            Self::Presentation(value) => value.references(&mut result),
            Self::CoefficientEvaluation(value) => value.references(&mut result),
            Self::RealSign(value) => value.references(&mut result),
            Self::RealRootCount(value) => value.references(&mut result),
            Self::RealCellExists(value) => value.references(&mut result),
        }
        result
    }
}

impl<R> SolveCertificate<R> {
    /// Borrow every earlier semantic object reference.
    pub(crate) fn objects(&self) -> Vec<&R> {
        let mut result = Vec::new();
        match self {
            Self::CoefficientIdentity(value) => value.references(&mut result),
            Self::RootAdjunction(value) => value.references(&mut result),
            Self::ExtensionUnit(value) => value.references(&mut result),
            Self::ComponentSplit(value) => value.references(&mut result),
            Self::Membership(value) => value.references(&mut result),
            Self::SolutionTransport(value) => value.references(&mut result),
            Self::TotalPredicateSplit(value) => value.references(&mut result),
            Self::BinomialCoverage(value) => value.references(&mut result),
            Self::RootSelectionRefinement(value) => value.references(&mut result),
            Self::ExactLinearEquivalence(value) => value.references(&mut result),
            Self::ProductCompleteness(value) => value.references(&mut result),
            Self::MultiplicityTransfer(value) => value.references(&mut result),
            Self::ConjugationTransport(value) => value.references(&mut result),
            Self::PermutationOrbitCoverage(value) => value.references(&mut result),
            Self::RealIsolationAndOrder(value) => value.references(&mut result),
            Self::ParameterImageCoverage(value) => value.references(&mut result),
            Self::ImplicitFunctionBranch(value) => value.references(&mut result),
            Self::CoefficientEvaluation(value) => value.references(&mut result),
            Self::RealSign(value) => value.references(&mut result),
            Self::RealRootCount(value) => value.references(&mut result),
            Self::RealCellExists(value) => value.references(&mut result),
        }
        result
    }
}

impl SolveDraft<Handle> {
    /// Stable graph namespace, separate from the wire record tag.
    pub(crate) const fn kind(&self) -> VertexKind {
        match self {
            Self::CoordinateSpace(_) => VertexKind::CoordinateSpace,
            Self::Tuple(_) => VertexKind::Tuple,
            Self::SolutionSet(_) => VertexKind::SolutionSet,
            Self::FamilyMap(_) => VertexKind::FamilyMap,
            Self::CoefficientDomain(_) => VertexKind::CoefficientDomain,
            Self::Polynomial(_) => VertexKind::Polynomial,
            Self::ExtensionLevel(_) => VertexKind::ExtensionLevel,
            Self::CoefficientMap(_) => VertexKind::CoefficientMap,
            Self::RootRelation(_) => VertexKind::RootRelation,
            Self::RootSelection(_) => VertexKind::RootSelection,
            Self::ComponentSplit(_) => VertexKind::ComponentSplit,
            Self::CoefficientValue(_) => VertexKind::CoefficientValue,
            Self::SolutionBranch(_) => VertexKind::SolutionBranch,
            Self::SolveProblem(_) => VertexKind::SolveProblem,
            Self::LocalBranch(_) => VertexKind::LocalBranch,
            Self::ParameterBase(_) => VertexKind::ParameterBase,
            Self::SpaceFamily(_) => VertexKind::SpaceFamily,
            Self::FamilyExpression(_) => VertexKind::FamilyExpression,
            Self::FibrePiece(_) => VertexKind::FibrePiece,
            Self::RealCell(_) => VertexKind::RealCell,
            Self::CoefficientEvaluation(_) => VertexKind::CoefficientEvaluation,
            Self::ScalarSpace(_) => VertexKind::ScalarSpace,
            Self::SpacePlacement(_) => VertexKind::SpacePlacement,
            Self::ScalarPiece(_) => VertexKind::ScalarPiece,
        }
    }

    /// Borrow the exact graph payload without copying any unbounded collection.
    const fn view(&self) -> SolveView<'_> {
        match self {
            Self::CoordinateSpace(value) => SolveView::CoordinateSpace(value),
            Self::Tuple(value) => SolveView::Tuple(value),
            Self::SolutionSet(value) => SolveView::SolutionSet(value),
            Self::FamilyMap(value) => SolveView::FamilyMap(value),
            Self::CoefficientDomain(value) => SolveView::CoefficientDomain(value),
            Self::Polynomial(value) => SolveView::Polynomial(value),
            Self::ExtensionLevel(value) => SolveView::ExtensionLevel(value),
            Self::CoefficientMap(value) => SolveView::CoefficientMap(value),
            Self::RootRelation(value) => SolveView::RootRelation(value),
            Self::RootSelection(value) => SolveView::RootSelection(value),
            Self::ComponentSplit(value) => SolveView::ComponentSplit(value),
            Self::CoefficientValue(value) => SolveView::CoefficientValue(value),
            Self::SolutionBranch(value) => SolveView::SolutionBranch(value),
            Self::SolveProblem(value) => SolveView::SolveProblem(value),
            Self::LocalBranch(value) => SolveView::LocalBranch(value),
            Self::ParameterBase(value) => SolveView::ParameterBase(value),
            Self::SpaceFamily(value) => SolveView::SpaceFamily(value),
            Self::FamilyExpression(value) => SolveView::FamilyExpression(value),
            Self::FibrePiece(value) => SolveView::FibrePiece(value),
            Self::RealCell(value) => SolveView::RealCell(value),
            Self::CoefficientEvaluation(value) => SolveView::CoefficientEvaluation(value),
            Self::ScalarSpace(value) => SolveView::ScalarSpace(value),
            Self::SpacePlacement(value) => SolveView::SpacePlacement(value),
            Self::ScalarPiece(value) => SolveView::ScalarPiece(value),
        }
    }
}

/// Read a solving object with its exact graph kind preserved.
pub(crate) fn object(state: &State, id: LogicalId) -> Result<&SolveDraft<Handle>, Fault> {
    match state.node(id)? {
        Vertex::Solve(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Product coordinates have one checked declaration per slot.
pub(crate) fn coordinates(
    state: &State,
    id: LogicalId,
) -> Result<&CoordinateSpaceData<Handle>, Fault> {
    match object(state, id)? {
        SolveDraft::CoordinateSpace(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Original problem data remains independent of any request or proof.
pub(crate) fn problem(state: &State, id: LogicalId) -> Result<&SolveProblemData<Handle>, Fault> {
    match object(state, id)? {
        SolveDraft::SolveProblem(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Look up the exact coefficient algebra, without claiming it is a field.
fn coefficient_domain<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a CoefficientDomainData<Handle>, Fault> {
    match object(state, value.logical_id())? {
        SolveDraft::CoefficientDomain(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a canonical polynomial independently of its intended theorem.
fn polynomial<'a>(state: &'a State, value: &Handle) -> Result<&'a PolynomialData<Handle>, Fault> {
    match object(state, value.logical_id())? {
        SolveDraft::Polynomial(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a coefficient with its exact domain link.
fn coefficient<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a CoefficientValueObject<Handle>, Fault> {
    match object(state, value.logical_id())? {
        SolveDraft::CoefficientValue(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow the defining relation of an earlier extension level.
fn level<'a>(state: &'a State, value: &Handle) -> Result<&'a ExtensionLevelData<Handle>, Fault> {
    match object(state, value.logical_id())? {
        SolveDraft::ExtensionLevel(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Check a specific dependency kind before interpreting its payload.
fn require_kind(value: &Handle, kind: VertexKind) -> Result<(), Fault> {
    if value.logical_id().kind == kind {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Exact headers and scopes prevent accidental capture through display names.
fn metadata_matches(
    state: &State,
    value: &Handle,
    header: &Handle,
    scope: &Scope,
) -> Result<(), Fault> {
    let (actual, frames) = crate::descriptor::metadata(state, value.logical_id())?;
    if actual != header.logical_id() || frames != scope {
        return Err(invalid());
    }
    Ok(())
}

/// Check a product region against the intended coordinate order.
fn product_region(
    state: &State,
    region: &Handle,
    header: &Handle,
    scope: &Scope,
    product: &Handle,
) -> Result<(), Fault> {
    let value = state.region(region.logical_id())?;
    if value.header != header.logical_id()
        || value.scope != *scope
        || value.ambient
            != (crate::domain_model::AmbientDomain::Product {
                coordinates: product.clone(),
            })
    {
        return Err(invalid());
    }
    Ok(())
}

/// Every coordinate is declared, distinct and in the exact requested role.
fn coordinate_role(
    state: &State,
    product: &Handle,
    header: &Handle,
    role: CoordinateRole,
) -> Result<(), Fault> {
    let value = coordinates(state, product.logical_id())?;
    if value.header != *header || value.coordinates.iter().any(|entry| entry.role != role) {
        return Err(invalid());
    }
    Ok(())
}

/// Validate coordinate declarations without reordering the product.
fn check_coordinates(
    state: &State,
    value: &CoordinateSpaceData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let environment = &state.header(value.header.logical_id())?.environment;
    budget.check(BudgetKind::Edges, size(value.coordinates.len(), AT)?, AT)?;
    let mut seen = std::collections::BTreeSet::new();
    for coordinate in &value.coordinates {
        budget.work(
            size(environment.declarations.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if !seen.insert(&coordinate.key) {
            return Err(invalid());
        }
        let declaration = environment
            .declarations
            .iter()
            .find(|declaration| declaration.key == coordinate.key)
            .ok_or_else(invalid)?;
        if declaration.sort != Sort::Value(AtomicSort::Scalar(coordinate.field.clone())) {
            return Err(Fault::invalid(
                InvalidCode::SymbolSortMismatch,
                Location::new("declared coordinate slot"),
            ));
        }
    }
    Ok(())
}

/// Tuple entries have exactly the product's order, field, header and scope.
fn check_tuple(state: &State, value: &TupleData<Handle>, budget: &mut Budget) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    let product = coordinates(state, value.coordinates.logical_id())?;
    let expected_entries = product.coordinates.len();
    if product.header != value.header || value.entries.len() != expected_entries {
        return Err(invalid());
    }
    for (coordinate, entry) in product.coordinates.iter().zip(&value.entries) {
        budget.work(1, AT)?;
        metadata_matches(state, entry, &value.header, &value.scope)?;
        if state.expression(entry.logical_id())?.sort
            != Sort::Value(AtomicSort::Scalar(coordinate.field.clone()))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A family introduces only its declared family frame, leaving coefficients free.
fn check_family(
    state: &State,
    value: &FamilyMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    coordinate_role(
        state,
        &value.coefficients,
        &value.header,
        CoordinateRole::Coefficient,
    )?;
    let source_role = coordinates(state, value.family.logical_id())?
        .coordinates
        .first()
        .map_or(CoordinateRole::Family, |entry| entry.role);
    let target_role = match source_role {
        CoordinateRole::Family => CoordinateRole::Unknown,
        CoordinateRole::Unknown => CoordinateRole::Family,
        CoordinateRole::Coefficient => return Err(invalid()),
    };
    coordinate_role(state, &value.family, &value.header, source_role)?;
    coordinate_role(state, &value.target, &value.header, target_role)?;
    let family = coordinates(state, value.family.logical_id())?;
    budget.solving(
        BudgetKind::FamilyParameters,
        size(family.coordinates.len(), AT)?,
        AT,
    )?;
    let mut scope = value.scope.clone();
    scope.push(
        family
            .coordinates
            .iter()
            .map(|entry| AtomicSort::Scalar(entry.field.clone()))
            .collect(),
    );
    product_region(state, &value.region, &value.header, &scope, &value.family)?;
    let SolveDraft::Tuple(body) = object(state, value.body.logical_id())? else {
        return Err(invalid());
    };
    if body.header != value.header || body.scope != scope || body.coordinates != value.target {
        return Err(invalid());
    }
    let coefficient_product = coordinates(state, value.coefficients.logical_id())?;
    let allowed: std::collections::BTreeSet<_> = coefficient_product
        .coordinates
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    free_keys(state, &value.region, &allowed, true, budget)?;
    free_keys(state, &value.body, &allowed, true, budget)
}

/// Inspect reachable scalar dependencies, including selected roots, maps and case domains.
pub(crate) fn free_keys(
    state: &State,
    root: &Handle,
    keys: &std::collections::BTreeSet<&str>,
    allow: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vec![root.logical_id()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        if !seen.insert(id) {
            continue;
        }
        if let Vertex::Expression(value) = state.node(id)?
            && let crate::model::Operator::Free(key) = &value.operator
            && keys.contains(key.as_str()) != allow
        {
            return Err(invalid());
        }
        free_children(state, id, &mut pending, budget)?;
        budget.check(BudgetKind::Edges, size(pending.len(), AT)?, AT)?;
    }
    Ok(())
}

/// Extend the free-key walk without treating declaration metadata as a free scalar use.
fn free_children(
    state: &State,
    id: LogicalId,
    pending: &mut Vec<LogicalId>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match state.node(id)? {
        Vertex::Header(_) => {}
        Vertex::Solve(value) => {
            let index = state.indices.get(&id).ok_or_else(invalid)?;
            let count = state.graph.edges(*index).count();
            let bytes = size(count, AT)?
                .checked_mul(64)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            budget.work(bytes, AT)?;
            pending
                .try_reserve(count)
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend(value.objects().into_iter().map(Handle::logical_id));
        }
        value => {
            let children = crate::descriptor::children(state, id, budget)?;
            pending
                .try_reserve(children.len().checked_add(2).ok_or_else(invalid)?)
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend(children.into_iter().map(|(_, child)| child));
            if let Vertex::Expression(expression) = value {
                match &expression.attachment {
                    Some(crate::graph::ExpressionAttachment::SelectedRoot(selection)) => {
                        pending.push(*selection);
                    }
                    Some(crate::graph::ExpressionAttachment::ExactCoefficient { value, map }) => {
                        pending.extend([*value, *map]);
                    }
                    None => {}
                }
            }
        }
    }
    Ok(())
}

/// Fraction fields are limited to independent coordinates over Q or Q(i).
fn check_domain(state: &State, value: &CoefficientDomainData<Handle>) -> Result<(), Fault> {
    match value {
        CoefficientDomainData::Rational | CoefficientDomainData::Gaussian => Ok(()),
        CoefficientDomainData::Fraction {
            base,
            coordinates: product,
        } => {
            if !matches!(
                coefficient_domain(state, base)?,
                CoefficientDomainData::Rational | CoefficientDomainData::Gaussian
            ) {
                return Err(invalid());
            }
            let product = coordinates(state, product.logical_id())?;
            if product
                .coordinates
                .iter()
                .any(|entry| entry.role != CoordinateRole::Coefficient)
            {
                return Err(invalid());
            }
            Ok(())
        }
        CoefficientDomainData::Quotient { level: extension } => {
            level(state, extension)?;
            Ok(())
        }
    }
}

/// Recognize formal zero without inferring specialized nonvanishing.
fn is_zero(state: &State, value: &Handle) -> Result<bool, Fault> {
    Ok(match &coefficient(state, value)?.value {
        CoefficientValueData::Rational { value } => value.is_zero(),
        CoefficientValueData::Gaussian { real, imaginary } => real.is_zero() && imaginary.is_zero(),
        CoefficientValueData::Fraction { numerator, .. } => {
            polynomial(state, numerator)?.terms.is_empty()
        }
        CoefficientValueData::Tower { coefficients } => coefficients.is_empty(),
    })
}

/// Recognize canonical one through parent constants without assuming any other element is a unit.
fn is_one(state: &State, value: &Handle, budget: &mut Budget) -> Result<bool, Fault> {
    let mut pending = vec![value];
    while let Some(current) = pending.pop() {
        budget.work(1, AT)?;
        match &coefficient(state, current)?.value {
            CoefficientValueData::Rational { value } => {
                if *value != crate::number::Rational::one() {
                    return Ok(false);
                }
            }
            CoefficientValueData::Gaussian { real, imaginary } => {
                if *real != crate::number::Rational::one() || !imaginary.is_zero() {
                    return Ok(false);
                }
            }
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => {
                let Some(numerator) = constant_coefficient(state, numerator, budget)? else {
                    return Ok(false);
                };
                let Some(denominator) = constant_coefficient(state, denominator, budget)? else {
                    return Ok(false);
                };
                pending.try_reserve(2).map_err(|_| Fault::allocation(AT))?;
                pending.extend([numerator, denominator]);
            }
            CoefficientValueData::Tower { coefficients } => {
                let [constant] = coefficients.as_slice() else {
                    return Ok(false);
                };
                pending.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                pending.push(constant);
            }
        }
    }
    Ok(true)
}

/// A formal constant has exactly one zero-exponent term in the declared variable frame.
fn constant_coefficient<'a>(
    state: &'a State,
    value: &Handle,
    budget: &mut Budget,
) -> Result<Option<&'a Handle>, Fault> {
    let polynomial_data = polynomial(state, value)?;
    let [term] = polynomial_data.terms.as_slice() else {
        return Ok(None);
    };
    budget.work(size(term.exponents.len(), AT)?, AT)?;
    Ok(term
        .exponents
        .iter()
        .all(|exponent| *exponent == 0)
        .then_some(&term.coefficient))
}

/// Check sparse term order and all coefficient/variable positions before publication.
fn check_polynomial(
    state: &State,
    value: &PolynomialData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    coefficient_domain(state, &value.domain)?;
    budget.check(
        BudgetKind::PolynomialVariables,
        size(value.variables.len(), AT)?,
        AT,
    )?;
    budget.check(
        BudgetKind::PolynomialTerms,
        size(value.terms.len(), AT)?,
        AT,
    )?;
    let mut previous = None;
    for term in &value.terms {
        budget.work(
            size(term.exponents.len(), AT)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if term.exponents.len() != value.variables.len()
            || coefficient(state, &term.coefficient)?.domain != value.domain
            || is_zero(state, &term.coefficient)?
        {
            return Err(invalid());
        }
        let degree = term
            .exponents
            .iter()
            .try_fold(0_u64, |total, item| total.checked_add(u64::from(*item)))
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
        let key = (degree, &term.exponents);
        if previous.is_some_and(|earlier| earlier <= key) {
            return Err(invalid());
        }
        previous = Some(key);
    }
    for field in &value.variables {
        if !matches!(field, Field::Rational | Field::Real | Field::Complex) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A univariate degree is explicit and positive where a root theorem could apply.
fn degree(state: &State, value: &Handle) -> Result<u32, Fault> {
    let value = polynomial(state, value)?;
    if value.variables.len() != 1 {
        return Err(invalid());
    }
    value
        .terms
        .first()
        .and_then(|term| term.exponents.first())
        .copied()
        .ok_or_else(invalid)
}

/// A defining polynomial is monic over its earlier parent, without asserting reducedness.
fn check_level(
    state: &State,
    value: &ExtensionLevelData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    coefficient_domain(state, &value.parent)?;
    let polynomial_data = polynomial(state, &value.polynomial)?;
    if polynomial_data.domain != value.parent
        || degree(state, &value.polynomial)? == 0
        || value.generator.len() != 1
        || value.generator.first()
            != polynomial_data
                .variables
                .first()
                .map(|field| AtomicSort::Scalar(field.clone()))
                .as_ref()
        || !is_one(
            state,
            &polynomial_data
                .terms
                .first()
                .ok_or_else(invalid)?
                .coefficient,
            budget,
        )?
    {
        return Err(invalid());
    }
    tower_limits(state, value, budget)?;
    crate::validation::scope_limit(&vec![value.generator.clone()], budget)
}

/// Observe extension depth and degree product before constructing a new tower level.
fn tower_limits(
    state: &State,
    value: &ExtensionLevelData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut depth = 1_u64;
    let mut product = u64::from(degree(state, &value.polynomial)?);
    let mut parent = &value.parent;
    loop {
        budget.solving(BudgetKind::TowerLevels, depth, AT)?;
        budget.solving(BudgetKind::TowerDegree, product, AT)?;
        let CoefficientDomainData::Quotient { level: extension } =
            coefficient_domain(state, parent)?
        else {
            break;
        };
        let ancestor = level(state, extension)?;
        depth = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::TowerLevels, AT))?;
        product = product
            .checked_mul(u64::from(degree(state, &ancestor.polynomial)?))
            .ok_or_else(|| Fault::capacity(BudgetKind::TowerDegree, AT))?;
        parent = &ancestor.parent;
    }
    Ok(())
}

/// Values have the exact representation belonging to their declared algebra.
fn check_value(
    state: &State,
    value: &CoefficientValueObject<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match (coefficient_domain(state, &value.domain)?, &value.value) {
        (CoefficientDomainData::Rational, CoefficientValueData::Rational { value }) => {
            value.check(budget, AT)
        }
        (CoefficientDomainData::Gaussian, CoefficientValueData::Gaussian { real, imaginary }) => {
            real.check(budget, AT)?;
            imaginary.check(budget, AT)
        }
        (
            CoefficientDomainData::Fraction {
                base,
                coordinates: product,
            },
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            },
        ) => check_fraction(state, base, product, numerator, denominator, budget),
        (
            CoefficientDomainData::Quotient { level: extension },
            CoefficientValueData::Tower { coefficients },
        ) => {
            let extension = level(state, extension)?;
            if size(coefficients.len(), AT)? > u64::from(degree(state, &extension.polynomial)?) {
                return Err(invalid());
            }
            for entry in coefficients {
                if coefficient(state, entry)?.domain != extension.parent {
                    return Err(invalid());
                }
            }
            if let Some(last) = coefficients.last()
                && is_zero(state, last)?
            {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Only manifestly coprime denominator-one fractions precede the normalization lane.
fn check_fraction(
    state: &State,
    base: &Handle,
    product: &Handle,
    numerator: &Handle,
    denominator: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let numerator_data = polynomial(state, numerator)?;
    let denominator_data = polynomial(state, denominator)?;
    let product = coordinates(state, product.logical_id())?;
    let fields: Vec<_> = product
        .coordinates
        .iter()
        .map(|entry| entry.field.clone())
        .collect();
    if numerator_data.domain != *base
        || denominator_data.domain != *base
        || numerator_data.variables != fields
        || denominator_data.variables != fields
    {
        return Err(invalid());
    }
    let Some(term) = denominator_data.terms.first() else {
        return Err(invalid());
    };
    if denominator_data.terms.len() != 1
        || term.exponents.iter().any(|exponent| *exponent != 0)
        || !is_one(state, &term.coefficient, budget)?
    {
        return super::coefficients::canonical_fraction(state, numerator, denominator, budget);
    }
    Ok(())
}

/// Scalar targets retain their exact header and field, while domain targets remain algebraic.
fn check_map(
    state: &State,
    value: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    coefficient_domain(state, &value.source)?;
    let region = state.region(value.region.logical_id())?;
    match &value.target {
        MapTarget::Family { .. } => super::family::mapping::validate(state, value, budget)?,
        MapTarget::CoefficientDomain { domain } => {
            coefficient_domain(state, domain)?;
            for image in &value.images {
                if coefficient(state, image)?.domain != *domain {
                    return Err(invalid());
                }
            }
        }
        MapTarget::ScalarAmbient { header, field } => {
            if region.header != header.logical_id() {
                return Err(invalid());
            }
            for image in &value.images {
                metadata_matches(state, image, header, &region.scope)?;
                if state.expression(image.logical_id())?.sort
                    != Sort::Value(AtomicSort::Scalar(field.clone()))
                {
                    return Err(invalid());
                }
            }
        }
    }
    for denominator in &value.denominators {
        budget.work(1, AT)?;
        match state.node(denominator.logical_id())? {
            Vertex::Expression(expression)
                if expression.header == region.header
                    && expression.scope == region.scope
                    && matches!(expression.sort, Sort::Value(AtomicSort::Scalar(_))) => {}
            Vertex::Solve(data)
                if matches!(
                    data.as_ref(),
                    SolveDraft::Polynomial(_) | SolveDraft::CoefficientValue(_)
                ) => {}
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// Root syntax records a requested binder and the exact scalar coefficient map.
fn check_relation(
    state: &State,
    value: &RootRelationData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    let polynomial_data = polynomial(state, &value.polynomial)?;
    if polynomial_data.variables.len() != 1
        || value.binder != vec![AtomicSort::Scalar(value.ambient.clone())]
        || value.multiplicity == Some(0)
    {
        return Err(invalid());
    }
    let SolveDraft::CoefficientMap(map) = object(state, value.map.logical_id())? else {
        return Err(invalid());
    };
    if map.source != polynomial_data.domain
        || map.mode != MapMode::Specialization
        || map.region != value.region
        || !super::family::mapping::relation_target(state, map, &value.header, &value.ambient)?
    {
        return Err(invalid());
    }
    metadata_matches(state, &value.region, &value.header, &value.scope)
}

/// Principal/isolated metadata is relevant only to its own closed mode.
fn check_selection(
    state: &State,
    value: &RootSelectionData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    let mut binders = Vec::new();
    for relation in &value.relations {
        let SolveDraft::RootRelation(relation) = object(state, relation.logical_id())? else {
            return Err(invalid());
        };
        if relation.header != value.header || relation.scope != value.scope {
            return Err(invalid());
        }
        binders.push(AtomicSort::Scalar(relation.ambient.clone()));
    }
    let mut scope = value.scope.clone();
    scope.push(binders);
    require_kind(&value.predicate, VertexKind::Condition)?;
    metadata_matches(state, &value.predicate, &value.header, &scope)?;
    super::sorts::selection_cell(state, value)?;
    let parameters = selection_parameters(state, value, budget)?;
    free_keys(state, &value.predicate, &parameters, true, budget)?;
    if (value.mode == SelectionMode::Principal) != value.convention.is_some()
        || value.convention.as_ref().is_some_and(String::is_empty)
        || (value.mode == SelectionMode::Isolated)
            != (value.interval.is_some() || value.cell.is_some())
        || value.interval.is_some() && value.cell.is_some()
    {
        return Err(invalid());
    }
    if let Some(interval) = &value.interval {
        if value.relations.len() != 1 {
            return Err(invalid());
        }
        let SolveDraft::RootRelation(relation) = object(
            state,
            value.relations.first().ok_or_else(invalid)?.logical_id(),
        )?
        else {
            return Err(invalid());
        };
        if relation.ambient != Field::Real
            || interval.left.compare(&interval.right, budget)? != std::cmp::Ordering::Less
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A joint predicate may use root slots and the declared coefficient products only.
fn selection_parameters<'a>(
    state: &'a State,
    value: &RootSelectionData<Handle>,
    budget: &mut Budget,
) -> Result<std::collections::BTreeSet<&'a str>, Fault> {
    let mut result = std::collections::BTreeSet::new();
    for relation in &value.relations {
        let SolveDraft::RootRelation(relation) = object(state, relation.logical_id())? else {
            return Err(invalid());
        };
        let region = state.region(relation.region.logical_id())?;
        if let crate::domain_model::AmbientDomain::Product {
            coordinates: product,
        } = &region.ambient
        {
            let product = coordinates(state, product.logical_id())?;
            budget.work(size(product.coordinates.len(), AT)?, AT)?;
            for coordinate in &product.coordinates {
                if coordinate.role != CoordinateRole::Coefficient {
                    return Err(invalid());
                }
                result.insert(coordinate.key.as_str());
            }
        }
    }
    Ok(result)
}

/// Child factors and quotient maps retain matching positions and strictly smaller degrees.
fn check_split(state: &State, value: &ComponentSplitData<Handle>) -> Result<(), Fault> {
    let parent = level(state, &value.parent)?;
    let parent_degree = degree(state, &parent.polynomial)?;
    state.region(value.region.logical_id())?;
    for (factor, domain, restriction) in [
        (&value.left_factor, &value.left_domain, &value.left_map),
        (&value.right_factor, &value.right_domain, &value.right_map),
    ] {
        let child_degree = degree(state, factor)?;
        if child_degree == 0
            || child_degree >= parent_degree
            || polynomial(state, factor)?.domain != parent.parent
        {
            return Err(invalid());
        }
        let CoefficientDomainData::Quotient { level: child } = coefficient_domain(state, domain)?
        else {
            return Err(invalid());
        };
        if level(state, child)?.polynomial != *factor {
            return Err(invalid());
        }
        let SolveDraft::CoefficientMap(map) = object(state, restriction.logical_id())? else {
            return Err(invalid());
        };
        if map.mode != MapMode::Specialization
            || map.region != value.region
            || map.target
                != (MapTarget::CoefficientDomain {
                    domain: domain.clone(),
                })
        {
            return Err(invalid());
        }
        if !matches!(coefficient_domain(state, &map.source)?, CoefficientDomainData::Quotient { level } if level == &value.parent)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// The predicate and original denominators use only original coefficient and unknown keys.
fn check_problem(
    state: &State,
    value: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    coefficient_domain(state, &value.coefficient_domain)?;
    require_kind(&value.predicate, VertexKind::Condition)?;
    metadata_matches(state, &value.predicate, &value.header, &value.scope)?;
    coordinate_role(
        state,
        &value.coefficients,
        &value.header,
        CoordinateRole::Coefficient,
    )?;
    coordinate_role(
        state,
        &value.unknowns,
        &value.header,
        CoordinateRole::Unknown,
    )?;
    product_region(
        state,
        &value.region,
        &value.header,
        &value.scope,
        &value.coefficients,
    )?;
    product_region(
        state,
        &value.domain,
        &value.header,
        &value.scope,
        &value.unknowns,
    )?;
    let coefficients = coordinates(state, value.coefficients.logical_id())?;
    let unknowns = coordinates(state, value.unknowns.logical_id())?;
    let parameters = coefficients
        .coordinates
        .iter()
        .map(|coordinate| coordinate.key.as_str())
        .collect();
    free_keys(state, &value.region, &parameters, true, budget)?;
    let mut keys = std::collections::BTreeSet::new();
    for coordinate in coefficients.coordinates.iter().chain(&unknowns.coordinates) {
        if !keys.insert(coordinate.key.as_str()) {
            return Err(invalid());
        }
    }
    if unknowns
        .coordinates
        .iter()
        .any(|coordinate| coordinate.field != value.ambient)
        || state.header(value.header.logical_id())?.environment.field != value.ambient
    {
        return Err(invalid());
    }
    free_keys(state, &value.domain, &keys, true, budget)?;
    free_keys(state, &value.predicate, &keys, true, budget)?;
    for denominator in &value.denominators {
        metadata_matches(state, denominator, &value.header, &value.scope)?;
        if !matches!(
            state.expression(denominator.logical_id())?.sort,
            Sort::Value(AtomicSort::Scalar(_))
        ) {
            return Err(invalid());
        }
        free_keys(state, denominator, &keys, true, budget)?;
    }
    Ok(())
}

/// A branch preserves the exact problem, coefficient scope and unknown product.
fn check_branch(
    state: &State,
    value: &SolutionBranchData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let problem_data = problem(state, value.problem.logical_id())?;
    product_region(
        state,
        &value.region,
        &problem_data.header,
        &problem_data.scope,
        &problem_data.coefficients,
    )?;
    product_region(
        state,
        &value.domain,
        &problem_data.header,
        &problem_data.scope,
        &problem_data.unknowns,
    )?;
    let coefficients = coordinates(state, problem_data.coefficients.logical_id())?;
    let parameters = coefficients
        .coordinates
        .iter()
        .map(|coordinate| coordinate.key.as_str())
        .collect();
    free_keys(state, &value.region, &parameters, true, budget)?;
    let expected_coordinates = &problem_data.unknowns;
    match &value.kind {
        SolutionBranchKind::Finite { tuples } => {
            for tuple in tuples {
                free_keys(state, tuple, &parameters, true, budget)?;
                let SolveDraft::Tuple(tuple) = object(state, tuple.logical_id())? else {
                    return Err(invalid());
                };
                if &tuple.coordinates != expected_coordinates || tuple.scope != problem_data.scope {
                    return Err(invalid());
                }
            }
        }
        SolutionBranchKind::Image { family } => {
            let SolveDraft::FamilyMap(family) = object(state, family.logical_id())? else {
                return Err(invalid());
            };
            if &family.target != expected_coordinates
                || family.coefficients != problem_data.coefficients
                || family.scope != problem_data.scope
            {
                return Err(invalid());
            }
        }
        SolutionBranchKind::Algebraic { selection } => {
            free_keys(state, selection, &parameters, true, budget)?;
            let SolveDraft::RootSelection(selection) = object(state, selection.logical_id())?
            else {
                return Err(invalid());
            };
            if selection.header != problem_data.header || selection.scope != problem_data.scope {
                return Err(invalid());
            }
        }
        SolutionBranchKind::Local { branch } => {
            let SolveDraft::LocalBranch(branch) = object(state, branch.logical_id())? else {
                return Err(invalid());
            };
            if branch.problem != value.problem {
                return Err(invalid());
            }
        }
        SolutionBranchKind::Empty => {}
    }
    Ok(())
}

/// A set is a canonical proposal, with every member attached to the same problem.
fn check_set(state: &State, value: &SolutionSetData<Handle>) -> Result<(), Fault> {
    problem(state, value.problem.logical_id())?;
    for branch in &value.branches {
        let SolveDraft::SolutionBranch(branch) = object(state, branch.logical_id())? else {
            return Err(invalid());
        };
        if branch.problem != value.problem {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Local germs keep base coordinates and existential neighborhoods distinct from global domains.
fn check_local(
    state: &State,
    value: &LocalBranchData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let problem_data = problem(state, value.problem.logical_id())?;
    for (base, product) in [
        (&value.base_coefficients, &problem_data.coefficients),
        (&value.base_unknowns, &problem_data.unknowns),
    ] {
        let SolveDraft::Tuple(tuple) = object(state, base.logical_id())? else {
            return Err(invalid());
        };
        if tuple.coordinates != *product || tuple.scope != problem_data.scope {
            return Err(invalid());
        }
    }
    for matrix in [&value.jacobian, &value.inverse] {
        metadata_matches(state, matrix, &problem_data.header, &problem_data.scope)?;
        if !matches!(
            state.expression(matrix.logical_id())?.sort,
            Sort::Value(AtomicSort::Matrix { .. })
        ) {
            return Err(invalid());
        }
    }
    match &value.neighborhood {
        Neighborhood::Existential {
            coefficient_binder,
            unknown_binder,
        } => {
            crate::validation::scope_limit(
                &vec![coefficient_binder.clone(), unknown_binder.clone()],
                budget,
            )?;
        }
        Neighborhood::Explicit {
            coefficient_region,
            unknown_region,
        } => {
            product_region(
                state,
                coefficient_region,
                &problem_data.header,
                &problem_data.scope,
                &problem_data.coefficients,
            )?;
            product_region(
                state,
                unknown_region,
                &problem_data.header,
                &problem_data.scope,
                &problem_data.unknowns,
            )?;
        }
    }
    if value.family_symbol.key.is_empty()
        || state
            .header(problem_data.header.logical_id())?
            .environment
            .declarations
            .iter()
            .any(|entry| entry.key == value.family_symbol.key)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Dispatch syntax checks independently of all theorem schema availability.
fn validate(state: &State, value: &SolveDraft<Handle>, budget: &mut Budget) -> Result<(), Fault> {
    match value {
        SolveDraft::CoordinateSpace(value) => check_coordinates(state, value, budget),
        SolveDraft::Tuple(value) => check_tuple(state, value, budget),
        SolveDraft::SolutionSet(value) => check_set(state, value),
        SolveDraft::FamilyMap(value) => check_family(state, value, budget),
        SolveDraft::CoefficientDomain(value) => check_domain(state, value),
        SolveDraft::Polynomial(value) => check_polynomial(state, value, budget),
        SolveDraft::ExtensionLevel(value) => check_level(state, value, budget),
        SolveDraft::CoefficientMap(value) => check_map(state, value, budget),
        SolveDraft::RootRelation(value) => check_relation(state, value, budget),
        SolveDraft::RootSelection(value) => check_selection(state, value, budget),
        SolveDraft::ComponentSplit(value) => check_split(state, value),
        SolveDraft::CoefficientValue(value) => check_value(state, value, budget),
        SolveDraft::SolutionBranch(value) => check_branch(state, value, budget),
        SolveDraft::SolveProblem(value) => check_problem(state, value, budget),
        SolveDraft::LocalBranch(value) => check_local(state, value, budget),
        SolveDraft::ParameterBase(_)
        | SolveDraft::SpaceFamily(_)
        | SolveDraft::FamilyExpression(_)
        | SolveDraft::FibrePiece(_)
        | SolveDraft::RealCell(_)
        | SolveDraft::CoefficientEvaluation(_)
        | SolveDraft::ScalarSpace(_)
        | SolveDraft::SpacePlacement(_)
        | SolveDraft::ScalarPiece(_) => super::sorts::validate(state, value, budget),
    }
}

/// Canonicalize only collections that the mathematical contract declares unordered.
fn canonicalize(value: &mut SolveDraft<Handle>, budget: &mut Budget) -> Result<(), Fault> {
    let members = match value {
        SolveDraft::SolutionSet(value) => &mut value.branches,
        SolveDraft::CoefficientMap(value) => &mut value.denominators,
        SolveDraft::SolutionBranch(SolutionBranchData {
            kind: SolutionBranchKind::Finite { tuples },
            ..
        }) => tuples,
        _ => return Ok(()),
    };
    let count = size(members.len(), AT)?;
    budget.work(
        count
            .checked_mul(count)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )?;
    members.sort_unstable_by_key(Handle::logical_id);
    members.dedup();
    Ok(())
}

impl Cas {
    /// Borrow an immutable solving object with checked store ownership and kind.
    ///
    /// # Errors
    /// Refuses foreign handles and handles outside the solving object vocabulary.
    pub fn solve_view(&self, object_handle: &Handle) -> Result<SolveView<'_>, Fault> {
        Ok(object(&self.state, self.checked(object_handle)?)?.view())
    }

    /// Resolve all references and publish within the existing admission journal.
    pub(crate) fn admit_solve(
        &self,
        state: &mut State,
        draft: &SolveDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let mut value = draft
            .resolve(&mut |reference| Ok(self.handle(self.resolve(state, reference, locals)?)))?;
        self.resolve_coordinate_spaces(state, &mut value)?;
        budget.check(BudgetKind::Edges, size(value.objects().len(), AT)?, AT)?;
        canonicalize(&mut value, budget)?;
        validate(state, &value, budget)?;
        let edges = edges(&value)?;
        self.insert(
            state,
            Vertex::Solve(Box::new(value)),
            &edges,
            pending,
            budget,
        )
    }
}

/// Add an exact symbolic dependency after a fallible allocation reservation.
fn edge(result: &mut Vec<(LogicalId, Edge)>, value: &Handle, role: Edge) -> Result<(), Fault> {
    result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    result.push((value.logical_id(), role));
    Ok(())
}

/// Preserve ordered dependency positions without gaps or duplicate positions.
fn edge_family(
    result: &mut Vec<(LogicalId, Edge)>,
    values: &[Handle],
    role: fn(u32) -> Edge,
) -> Result<(), Fault> {
    for (position, value) in values.iter().enumerate() {
        edge(result, value, role(crate::graph::position_u32(position)?))?;
    }
    Ok(())
}

/// Reconstruct every typed dependency from the admitted payload.
fn edges(value: &SolveDraft<Handle>) -> Result<Vec<(LogicalId, Edge)>, Fault> {
    let mut result = Vec::new();
    match value {
        SolveDraft::CoordinateSpace(value) => edges_coordinate_space(value, &mut result)?,
        SolveDraft::Tuple(value) => edges_tuple(value, &mut result)?,
        SolveDraft::SolutionSet(value) => edges_solution_set(value, &mut result)?,
        SolveDraft::FamilyMap(value) => edges_family_map(value, &mut result)?,
        SolveDraft::CoefficientDomain(value) => edges_coefficient_domain(value, &mut result)?,
        SolveDraft::Polynomial(value) => edges_polynomial(value, &mut result)?,
        SolveDraft::ExtensionLevel(value) => edges_extension_level(value, &mut result)?,
        SolveDraft::CoefficientMap(value) => edges_coefficient_map(value, &mut result)?,
        SolveDraft::RootRelation(value) => edges_root_relation(value, &mut result)?,
        SolveDraft::RootSelection(value) => edges_root_selection(value, &mut result)?,
        SolveDraft::ComponentSplit(value) => edges_component_split(value, &mut result)?,
        SolveDraft::CoefficientValue(value) => edges_coefficient_value(value, &mut result)?,
        SolveDraft::SolutionBranch(value) => edges_solution_branch(value, &mut result)?,
        SolveDraft::SolveProblem(value) => edges_solve_problem(value, &mut result)?,
        SolveDraft::LocalBranch(value) => edges_local_branch(value, &mut result)?,
        SolveDraft::ParameterBase(_)
        | SolveDraft::SpaceFamily(_)
        | SolveDraft::FamilyExpression(_)
        | SolveDraft::FibrePiece(_)
        | SolveDraft::RealCell(_)
        | SolveDraft::CoefficientEvaluation(_)
        | SolveDraft::ScalarSpace(_)
        | SolveDraft::SpacePlacement(_)
        | SolveDraft::ScalarPiece(_) => super::sorts::edges(value, &mut result)?,
    }
    Ok(result)
}

/// Typed dependencies of `CoordinateSpace`.
fn edges_coordinate_space(
    value: &CoordinateSpaceData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    for (position, space) in value.scalar_spaces.iter().enumerate() {
        if let Some(space) = space {
            edge(
                result,
                space,
                Edge::Scalar(crate::graph::ScalarEdge::DeclarationScalarSpace(
                    crate::graph::position_u32(position)?,
                )),
            )?;
        }
    }
    Ok(())
}

/// Typed dependencies of `Tuple`.
fn edges_tuple(
    value: &TupleData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    edge(result, &value.coordinates, Edge::Coordinates)?;
    edge_family(result, &value.entries, Edge::TupleEntry)?;
    Ok(())
}

/// Typed dependencies of `SolutionSet`.
fn edges_solution_set(
    value: &SolutionSetData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.problem, Edge::Problem)?;
    edge_family(result, &value.branches, Edge::BranchMember)?;
    Ok(())
}

/// Typed dependencies of `FamilyMap`.
fn edges_family_map(
    value: &FamilyMapData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    edge(result, &value.coefficients, Edge::CoefficientCoordinates)?;
    edge(result, &value.family, Edge::FamilyCoordinates)?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    edge(result, &value.target, Edge::Coordinates)?;
    edge(result, &value.body, Edge::FamilyBody)?;
    Ok(())
}

/// Typed dependencies of `CoefficientDomain`.
fn edges_coefficient_domain(
    value: &CoefficientDomainData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    match value {
        CoefficientDomainData::Fraction { base, coordinates } => {
            edge(result, base, Edge::CoefficientBase)?;
            edge(result, coordinates, Edge::CoefficientCoordinates)?;
        }
        CoefficientDomainData::Quotient { level } => edge(result, level, Edge::TowerLevel)?,
        CoefficientDomainData::Rational | CoefficientDomainData::Gaussian => {}
    }
    Ok(())
}

/// Typed dependencies of `Polynomial`.
fn edges_polynomial(
    value: &PolynomialData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.domain, Edge::CoefficientBase)?;
    for (index, term) in value.terms.iter().enumerate() {
        edge(
            result,
            &term.coefficient,
            Edge::CoefficientTerm(crate::graph::position_u32(index)?),
        )?;
    }
    Ok(())
}

/// Typed dependencies of `ExtensionLevel`.
fn edges_extension_level(
    value: &ExtensionLevelData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.parent, Edge::TowerParent)?;
    edge(result, &value.polynomial, Edge::DefiningPolynomial)?;
    Ok(())
}

/// Typed dependencies of `CoefficientMap`.
fn edges_coefficient_map(
    value: &CoefficientMapData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.source, Edge::MapSource)?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    edge_family(result, &value.images, Edge::GeneratorImage)?;
    edge_family(result, &value.denominators, Edge::DenominatorFactor)?;
    match &value.target {
        MapTarget::Family { family } => edge(
            result,
            family,
            Edge::Scalar(crate::graph::ScalarEdge::Family),
        )?,
        MapTarget::CoefficientDomain { domain } => edge(result, domain, Edge::MapTarget)?,
        MapTarget::ScalarAmbient { header, .. } => edge(result, header, Edge::InHeader)?,
    }
    Ok(())
}

/// Typed dependencies of `RootRelation`.
fn edges_root_relation(
    value: &RootRelationData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    edge(result, &value.polynomial, Edge::DefiningPolynomial)?;
    edge(result, &value.map, Edge::CoefficientLiteralMap)?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    Ok(())
}

/// Typed dependencies of `RootSelection`.
fn edges_root_selection(
    value: &RootSelectionData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    edge_family(result, &value.relations, Edge::RootSlot)?;
    edge(result, &value.predicate, Edge::SelectionPredicate)?;
    if let Some(cell) = &value.cell {
        edge(
            result,
            cell,
            Edge::Scalar(crate::graph::ScalarEdge::SelectionCell),
        )?;
    }
    Ok(())
}

/// Typed dependencies of `ComponentSplit`.
fn edges_component_split(
    value: &ComponentSplitData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.parent, Edge::SplitParent)?;
    edge(result, &value.left_factor, Edge::SplitFactor(0))?;
    edge(result, &value.right_factor, Edge::SplitFactor(1))?;
    edge(result, &value.left_domain, Edge::SplitChild(0))?;
    edge(result, &value.right_domain, Edge::SplitChild(1))?;
    edge(result, &value.left_map, Edge::SplitMap(0))?;
    edge(result, &value.right_map, Edge::SplitMap(1))?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    Ok(())
}

/// Typed dependencies of `CoefficientValue`.
fn edges_coefficient_value(
    value: &CoefficientValueObject<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.domain, Edge::CoefficientBase)?;
    match &value.value {
        CoefficientValueData::Fraction {
            numerator,
            denominator,
        } => {
            edge(result, numerator, Edge::FractionNumerator)?;
            edge(result, denominator, Edge::FractionDenominator)?;
        }
        CoefficientValueData::Tower { coefficients } => {
            edge_family(result, coefficients, Edge::CoefficientTerm)?;
        }
        CoefficientValueData::Rational { .. } | CoefficientValueData::Gaussian { .. } => {}
    }
    Ok(())
}

/// Typed dependencies of `SolutionBranch`.
fn edges_solution_branch(
    value: &SolutionBranchData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.problem, Edge::Problem)?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    edge(result, &value.domain, Edge::SolutionDomain)?;
    match &value.kind {
        SolutionBranchKind::Finite { tuples } => edge_family(result, tuples, Edge::TupleMember)?,
        SolutionBranchKind::Image { family } => edge(result, family, Edge::FamilyImage)?,
        SolutionBranchKind::Algebraic { selection } => edge(result, selection, Edge::SelectedBy)?,
        SolutionBranchKind::Local { branch } => edge(result, branch, Edge::LocalGerm)?,
        SolutionBranchKind::Empty => {}
    }
    Ok(())
}

/// Typed dependencies of `SolveProblem`.
fn edges_solve_problem(
    value: &SolveProblemData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.header, Edge::InHeader)?;
    edge(result, &value.predicate, Edge::OriginalPredicate)?;
    edge(result, &value.coefficients, Edge::CoefficientCoordinates)?;
    edge(result, &value.unknowns, Edge::UnknownCoordinates)?;
    edge(result, &value.region, Edge::ParameterRegion)?;
    edge(result, &value.domain, Edge::SolutionDomain)?;
    edge(result, &value.coefficient_domain, Edge::CoefficientBase)?;
    edge_family(result, &value.denominators, Edge::OriginalDenominator)?;
    Ok(())
}

/// Typed dependencies of `LocalBranch`.
fn edges_local_branch(
    value: &LocalBranchData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    edge(result, &value.problem, Edge::Problem)?;
    edge(result, &value.base_coefficients, Edge::LocalBase(0))?;
    edge(result, &value.base_unknowns, Edge::LocalBase(1))?;
    edge(result, &value.jacobian, Edge::LocalJacobian)?;
    edge(result, &value.inverse, Edge::LocalInverse)?;
    if let Neighborhood::Explicit {
        coefficient_region,
        unknown_region,
    } = &value.neighborhood
    {
        edge(result, coefficient_region, Edge::ParameterRegion)?;
        edge(result, unknown_region, Edge::SolutionDomain)?;
    }
    Ok(())
}

/// Encode structural literal dependencies without serializing a store token.
pub(crate) fn attachment_key(
    value: Option<&crate::graph::ExpressionAttachment>,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    match value {
        None => out.byte(0),
        Some(crate::graph::ExpressionAttachment::SelectedRoot(selection)) => {
            out.byte(1)?;
            crate::graph::encode_id(*selection, out)
        }
        Some(crate::graph::ExpressionAttachment::ExactCoefficient { value, map }) => {
            out.byte(2)?;
            crate::graph::encode_id(*value, out)?;
            crate::graph::encode_id(*map, out)
        }
    }
}

impl Cas {
    /// Resolve a selected slot or mapped coefficient through dedicated typed syntax.
    pub(crate) fn admit_solve_literal(
        &self,
        state: &mut State,
        draft: &crate::graph::Draft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let mut literal = match draft {
            crate::graph::Draft::RootValue {
                header,
                selection,
                slot,
            } => selected_literal(
                state,
                self.resolve(state, header, locals)?,
                self.resolve(state, selection, locals)?,
                *slot,
            )?,
            crate::graph::Draft::ExactCoefficient { header, value, map } => coefficient_literal(
                self,
                state,
                self.resolve(state, header, locals)?,
                self.resolve(state, value, locals)?,
                self.resolve(state, map, locals)?,
            )?,
            _ => return Err(invalid()),
        };
        state.header(literal.header)?;
        literal.edges.push((literal.header, Edge::InHeader));
        let sort = Sort::Value(AtomicSort::Scalar(literal.field));
        let ambient = super::sorts::ambient(state, literal.header, &sort)?;
        if let Some(ambient) = ambient {
            literal.edges.push((
                ambient,
                Edge::Scalar(crate::graph::ScalarEdge::CheckedAmbient),
            ));
        }
        let expression = crate::graph::Expression {
            supported_enclosure: false,
            enclosure_roots: false,
            enclosure_exp: crate::enclosure::AnalyticShape::default(),
            enclosure_saturation: 0,
            enclosure_logs: false,
            enclosure_log_bound: None,
            ambient,
            attachment: Some(literal.attachment),
            operator: literal.operator,
            sort,
            header: literal.header,
            layer: crate::domain_model::Layer::Pointwise,
            space: None,
            domain: None,
            arms: Vec::new(),
            scope: literal.scope,
            binding: None,
            operands: Vec::new(),
            depth: 1,
            unfolded: 1,
        };
        self.insert(
            state,
            Vertex::Expression(expression),
            &literal.edges,
            pending,
            budget,
        )
    }
}

/// Bounded resolved ingredients shared by the two attached scalar constructors.
struct ScalarLiteral {
    /// Exact immutable interpretation.
    header: LogicalId,
    /// Literal constructor and optional ordered slot.
    operator: crate::model::Operator,
    /// Scalar result field.
    field: Field,
    /// Outer binder frames.
    scope: Scope,
    /// Actual semantic attachment.
    attachment: crate::graph::ExpressionAttachment,
    /// At most two typed dependencies before the header is appended.
    edges: Vec<(LogicalId, Edge)>,
}

/// Resolve one ordered root slot without manufacturing an existence theorem.
fn selected_literal(
    state: &State,
    header: LogicalId,
    selection: LogicalId,
    slot: u32,
) -> Result<ScalarLiteral, Fault> {
    let SolveDraft::RootSelection(selected) = object(state, selection)? else {
        return Err(invalid());
    };
    if selected.header.logical_id() != header {
        return Err(invalid());
    }
    let relation = selected
        .relations
        .get(usize::try_from(slot).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let SolveDraft::RootRelation(relation) = object(state, relation.logical_id())? else {
        return Err(invalid());
    };
    Ok(ScalarLiteral {
        header,
        operator: crate::model::Operator::RootValue { slot },
        field: relation.ambient.clone(),
        scope: selected.scope.clone(),
        attachment: crate::graph::ExpressionAttachment::SelectedRoot(selection),
        edges: vec![(selection, Edge::SelectedBy)],
    })
}

/// A coefficient literal retains the exact scalar map, including its specialization region.
fn coefficient_literal(
    cas: &Cas,
    state: &State,
    header: LogicalId,
    value: LogicalId,
    map: LogicalId,
) -> Result<ScalarLiteral, Fault> {
    let SolveDraft::CoefficientMap(mapping) = object(state, map)? else {
        return Err(invalid());
    };
    let (target, field) = super::family::mapping::scalar_target(state, mapping)?;
    if target.logical_id() != header
        || coefficient(state, &cas.handle(value))?.domain != mapping.source
    {
        return Err(invalid());
    }
    Ok(ScalarLiteral {
        header,
        operator: crate::model::Operator::ExactCoefficient,
        field,
        scope: state.region(mapping.region.logical_id())?.scope.clone(),
        attachment: crate::graph::ExpressionAttachment::ExactCoefficient { value, map },
        edges: vec![
            (value, Edge::CoefficientLiteralValue),
            (map, Edge::CoefficientLiteralMap),
        ],
    })
}

/// Borrow a header and scope only from solving objects that actually have them.
pub(crate) fn metadata<'a>(
    state: &'a State,
    value: &'a SolveDraft<Handle>,
) -> Result<Option<(LogicalId, &'a Scope)>, Fault> {
    // Header-only products have no bound frames.
    static EMPTY: Scope = Vec::new();
    Ok(match value {
        SolveDraft::CoordinateSpace(value) => Some((value.header.logical_id(), &EMPTY)),
        SolveDraft::Tuple(value) => Some((value.header.logical_id(), &value.scope)),
        SolveDraft::FamilyMap(value) => Some((value.header.logical_id(), &value.scope)),
        SolveDraft::RootRelation(value) => Some((value.header.logical_id(), &value.scope)),
        SolveDraft::RootSelection(value) => Some((value.header.logical_id(), &value.scope)),
        SolveDraft::SolveProblem(value) => Some((value.header.logical_id(), &value.scope)),
        SolveDraft::SolutionSet(value) => Some(problem_metadata(state, &value.problem)?),
        SolveDraft::SolutionBranch(value) => Some(problem_metadata(state, &value.problem)?),
        SolveDraft::LocalBranch(value) => Some(problem_metadata(state, &value.problem)?),
        SolveDraft::CoefficientMap(value) => {
            let region = state.region(value.region.logical_id())?;
            Some((region.header, &region.scope))
        }
        SolveDraft::ComponentSplit(value) => {
            let region = state.region(value.region.logical_id())?;
            Some((region.header, &region.scope))
        }
        SolveDraft::CoefficientDomain(_)
        | SolveDraft::Polynomial(_)
        | SolveDraft::ExtensionLevel(_)
        | SolveDraft::CoefficientValue(_) => None,
        SolveDraft::ParameterBase(_)
        | SolveDraft::SpaceFamily(_)
        | SolveDraft::FamilyExpression(_)
        | SolveDraft::FibrePiece(_)
        | SolveDraft::RealCell(_)
        | SolveDraft::CoefficientEvaluation(_)
        | SolveDraft::ScalarSpace(_)
        | SolveDraft::SpacePlacement(_)
        | SolveDraft::ScalarPiece(_) => super::sorts::metadata(state, value)?,
    })
}

/// Branch objects inherit their original problem's exact mathematical scope.
fn problem_metadata<'a>(state: &'a State, value: &Handle) -> Result<(LogicalId, &'a Scope), Fault> {
    let value = problem(state, value.logical_id())?;
    Ok((value.header.logical_id(), &value.scope))
}

/// Requests must name exactly the predicate covered by the original occurrence receipt.
pub(crate) fn request(
    state: &State,
    spec: &SolveSpec,
    context: &crate::graph::Context,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let problem_data = problem(state, spec.problem.logical_id())?;
    if problem_data.header.logical_id() != context.header_id
        || !problem_data.scope.is_empty()
        || inputs.len() != 1
    {
        return Err(invalid());
    }
    let root = *inputs.first().ok_or_else(invalid)?;
    let occurrence = state.occurrence(root)?;
    let expression = state.expression(occurrence.expression)?;
    let Vertex::Condition(predicate) = state.node(problem_data.predicate.logical_id())? else {
        return Err(invalid());
    };
    if expression.operator != crate::model::Operator::Predicate(predicate.kind.clone()) {
        return Err(invalid());
    }
    budget.work(size(predicate.arguments.len(), AT)?, AT)?;
    let expected_arguments = &predicate.arguments;
    if &expression.operands != expected_arguments
        || expression.header != predicate.header
        || expression.scope != predicate.scope
    {
        return Err(invalid());
    }
    original_denominators(state, root, &problem_data.denominators, budget)
}

/// Validate typed solving proposition roles without accepting a theorem.
pub(crate) fn statement(
    cas: &Cas,
    value: &SolveStatement,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut encoded = Encoder::new(budget, AT);
    value.encode(&mut encoded, &mut crate::statement::handle)?;
    let bytes = size(encoded.finish().len(), AT)?;
    budget.work(bytes, AT)?;
    for reference in value.objects() {
        cas.checked(reference)?;
    }
    match value {
        SolveStatement::Member(value) => statement_member(cas, value),
        SolveStatement::Equivalent(value) => statement_equivalent(cas, value),
        SolveStatement::Coverage(value) => statement_coverage(cas, value),
        SolveStatement::FamilyMembership(value) => statement_family_membership(cas, value),
        SolveStatement::FamilyCoverage(value) => statement_family_coverage(cas, value),
        SolveStatement::Rank(value) => statement_rank(cas, value),
        SolveStatement::LinearSystem(value) => statement_linear_system(cas, value),
        SolveStatement::LocalBranch(value) => statement_local_branch(cas, value),
        SolveStatement::LocalDerivative(value) => statement_local_derivative(cas, value),
        SolveStatement::CoefficientEquality(value) => statement_coefficient_equality(cas, value),
        SolveStatement::Unit(value) => statement_unit(cas, value),
        SolveStatement::ComponentCover(value) => statement_component_cover(cas, value),
        SolveStatement::RootExistence(value) => statement_root_existence(cas, value),
        SolveStatement::SelectionUnique(value) => statement_selection_unique(cas, value),
        SolveStatement::RootOrder(value) => statement_root_order(cas, value),
        SolveStatement::Orbit(value) => statement_orbit(cas, value),
        SolveStatement::Multiplicity(value) => statement_multiplicity(cas, value),
        SolveStatement::RealValues(value) => statement_real_values(cas, value),
        SolveStatement::Isolation(value) => statement_isolation(cas, value),
        SolveStatement::Presentation(value) => statement_presentation(cas, value),
        SolveStatement::CoefficientEvaluation(_)
        | SolveStatement::RealSign(_)
        | SolveStatement::RealRootCount(_)
        | SolveStatement::RealCellExists(_) => super::sorts::statement(cas, value, budget),
    }
}

/// Validate the object categories of a `Member` proposition.
fn statement_member(cas: &Cas, value: &StatementMember) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.tuple, VertexKind::Tuple)?;
    require_kind(&value.region, VertexKind::Region)?;
    let problem_data = problem(&cas.state, value.problem.logical_id())?;
    let SolveDraft::Tuple(tuple) = object(&cas.state, value.tuple.logical_id())? else {
        return Err(invalid());
    };
    let expected_coordinates = &problem_data.unknowns;
    if &tuple.coordinates != expected_coordinates || tuple.scope != problem_data.scope {
        return Err(invalid());
    }
    product_region(
        &cas.state,
        &value.region,
        &problem_data.header,
        &problem_data.scope,
        &problem_data.coefficients,
    )?;
    Ok(())
}

/// Validate the object categories of a `Equivalent` proposition.
fn statement_equivalent(_cas: &Cas, value: &StatementEquivalent) -> Result<(), Fault> {
    require_kind(&value.source, VertexKind::SolveProblem)?;
    require_kind(&value.target, VertexKind::SolveProblem)?;
    require_kind(&value.forward, VertexKind::FamilyMap)?;
    require_kind(&value.backward, VertexKind::FamilyMap)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `Coverage` proposition.
fn statement_coverage(cas: &Cas, value: &StatementCoverage) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.set, VertexKind::SolutionSet)?;
    require_kind(&value.region, VertexKind::Region)?;
    let problem_data = problem(&cas.state, value.problem.logical_id())?;
    let SolveDraft::SolutionSet(set) = object(&cas.state, value.set.logical_id())? else {
        return Err(invalid());
    };
    if set.problem != value.problem {
        return Err(invalid());
    }
    product_region(
        &cas.state,
        &value.region,
        &problem_data.header,
        &problem_data.scope,
        &problem_data.coefficients,
    )?;
    Ok(())
}

/// Validate the object categories of a `FamilyMembership` proposition.
fn statement_family_membership(_cas: &Cas, value: &StatementFamilyMembership) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.family, VertexKind::FamilyMap)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `FamilyCoverage` proposition.
fn statement_family_coverage(cas: &Cas, value: &StatementFamilyCoverage) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.family, VertexKind::FamilyMap)?;
    require_kind(&value.recovery, VertexKind::FamilyMap)?;
    require_kind(&value.region, VertexKind::Region)?;
    let SolveDraft::FamilyMap(parameterization) = object(&cas.state, value.family.logical_id())?
    else {
        return Err(invalid());
    };
    let SolveDraft::FamilyMap(recovery) = object(&cas.state, value.recovery.logical_id())? else {
        return Err(invalid());
    };
    if (&recovery.family, &recovery.target) != (&parameterization.target, &parameterization.family)
    {
        return Err(invalid());
    }
    coordinate_role(
        &cas.state,
        &parameterization.family,
        &parameterization.header,
        CoordinateRole::Family,
    )?;
    coordinate_role(
        &cas.state,
        &parameterization.target,
        &parameterization.header,
        CoordinateRole::Unknown,
    )?;
    coordinate_role(
        &cas.state,
        &recovery.family,
        &recovery.header,
        CoordinateRole::Unknown,
    )?;
    coordinate_role(
        &cas.state,
        &recovery.target,
        &recovery.header,
        CoordinateRole::Family,
    )?;
    Ok(())
}

/// Validate the object categories of a `Rank` proposition.
fn statement_rank(cas: &Cas, value: &StatementRank) -> Result<(), Fault> {
    require_kind(&value.matrix, VertexKind::Expression)?;
    require_kind(&value.domain, VertexKind::CoefficientDomain)?;
    require_kind(&value.minor, VertexKind::Expression)?;
    require_kind(&value.elimination, VertexKind::Expression)?;
    if !matches!(
        cas.state.expression(value.matrix.logical_id())?.sort,
        Sort::Value(AtomicSort::Matrix { .. })
    ) {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the object categories of a `LinearSystem` proposition.
fn statement_linear_system(cas: &Cas, value: &StatementLinearSystem) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.reduced, VertexKind::Expression)?;
    require_kind(&value.region, VertexKind::Region)?;
    if !matches!(
        cas.state.expression(value.reduced.logical_id())?.sort,
        Sort::Value(AtomicSort::Matrix { .. })
    ) {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the object categories of a `LocalBranch` proposition.
fn statement_local_branch(_cas: &Cas, value: &StatementLocalBranch) -> Result<(), Fault> {
    require_kind(&value.branch, VertexKind::LocalBranch)?;
    Ok(())
}

/// Validate the object categories of a `LocalDerivative` proposition.
fn statement_local_derivative(_cas: &Cas, value: &StatementLocalDerivative) -> Result<(), Fault> {
    require_kind(&value.branch, VertexKind::LocalBranch)?;
    require_kind(&value.derivative, VertexKind::Expression)?;
    Ok(())
}

/// Validate the object categories of a `CoefficientEquality` proposition.
fn statement_coefficient_equality(
    _cas: &Cas,
    value: &StatementCoefficientEquality,
) -> Result<(), Fault> {
    require_kind(&value.map, VertexKind::CoefficientMap)?;
    require_kind(&value.region, VertexKind::Region)?;
    for endpoint in [&value.left, &value.right] {
        if !matches!(
            endpoint.logical_id().kind,
            VertexKind::CoefficientValue | VertexKind::Polynomial | VertexKind::Expression
        ) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Validate the object categories of a `Unit` proposition.
fn statement_unit(_cas: &Cas, value: &StatementUnit) -> Result<(), Fault> {
    require_kind(&value.value, VertexKind::CoefficientValue)?;
    require_kind(&value.inverse, VertexKind::CoefficientValue)?;
    require_kind(&value.domain, VertexKind::CoefficientDomain)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `ComponentCover` proposition.
fn statement_component_cover(_cas: &Cas, value: &StatementComponentCover) -> Result<(), Fault> {
    require_kind(&value.split, VertexKind::ComponentSplit)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `RootExistence` proposition.
fn statement_root_existence(_cas: &Cas, value: &StatementRootExistence) -> Result<(), Fault> {
    require_kind(&value.region, VertexKind::Region)?;
    if !matches!(
        value.relation.logical_id().kind,
        VertexKind::RootRelation | VertexKind::RootSelection
    ) {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the object categories of a `SelectionUnique` proposition.
fn statement_selection_unique(_cas: &Cas, value: &StatementSelectionUnique) -> Result<(), Fault> {
    require_kind(&value.selection, VertexKind::RootSelection)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `RootOrder` proposition.
fn statement_root_order(cas: &Cas, value: &StatementRootOrder) -> Result<(), Fault> {
    require_kind(&value.value, VertexKind::Expression)?;
    require_kind(&value.region, VertexKind::Region)?;
    if value.order == 0
        || !matches!(
            cas.state.expression(value.value.logical_id())?.operator,
            crate::model::Operator::RootValue { .. }
        )
    {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the object categories of a `Orbit` proposition.
fn statement_orbit(_cas: &Cas, value: &StatementOrbit) -> Result<(), Fault> {
    require_kind(&value.seed, VertexKind::Polynomial)?;
    for item in &value.members {
        require_kind(item, VertexKind::Polynomial)?;
    }
    Ok(())
}

/// Validate the object categories of a `Multiplicity` proposition.
fn statement_multiplicity(_cas: &Cas, value: &StatementMultiplicity) -> Result<(), Fault> {
    require_kind(&value.problem, VertexKind::SolveProblem)?;
    require_kind(&value.selection, VertexKind::RootSelection)?;
    require_kind(&value.region, VertexKind::Region)?;
    if value.multiplicities.contains(&0) {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the object categories of a `RealValues` proposition.
fn statement_real_values(_cas: &Cas, value: &StatementRealValues) -> Result<(), Fault> {
    for item in &value.values {
        require_kind(item, VertexKind::Expression)?;
    }
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `Isolation` proposition.
fn statement_isolation(_cas: &Cas, value: &StatementIsolation) -> Result<(), Fault> {
    require_kind(&value.relation, VertexKind::RootRelation)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Validate the object categories of a `Presentation` proposition.
fn statement_presentation(_cas: &Cas, value: &StatementPresentation) -> Result<(), Fault> {
    require_kind(&value.set, VertexKind::SolutionSet)?;
    require_kind(&value.region, VertexKind::Region)?;
    Ok(())
}

/// Check semantic collection ceilings before resolving or cloning a caller's payload.
pub(crate) fn preflight<R>(value: &SolveDraft<R>, budget: &mut Budget) -> Result<(), Fault> {
    staged_resources(value, budget)?;
    let (kind, count) = match value {
        SolveDraft::CoordinateSpace(value) => {
            let family = value
                .coordinates
                .iter()
                .filter(|coordinate| coordinate.role == CoordinateRole::Family)
                .count();
            budget.solving(BudgetKind::FamilyParameters, size(family, AT)?, AT)?;
            (BudgetKind::Edges, value.coordinates.len())
        }
        SolveDraft::Tuple(value) => (BudgetKind::MatrixEntries, value.entries.len()),
        SolveDraft::SolutionSet(value) => (BudgetKind::SolutionBranches, value.branches.len()),
        SolveDraft::Polynomial(value) => {
            budget.check(
                BudgetKind::PolynomialVariables,
                size(value.variables.len(), AT)?,
                AT,
            )?;
            for term in &value.terms {
                budget.check(
                    BudgetKind::PolynomialVariables,
                    size(term.exponents.len(), AT)?,
                    AT,
                )?;
            }
            (BudgetKind::PolynomialTerms, value.terms.len())
        }
        SolveDraft::RootSelection(value) => {
            return budget.solving(
                BudgetKind::SelectionSlots,
                size(value.relations.len(), AT)?,
                AT,
            );
        }
        SolveDraft::CoefficientMap(value) => {
            budget.solving(
                BudgetKind::DenominatorFactors,
                size(value.denominators.len(), AT)?,
                AT,
            )?;
            (BudgetKind::Edges, value.images.len())
        }
        SolveDraft::SolveProblem(value) => {
            return budget.solving(
                BudgetKind::DenominatorFactors,
                size(value.denominators.len(), AT)?,
                AT,
            );
        }
        SolveDraft::CoefficientValue(CoefficientValueObject {
            value: CoefficientValueData::Tower { coefficients },
            ..
        }) => (BudgetKind::PolynomialDegree, coefficients.len()),
        SolveDraft::SolutionBranch(SolutionBranchData {
            kind: SolutionBranchKind::Finite { tuples },
            ..
        }) => (BudgetKind::CaseArms, tuples.len()),
        _ => return Ok(()),
    };
    budget.check(kind, size(count, AT)?, AT)
}
/// Charge every attempted construction before draft encoding or reference cloning.
fn staged_resources<R>(value: &SolveDraft<R>, budget: &mut Budget) -> Result<(), Fault> {
    match value {
        SolveDraft::CoefficientMap(_) => budget.solving(BudgetKind::CoefficientMaps, 1, AT),
        SolveDraft::ComponentSplit(_) => {
            budget.solving(BudgetKind::ComponentSplits, 1, AT)?;
            budget.solving(BudgetKind::SolutionBranches, 2, AT)
        }
        SolveDraft::SolutionBranch(_) => budget.solving(BudgetKind::SolutionBranches, 1, AT),
        SolveDraft::LocalBranch(_) => budget.solving(BudgetKind::LocalBranches, 1, AT),
        _ => Ok(()),
    }
}

/// Retain all syntactic denominator occurrences, even under a later cancellation or zero factor.
fn original_denominators(
    state: &State,
    root: LogicalId,
    declared: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vec![root];
    let mut denominators = Vec::new();
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let occurrence = state.occurrence(id)?;
        let denominator = match state.node(occurrence.expression)? {
            Vertex::Expression(expression) => match expression.operator {
                crate::model::Operator::Div => expression.operands.get(1),
                crate::model::Operator::IntegerPower(..=-1) => expression.operands.first(),
                _ => None,
            },
            _ => None,
        };
        if let Some(denominator) = denominator {
            budget.solving(
                BudgetKind::DenominatorFactors,
                size(denominators.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::DenominatorFactors, AT))?,
                AT,
            )?;
            denominators
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            denominators.push(*denominator);
        }
        budget.check(
            BudgetKind::Edges,
            size(pending.len(), AT)?
                .checked_add(size(occurrence.children.len(), AT)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?,
            AT,
        )?;
        pending
            .try_reserve(occurrence.children.len())
            .map_err(|_| Fault::allocation(AT))?;
        pending.extend(occurrence.children.iter().rev().copied());
    }
    if !denominators
        .iter()
        .copied()
        .eq(declared.iter().map(Handle::logical_id))
    {
        return Err(invalid());
    }
    Ok(())
}
