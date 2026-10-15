// SPDX-License-Identifier: AGPL-3.0-only
//! Closed version-one primitive registry, without user-defined trusted callbacks.

/// A compiled checker whose mathematical meaning is fixed by semantic version one.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Primitive {
    /// One registered solving schema, available only when its owning checker is enabled.
    Solve(crate::solving::SolveSchema),
    /// Affine delta pullback with both spaces, inverse and absolute Jacobian.
    DistAffinePullback,
    /// Complete rational-root factorization with nonzero slopes at every zero.
    DistSimpleZeroPullback,
    /// Equal distribution derivatives imply a constant difference on a connected interval.
    DistPrimitiveUnique,
    /// Transport checked polynomial traces to standalone punctured limits.
    LimitCaseRestrict,
    /// Restrict checked jets with exact polynomial remainder evidence.
    SeriesCaseRestrict,
    /// Finite polynomial interval integration with checked endpoint differences.
    IntegralCaseSplit,
    /// Inclusive integer ranges with checked forward differences.
    SumCaseSplit,
    /// Checked `dist.regular` schema.
    DistRegular,
    /// Checked `dist.ae` schema.
    DistAe,
    /// Checked `dist.linear` schema.
    DistLinear,
    /// Checked `dist.derivative` schema.
    DistDerivative,
    /// Checked `dist.delta-pair` schema.
    DistDeltaPair,
    /// Checked `dist.smooth` schema.
    DistSmooth,
    /// Checked `dist.delta-multiply` schema.
    DistDeltaMultiply,
    /// Checked `dist.support-action` schema.
    DistSupportAction,
    /// Checked `dist.jump` schema.
    DistJump,
    /// Complete finite partitions and exact oriented polynomial jets.
    JetPolynomial,
    /// Transport a checked derivative through local case equality.
    CaseInterior,
    /// Compare actual point ownership and two incident finite jets.
    CaseBoundary,
    /// Reconstruct ordered source clauses and their reached regions.
    CaseOrdered,
    /// Reconstruct a compatible case's exact defined domain.
    CaseDomain,
    /// Select a local association or certify function-specific point ownership.
    CaseSelect,
    /// Check local agreement or complete case/domain congruence.
    CaseCongruence,
    /// Join mechanically restricted closed conclusions under a checked cover.
    CaseEliminate,
    /// Equality of an expression with itself on its defined domain.
    Reflexivity,
    /// Reverse one equality premise.
    Symmetry,
    /// Compose two equality premises with an identical middle term.
    Transitivity,
    /// Replace a checked operand path using one equality premise.
    Congruence,
    /// Scalar additive commutativity.
    AddCommute,
    /// Scalar multiplicative commutativity.
    MulCommute,
    /// Reassociate an ordered scalar sum to the right.
    AddAssociate,
    /// Reassociate an ordered scalar product to the right.
    MulAssociate,
    /// Distribute a left scalar factor over a sum.
    Distribute,
    /// Remove a right additive zero.
    AddZero,
    /// Remove a right multiplicative one.
    MulOne,
    /// Add an expression to its negation.
    AddInverse,
    /// Multiply by a right zero while retaining the other operand's domain.
    MulZero,
    /// Cancel equal numerator and denominator under a nonzero guard.
    Cancel,
    /// Strict integer-power recurrence.
    Power,
    /// Recompute an operation on rational literal operands.
    Exact,
    /// Equality of finite polynomial coefficient jets.
    Jets,
    /// Clear denominators and compare finite jets with all original guards.
    RationalIdentity,
    /// Derivative of a rational or named constant.
    DifferentiateConstant,
    /// Derivative of a declared real scalar variable.
    DifferentiateVariable,
    /// Derivative of a negation from its child premise.
    DifferentiateNeg,
    /// Derivative of an ordered sum from both child premises.
    DifferentiateAdd,
    /// Derivative of an ordered difference from both child premises.
    DifferentiateSub,
    /// Product rule with ordered child derivative premises.
    DifferentiateProduct,
    /// Quotient rule with ordered child derivative premises.
    DifferentiateQuotient,
    /// Strict fixed-integer power rule with a child derivative premise.
    DifferentiatePower,
    /// Transport a derivative result along an equality premise.
    DerivativeTransport,
    /// Simultaneously substitute an equality and all its domain requirements.
    Substitute,
    /// Assemble an ordered matrix from a derivative premise for every cell.
    Jacobian,
    /// Rational square-free multiplicity layers with independently checked Bezout evidence.
    SquareFree,
    /// Use one exact qualified hypothesis in an ancestor or current frame.
    ContextHypothesis,
    /// Transport a theorem through an explicit restriction or semantic embedding.
    ContextWeaken,
    /// Close a local frame while retaining every used predicate as evidence or residual.
    ContextClose,
    /// Recompute a finite affine or integer truth partition.
    RegionDecide,
    /// Check the exact defining membership and inclusion laws of region constructors.
    RegionConstruct,
}

/// Typed conclusion family selected by an executable checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConclusionKind {
    /// Native typed solution, coefficient, root, family or local fact.
    Solve,
    /// Finite limit or polynomial series on an explicit approach.
    Germ,
    /// Finite split integral or sum.
    Split,
    /// Native distribution membership, equality, derivative or scalar action.
    Distribution,
    /// Exact partition, jet, trace, jump or point derivative fact.
    Boundary,
    /// Typed case construction, domain, agreement, ownership or elimination.
    Case,
    /// Scalar equality on an explicit domain.
    Equality,
    /// Scalar derivative on an explicit domain.
    Derivative,
    /// Ordered Jacobian cells.
    Jacobian,
    /// Rational multiplicity decomposition.
    SquareFree,
    /// Exact introduced predicate or membership fact.
    Hypothesis,
    /// The same typed statement under checked scope transport.
    Transport,
    /// A constructor or independently decided set relation.
    Region,
}

/// Premise shape is explicit independently of certificate representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PremiseShape {
    /// Ordered solving evidence consumed by the exact certificate subtype and mode.
    SolveEvidence,
    /// Ordered analytic and distribution facts consumed by the closed schema.
    DistributionEvidence,
    /// Exactly the ordered premises consumed by the closed boundary mode.
    BoundaryEvidence,
    /// Typed case premises consumed according to the closed certificate mode.
    CaseEvidence,
    /// Exactly this many ordered earlier applications.
    Exact(u32),
    /// A fixed prefix followed by explicitly consumed discharge proofs.
    AtLeast(u32),
    /// One ordered derivative premise per output/coordinate cell.
    JacobianCells,
}

/// The sole auxiliary schema admitted by one primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateSchema {
    /// One exact typed solving certificate subtype.
    Solve,
    /// Finite factorization, zero and inverse-Jacobian evidence.
    Pullback,
    /// Exact polynomial side remainders.
    Germ,
    /// Finite cover, point-extension and endpoint witnesses.
    Split,
    /// Closed regularity, exceptional-set and support modes.
    Distribution,
    /// Closed finite boundary-calculus modes.
    Boundary,
    /// Complete typed association, restriction and exact-cell evidence.
    Cases,
    /// No auxiliary payload.
    None,
    /// Ordered operand path.
    Congruence,
    /// Exact polynomial GCD and Bezout evidence.
    SquareFree,
    /// Qualified introducing frame and slot.
    Hypothesis,
    /// Exact local frame being closed.
    Close,
    /// Exact rational cuts and Boolean cell rows.
    Region,
}

/// Immutable public metadata for one compiled schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleSpec {
    /// Closed checker identity.
    pub primitive: Primitive,
    /// Stable ASCII rule key.
    pub key: &'static str,
    /// Exact semantic version; no implicit upgrades occur during replay.
    pub version: u32,
    /// Positive transition cost in the current logical policy.
    pub weight: u64,
    /// Typed output family; no dummy scalar endpoints stand for set facts.
    pub conclusion: ConclusionKind,
    /// Ordered premise contract checked by the kernel.
    pub premises: PremiseShape,
    /// Exact auxiliary-data schema; an absent producer cannot register a placeholder.
    pub certificate: CertificateSchema,
    /// Whether this build has an executable independent checker for this registered schema.
    pub available: bool,
}

impl Primitive {
    /// Whether this rule requires native typed checking rather than scalar projection.
    pub(crate) const fn native(self) -> bool {
        matches!(
            self,
            Self::Solve(_)
                | Self::DistAffinePullback
                | Self::DistSimpleZeroPullback
                | Self::DistPrimitiveUnique
                | Self::LimitCaseRestrict
                | Self::SeriesCaseRestrict
                | Self::IntegralCaseSplit
                | Self::SumCaseSplit
                | Self::DistRegular
                | Self::DistAe
                | Self::DistLinear
                | Self::DistDerivative
                | Self::DistDeltaPair
                | Self::DistSmooth
                | Self::DistDeltaMultiply
                | Self::DistSupportAction
                | Self::DistJump
                | Self::JetPolynomial
                | Self::CaseInterior
                | Self::CaseBoundary
                | Self::ContextHypothesis
                | Self::ContextWeaken
                | Self::ContextClose
                | Self::RegionDecide
                | Self::RegionConstruct
                | Self::CaseOrdered
                | Self::CaseDomain
                | Self::CaseSelect
                | Self::CaseCongruence
                | Self::CaseEliminate
        )
    }

    /// Read the pinned version-one schema metadata.
    #[must_use]
    pub const fn specification(self) -> RuleSpec {
        let (key, weight) = match self {
            Self::Solve(schema) => return schema.specification(),
            Self::DistAffinePullback => ("dist.affine-pullback", 12),
            Self::DistSimpleZeroPullback => ("dist.simple-zero-pullback", 12),
            Self::DistPrimitiveUnique => ("dist.primitive-unique", 8),
            Self::LimitCaseRestrict => ("limit.case-restrict", 2),
            Self::SeriesCaseRestrict => ("series.case-restrict", 4),
            Self::IntegralCaseSplit => ("integral.case-split", 8),
            Self::SumCaseSplit => ("sum.case-split", 4),
            Self::DistRegular => ("dist.regular", 8),
            Self::DistAe => ("dist.ae", 4),
            Self::DistLinear => ("dist.linear", 2),
            Self::DistDerivative => ("dist.derivative", 2),
            Self::DistDeltaPair => ("dist.delta-pair", 4),
            Self::DistSmooth => ("dist.smooth", 8),
            Self::DistDeltaMultiply => ("dist.delta-multiply", 8),
            Self::DistSupportAction => ("dist.support-action", 8),
            Self::DistJump => ("dist.jump", 8),
            Self::JetPolynomial => ("jet.polynomial", 4),
            Self::CaseInterior => ("diff.case-interior", 2),
            Self::CaseBoundary => ("diff.case-boundary", 4),
            Self::CaseOrdered => ("case.ordered", 4),
            Self::CaseDomain => ("case.domain", 2),
            Self::CaseSelect => ("case.select", 2),
            Self::CaseCongruence => ("case.congruence", 4),
            Self::CaseEliminate => ("case.eliminate", 4),
            Self::Reflexivity => ("eq.refl", 1),
            Self::Symmetry => ("eq.sym", 1),
            Self::Transitivity => ("eq.trans", 1),
            Self::Congruence => ("eq.congruence", 1),
            Self::AddCommute => ("eq.add-commute", 2),
            Self::MulCommute => ("eq.mul-commute", 2),
            Self::AddAssociate => ("eq.add-associate", 2),
            Self::MulAssociate => ("eq.mul-associate", 2),
            Self::Distribute => ("eq.distribute", 2),
            Self::AddZero => ("eq.add-zero", 2),
            Self::MulOne => ("eq.mul-one", 2),
            Self::AddInverse => ("eq.add-inverse", 2),
            Self::MulZero => ("eq.mul-zero", 2),
            Self::Cancel => ("eq.cancel", 2),
            Self::Power => ("eq.integer-power", 2),
            Self::Exact => ("exact.rational", 1),
            Self::Jets => ("poly.jets-equal", 4),
            Self::RationalIdentity => ("poly.rational-equal", 4),
            Self::DifferentiateConstant => ("diff.constant", 2),
            Self::DifferentiateVariable => ("diff.variable", 2),
            Self::DifferentiateNeg => ("diff.neg", 2),
            Self::DifferentiateAdd => ("diff.add", 2),
            Self::DifferentiateSub => ("diff.sub", 2),
            Self::DifferentiateProduct => ("diff.product", 2),
            Self::DifferentiateQuotient => ("diff.quotient", 2),
            Self::DifferentiatePower => ("diff.integer-power", 2),
            Self::DerivativeTransport => ("diff.transport", 1),
            Self::Substitute => ("eq.substitute", 1),
            Self::Jacobian => ("diff.jacobian", 2),
            Self::SquareFree => ("poly.square-free", 8),
            Self::ContextHypothesis => ("ctx.hypothesis", 1),
            Self::ContextWeaken => ("ctx.weaken", 1),
            Self::ContextClose => ("ctx.close", 2),
            Self::RegionDecide => ("region.decide", 4),
            Self::RegionConstruct => ("region.construct", 2),
        };
        RuleSpec {
            primitive: self,
            key,
            version: 1,
            weight,
            conclusion: self.conclusion_kind(),
            premises: self.premise_shape(),
            certificate: self.certificate_schema(),
            available: true,
        }
    }

    /// Identify the exact proposition family without inspecting untrusted payloads.
    const fn conclusion_kind(self) -> ConclusionKind {
        match self {
            Self::Solve(_) => ConclusionKind::Solve,
            Self::LimitCaseRestrict | Self::SeriesCaseRestrict => ConclusionKind::Germ,
            Self::IntegralCaseSplit | Self::SumCaseSplit => ConclusionKind::Split,
            Self::DistAffinePullback
            | Self::DistSimpleZeroPullback
            | Self::DistPrimitiveUnique
            | Self::DistRegular
            | Self::DistAe
            | Self::DistLinear
            | Self::DistDerivative
            | Self::DistDeltaPair
            | Self::DistSmooth
            | Self::DistDeltaMultiply
            | Self::DistSupportAction
            | Self::DistJump => ConclusionKind::Distribution,
            Self::JetPolynomial | Self::CaseBoundary => ConclusionKind::Boundary,
            Self::CaseOrdered
            | Self::CaseDomain
            | Self::CaseSelect
            | Self::CaseCongruence
            | Self::CaseEliminate => ConclusionKind::Case,
            Self::ContextHypothesis => ConclusionKind::Hypothesis,
            Self::ContextWeaken | Self::ContextClose => ConclusionKind::Transport,
            Self::RegionDecide | Self::RegionConstruct => ConclusionKind::Region,
            Self::Jacobian => ConclusionKind::Jacobian,
            Self::SquareFree => ConclusionKind::SquareFree,
            Self::DifferentiateConstant
            | Self::DifferentiateVariable
            | Self::DifferentiateNeg
            | Self::DifferentiateAdd
            | Self::DifferentiateSub
            | Self::DifferentiateProduct
            | Self::DifferentiateQuotient
            | Self::DifferentiatePower
            | Self::DerivativeTransport
            | Self::CaseInterior => ConclusionKind::Derivative,
            _ => ConclusionKind::Equality,
        }
    }

    /// Fixed and dependent ordered-premise arities remain visible in the registry.
    const fn premise_shape(self) -> PremiseShape {
        match self {
            Self::Solve(_) => PremiseShape::SolveEvidence,
            Self::DistPrimitiveUnique
            | Self::DistRegular
            | Self::DistAe
            | Self::DistLinear
            | Self::DistDerivative
            | Self::DistDeltaPair
            | Self::DistSmooth
            | Self::DistDeltaMultiply
            | Self::DistSupportAction
            | Self::DistJump => PremiseShape::DistributionEvidence,
            Self::LimitCaseRestrict
            | Self::SeriesCaseRestrict
            | Self::JetPolynomial
            | Self::CaseInterior
            | Self::CaseBoundary => PremiseShape::BoundaryEvidence,
            Self::CaseOrdered
            | Self::CaseDomain
            | Self::CaseSelect
            | Self::CaseCongruence
            | Self::CaseEliminate => PremiseShape::CaseEvidence,
            Self::ContextClose => PremiseShape::AtLeast(2),
            Self::Jacobian => PremiseShape::JacobianCells,
            Self::Transitivity
            | Self::ContextWeaken
            | Self::DifferentiateAdd
            | Self::DifferentiateSub
            | Self::DifferentiateProduct
            | Self::DifferentiateQuotient
            | Self::DerivativeTransport => PremiseShape::Exact(2),
            Self::Symmetry
            | Self::Congruence
            | Self::Substitute
            | Self::DifferentiateNeg
            | Self::DifferentiatePower => PremiseShape::Exact(1),
            _ => PremiseShape::Exact(0),
        }
    }

    /// Certificate meaning is independent of a statement's wire representation.
    const fn certificate_schema(self) -> CertificateSchema {
        match self {
            Self::Solve(_) => CertificateSchema::Solve,
            Self::DistAffinePullback | Self::DistSimpleZeroPullback => CertificateSchema::Pullback,
            Self::SeriesCaseRestrict => CertificateSchema::Germ,
            Self::IntegralCaseSplit | Self::SumCaseSplit => CertificateSchema::Split,
            Self::DistRegular
            | Self::DistAe
            | Self::DistLinear
            | Self::DistDerivative
            | Self::DistDeltaPair
            | Self::DistSmooth
            | Self::DistDeltaMultiply
            | Self::DistSupportAction
            | Self::DistJump => CertificateSchema::Distribution,
            Self::JetPolynomial | Self::CaseInterior | Self::CaseBoundary => {
                CertificateSchema::Boundary
            }
            Self::CaseOrdered
            | Self::CaseDomain
            | Self::CaseSelect
            | Self::CaseCongruence
            | Self::CaseEliminate => CertificateSchema::Cases,
            Self::Congruence => CertificateSchema::Congruence,
            Self::SquareFree => CertificateSchema::SquareFree,
            Self::ContextHypothesis => CertificateSchema::Hypothesis,
            Self::ContextClose => CertificateSchema::Close,
            Self::RegionDecide => CertificateSchema::Region,
            _ => CertificateSchema::None,
        }
    }
}

/// Every registered primitive, in ASCII key order; availability is separate metadata.
pub const PRIMITIVES: [Primitive; 77] = [
    Primitive::CaseCongruence,
    Primitive::CaseDomain,
    Primitive::CaseEliminate,
    Primitive::CaseOrdered,
    Primitive::CaseSelect,
    Primitive::Solve(crate::solving::SolveSchema::CoefficientIdentity),
    Primitive::Solve(crate::solving::SolveSchema::ConjugationTransport),
    Primitive::ContextClose,
    Primitive::ContextHypothesis,
    Primitive::ContextWeaken,
    Primitive::DifferentiateAdd,
    Primitive::CaseBoundary,
    Primitive::CaseInterior,
    Primitive::DifferentiateConstant,
    Primitive::DifferentiatePower,
    Primitive::Jacobian,
    Primitive::DifferentiateNeg,
    Primitive::DifferentiateProduct,
    Primitive::DifferentiateQuotient,
    Primitive::DifferentiateSub,
    Primitive::DerivativeTransport,
    Primitive::DifferentiateVariable,
    Primitive::DistAe,
    Primitive::DistAffinePullback,
    Primitive::DistDeltaMultiply,
    Primitive::DistDeltaPair,
    Primitive::DistDerivative,
    Primitive::DistJump,
    Primitive::DistLinear,
    Primitive::DistPrimitiveUnique,
    Primitive::DistRegular,
    Primitive::DistSimpleZeroPullback,
    Primitive::DistSmooth,
    Primitive::DistSupportAction,
    Primitive::AddAssociate,
    Primitive::AddCommute,
    Primitive::AddInverse,
    Primitive::AddZero,
    Primitive::Cancel,
    Primitive::Congruence,
    Primitive::Distribute,
    Primitive::Power,
    Primitive::MulAssociate,
    Primitive::MulCommute,
    Primitive::MulOne,
    Primitive::MulZero,
    Primitive::Reflexivity,
    Primitive::Substitute,
    Primitive::Symmetry,
    Primitive::Transitivity,
    Primitive::Exact,
    Primitive::Solve(crate::solving::SolveSchema::RootAdjunction),
    Primitive::Solve(crate::solving::SolveSchema::ComponentSplit),
    Primitive::Solve(crate::solving::SolveSchema::ExtensionUnit),
    Primitive::IntegralCaseSplit,
    Primitive::JetPolynomial,
    Primitive::LimitCaseRestrict,
    Primitive::Solve(crate::solving::SolveSchema::ExactLinearEquivalence),
    Primitive::Solve(crate::solving::SolveSchema::PermutationOrbitCoverage),
    Primitive::Jets,
    Primitive::RationalIdentity,
    Primitive::SquareFree,
    Primitive::RegionConstruct,
    Primitive::RegionDecide,
    Primitive::Solve(crate::solving::SolveSchema::BinomialCoverage),
    Primitive::Solve(crate::solving::SolveSchema::RealIsolationAndOrder),
    Primitive::Solve(crate::solving::SolveSchema::RootSelectionRefinement),
    Primitive::SeriesCaseRestrict,
    Primitive::Solve(crate::solving::SolveSchema::ParameterImageCoverage),
    Primitive::Solve(crate::solving::SolveSchema::ImplicitFunctionBranch),
    Primitive::Solve(crate::solving::SolveSchema::Membership),
    Primitive::Solve(crate::solving::SolveSchema::MultiplicityTransfer),
    Primitive::Solve(crate::solving::SolveSchema::ProductCompleteness),
    Primitive::Solve(crate::solving::SolveSchema::RealRestriction),
    Primitive::Solve(crate::solving::SolveSchema::TotalPredicateSplit),
    Primitive::Solve(crate::solving::SolveSchema::SolutionTransport),
    Primitive::SumCaseSplit,
];
