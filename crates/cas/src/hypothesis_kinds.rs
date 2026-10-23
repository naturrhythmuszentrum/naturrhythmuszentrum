// SPDX-License-Identifier: AGPL-3.0-only
//! Formal kinds classify retained evidence without asserting that an assumed fact is proved.

use crate::domain_model::HypothesisRef;
use crate::emptiness::LemmaSchema;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::PredicateKind;
use crate::rule::PremiseShape;

#[path = "hypothesis_kinds/classify.rs"]
mod classify;

/// Formal mathematical kind of a predicate, statement or ordered premise contract.
///
/// A kind is an assumed shape; a checked proof of an instance remains evidence for that instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HypothesisKind {
    /// The original operands are defined on the stated region.
    DomainGuard,
    /// The exact scalar subject is defined and nonzero.
    NonZero,
    /// The exact Real subject is strictly positive or strictly negative.
    StrictSign,
    /// The exact Real subject is nonnegative.
    NonNegative,
    /// The exact integer belongs to the stated half-open interval.
    IntegerRange,
    /// The original function has the stated differentiability.
    Differentiable,
    /// The two exact expressions agree under the stated definedness contract.
    Equality,
    /// The exact subject equals zero.
    DeclaredZero,
    /// The original matrix is symmetric.
    Symmetric,
    /// Every retained conjunct holds.
    Conjunction,
    /// At least one retained disjunct holds.
    Disjunction,
    /// The retained predicate is false.
    Negation,
    /// The retained predicate holds for all values of its declared binders.
    Universal,
    /// The explicitly named mathematical predicate is assumed true.
    Named,
    /// The exact Boolean expression is defined and true.
    Boolean,
    /// Every point of the stated source region belongs to the exact containing region.
    RegionMembership,
    /// The stated region has no assignments under the retained conditions.
    EmptyRegion,
    /// Every point of the exact domain belongs to a retained cover member.
    Cover,
    /// The two exact regions have empty intersection.
    Disjoint,
    /// The retained region satisfies its exact defining membership law.
    Constructor,
    /// The original function is defined on exactly the stated domain.
    DefinedDomain,
    /// The two original case associations agree in value and definedness on their overlap.
    Compatibility,
    /// The ordered source form elaborates to the stated native expression.
    Elaboration,
    /// The stated side of a boundary has the exact incident neighborhood.
    Incidence,
    /// The exact function owns the stated point value or is absent there.
    Ownership,
    /// The stated expression is the derivative of the original function on the exact domain.
    Derivative,
    /// The ordered matrix contains the stated partial derivatives.
    Jacobian,
    /// The displayed layers reconstruct the polynomial with their exact multiplicities.
    SquareFree,
    /// The original function has the stated oriented finite jet.
    Jet,
    /// The stated value is the right trace minus the left trace at the exact boundary.
    Jump,
    /// The retained pieces form the stated complete finite boundary partition.
    BoundaryPartition,
    /// The case has the stated positive-order classical derivative.
    CaseDerivative,
    /// The function has the stated classical derivative at its owned point.
    DerivativeAt,
    /// The stated finite obstruction excludes the point derivative.
    DerivativeNonexistence,
    /// The original function has the stated oriented trace.
    Trace,
    /// The retained typed solution, coefficient, root, family or Real fact holds.
    SolveEvidence,
    /// The retained distribution or analytic fact holds in its exact declared space.
    DistributionEvidence,
    /// The retained boundary-calculus facts hold with their exact oriented roles.
    BoundaryEvidence,
    /// The retained case facts hold on their exact regions.
    CaseEvidence,
    /// Every member of the exact ordered premise sequence holds.
    ExactPremises,
    /// The fixed premise prefix and every explicitly consumed discharge fact hold.
    DischargePremises,
    /// Every ordered output and coordinate cell has its required derivative fact.
    JacobianCells,
    /// The exact Real subject is nonpositive.
    NonPositive,
    /// The original function has the stated finite limit or series on its exact approach.
    GermEvidence,
    /// The original function has the stated finite integral or sum.
    SplitEvidence,
}

/// Every formal hypothesis kind in register order.
pub const HYPOTHESIS_KINDS: [HypothesisKind; 45] = [
    HypothesisKind::DomainGuard,
    HypothesisKind::NonZero,
    HypothesisKind::StrictSign,
    HypothesisKind::NonNegative,
    HypothesisKind::IntegerRange,
    HypothesisKind::Differentiable,
    HypothesisKind::Equality,
    HypothesisKind::DeclaredZero,
    HypothesisKind::Symmetric,
    HypothesisKind::Conjunction,
    HypothesisKind::Disjunction,
    HypothesisKind::Negation,
    HypothesisKind::Universal,
    HypothesisKind::Named,
    HypothesisKind::Boolean,
    HypothesisKind::RegionMembership,
    HypothesisKind::EmptyRegion,
    HypothesisKind::Cover,
    HypothesisKind::Disjoint,
    HypothesisKind::Constructor,
    HypothesisKind::DefinedDomain,
    HypothesisKind::Compatibility,
    HypothesisKind::Elaboration,
    HypothesisKind::Incidence,
    HypothesisKind::Ownership,
    HypothesisKind::Derivative,
    HypothesisKind::Jacobian,
    HypothesisKind::SquareFree,
    HypothesisKind::Jet,
    HypothesisKind::Jump,
    HypothesisKind::BoundaryPartition,
    HypothesisKind::CaseDerivative,
    HypothesisKind::DerivativeAt,
    HypothesisKind::DerivativeNonexistence,
    HypothesisKind::Trace,
    HypothesisKind::SolveEvidence,
    HypothesisKind::DistributionEvidence,
    HypothesisKind::BoundaryEvidence,
    HypothesisKind::CaseEvidence,
    HypothesisKind::ExactPremises,
    HypothesisKind::DischargePremises,
    HypothesisKind::JacobianCells,
    HypothesisKind::NonPositive,
    HypothesisKind::GermEvidence,
    HypothesisKind::SplitEvidence,
];

/// The named assumed shape of every supported hypothesis kind.
pub static HYPOTHESIS_SCHEMAS: [LemmaSchema; 45] = [
    LemmaSchema {
        name: "hyp.domain-guard",
        label: "assum:cas:hyp-domain-guard",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original operands are defined on the stated region.",
        premises: "Exact original operands, context, scope and region.",
    },
    LemmaSchema {
        name: "hyp.nonzero",
        label: "assum:cas:hyp-nonzero",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact scalar subject is defined and nonzero.",
        premises: "Original subject and qualified containing region.",
    },
    LemmaSchema {
        name: "hyp.strict-sign",
        label: "assum:cas:hyp-strict-sign",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact Real subject is strictly positive or strictly negative.",
        premises: "Original Real subject, stated sign and qualified containing region.",
    },
    LemmaSchema {
        name: "hyp.nonnegative",
        label: "assum:cas:hyp-nonnegative",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact Real subject is nonnegative.",
        premises: "Original Real subject and qualified containing region.",
    },
    LemmaSchema {
        name: "hyp.integer-range",
        label: "assum:cas:hyp-integer-range",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact integer belongs to the stated half-open interval.",
        premises: "Original integer expression and exact interval bounds.",
    },
    LemmaSchema {
        name: "hyp.differentiable",
        label: "assum:cas:hyp-differentiable",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function has the stated differentiability.",
        premises: "Exact function, coordinate, order and region.",
    },
    LemmaSchema {
        name: "hyp.equality",
        label: "assum:cas:hyp-equality",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The two exact expressions agree under the stated definedness contract.",
        premises: "Original endpoints, context and domain with their definedness requirements.",
    },
    LemmaSchema {
        name: "hyp.declared-zero",
        label: "assum:cas:hyp-declared-zero",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact subject equals zero.",
        premises: "Original equality operands with an exact rational zero, or a scalar zero-sign conclusion, on the qualified region.",
    },
    LemmaSchema {
        name: "hyp.symmetric",
        label: "assum:cas:hyp-symmetric",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original matrix is symmetric.",
        premises: "Exact ordered matrix expression and declaration snapshot.",
    },
    LemmaSchema {
        name: "hyp.conjunction",
        label: "assum:cas:hyp-conjunction",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "Every retained conjunct holds.",
        premises: "The complete original ordered predicate operands.",
    },
    LemmaSchema {
        name: "hyp.disjunction",
        label: "assum:cas:hyp-disjunction",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "At least one retained disjunct holds.",
        premises: "The complete original ordered predicate operands.",
    },
    LemmaSchema {
        name: "hyp.negation",
        label: "assum:cas:hyp-negation",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained predicate is false.",
        premises: "The exact original predicate and its qualified region.",
    },
    LemmaSchema {
        name: "hyp.universal",
        label: "assum:cas:hyp-universal",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained predicate holds for all values of its declared binders.",
        premises: "Exact binder sorts, body and outer scope.",
    },
    LemmaSchema {
        name: "hyp.named",
        label: "assum:cas:hyp-named",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The explicitly named mathematical predicate is assumed true.",
        premises: "Exact predicate name, ordered arguments and introducing frame.",
    },
    LemmaSchema {
        name: "hyp.boolean",
        label: "assum:cas:hyp-boolean",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact Boolean expression is defined and true.",
        premises: "Original Boolean expression, declaration snapshot and region.",
    },
    LemmaSchema {
        name: "hyp.region-membership",
        label: "assum:cas:hyp-region-membership",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "Every point of the stated source region belongs to the exact containing region.",
        premises: "Exact source and containing regions, including the local membership binder when used.",
    },
    LemmaSchema {
        name: "hyp.empty-region",
        label: "assum:cas:hyp-empty-region",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The stated region has no assignments under the retained conditions.",
        premises: "Exact region and complete qualified conditions.",
    },
    LemmaSchema {
        name: "hyp.cover",
        label: "assum:cas:hyp-cover",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "Every point of the exact domain belongs to a retained cover member.",
        premises: "Complete exact domain and unordered member inventory.",
    },
    LemmaSchema {
        name: "hyp.disjoint",
        label: "assum:cas:hyp-disjoint",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The two exact regions have empty intersection.",
        premises: "Both original regions under the same qualified interpretation.",
    },
    LemmaSchema {
        name: "hyp.constructor",
        label: "assum:cas:hyp-constructor",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained region satisfies its exact defining membership law.",
        premises: "Original region constructor and stated law.",
    },
    LemmaSchema {
        name: "hyp.defined-domain",
        label: "assum:cas:hyp-defined-domain",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function is defined on exactly the stated domain.",
        premises: "Original function and exact region, including every operand requirement.",
    },
    LemmaSchema {
        name: "hyp.compatibility",
        label: "assum:cas:hyp-compatibility",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The two original case associations agree in value and definedness on their overlap.",
        premises: "Exact associations and stated overlap region.",
    },
    LemmaSchema {
        name: "hyp.elaboration",
        label: "assum:cas:hyp-elaboration",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The ordered source form elaborates to the stated native expression.",
        premises: "Complete source form, result and exact defined domain.",
    },
    LemmaSchema {
        name: "hyp.incidence",
        label: "assum:cas:hyp-incidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The stated side of a boundary has the exact incident neighborhood.",
        premises: "Original region, boundary, oriented side and neighborhood.",
    },
    LemmaSchema {
        name: "hyp.ownership",
        label: "assum:cas:hyp-ownership",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact function owns the stated point value or is absent there.",
        premises: "Original function, boundary and explicit presence or absence claim.",
    },
    LemmaSchema {
        name: "hyp.derivative",
        label: "assum:cas:hyp-derivative",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The stated expression is the derivative of the original function on the exact domain.",
        premises: "Original function, derivative, declared coordinate and domain.",
    },
    LemmaSchema {
        name: "hyp.jacobian",
        label: "assum:cas:hyp-jacobian",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The ordered matrix contains the stated partial derivatives.",
        premises: "Original output column, matrix, ordered distinct coordinates and domain.",
    },
    LemmaSchema {
        name: "hyp.square-free",
        label: "assum:cas:hyp-square-free",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The displayed layers reconstruct the polynomial with their exact multiplicities.",
        premises: "Original polynomial, factors, declared variable, coefficient field and domain.",
    },
    LemmaSchema {
        name: "hyp.jet",
        label: "assum:cas:hyp-jet",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function has the stated oriented finite jet.",
        premises: "Exact region, boundary, side and ordered derivative values.",
    },
    LemmaSchema {
        name: "hyp.jump",
        label: "assum:cas:hyp-jump",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The stated value is the right trace minus the left trace at the exact boundary.",
        premises: "Original function, boundary, derivative order and both actual traces.",
    },
    LemmaSchema {
        name: "hyp.boundary-partition",
        label: "assum:cas:hyp-boundary-partition",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained pieces form the stated complete finite boundary partition.",
        premises: "Complete selector pieces, point ownership and exact boundary inventory.",
    },
    LemmaSchema {
        name: "hyp.case-derivative",
        label: "assum:cas:hyp-case-derivative",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The case has the stated positive-order classical derivative.",
        premises: "Original case, coordinate, order, domain and boundary evidence.",
    },
    LemmaSchema {
        name: "hyp.derivative-at",
        label: "assum:cas:hyp-derivative-at",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The function has the stated classical derivative at its owned point.",
        premises: "Original function, point ownership and incident finite jets.",
    },
    LemmaSchema {
        name: "hyp.derivative-nonexistence",
        label: "assum:cas:hyp-derivative-nonexistence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The stated finite obstruction excludes the point derivative.",
        premises: "Original function, owned point and exact nonexistence evidence.",
    },
    LemmaSchema {
        name: "hyp.trace",
        label: "assum:cas:hyp-trace",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function has the stated oriented trace.",
        premises: "Exact function, boundary, side, derivative order and value.",
    },
    LemmaSchema {
        name: "hyp.solve-evidence",
        label: "assum:cas:hyp-solve-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained typed solution, coefficient, root, family or Real fact holds.",
        premises: "The complete solving statement and ordered evidence selected by its certificate subtype and mode.",
    },
    LemmaSchema {
        name: "hyp.distribution-evidence",
        label: "assum:cas:hyp-distribution-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained distribution or analytic fact holds in its exact declared space.",
        premises: "The complete typed statement and ordered evidence selected by its closed mode.",
    },
    LemmaSchema {
        name: "hyp.boundary-evidence",
        label: "assum:cas:hyp-boundary-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained boundary-calculus facts hold with their exact oriented roles.",
        premises: "Exactly the ordered premises consumed by the closed boundary mode.",
    },
    LemmaSchema {
        name: "hyp.case-evidence",
        label: "assum:cas:hyp-case-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The retained case facts hold on their exact regions.",
        premises: "Exactly the ordered premises consumed by the closed case mode.",
    },
    LemmaSchema {
        name: "hyp.exact-premises",
        label: "assum:cas:hyp-exact-premises",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "Every member of the exact ordered premise sequence holds.",
        premises: "Exactly the arity stored in the associated premise shape; each slot retains its typed statement and conditions.",
    },
    LemmaSchema {
        name: "hyp.discharge-premises",
        label: "assum:cas:hyp-discharge-premises",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The fixed premise prefix and every explicitly consumed discharge fact hold.",
        premises: "The minimum prefix arity stored in the associated premise shape, followed only by consumed evidence.",
    },
    LemmaSchema {
        name: "hyp.jacobian-cells",
        label: "assum:cas:hyp-jacobian-cells",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "Every ordered output and coordinate cell has its required derivative fact.",
        premises: "One derivative premise for every cell, with the original row and coordinate order.",
    },
    LemmaSchema {
        name: "hyp.nonpositive",
        label: "assum:cas:hyp-nonpositive",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact Real subject is nonpositive.",
        premises: "Original Real subject and qualified containing region.",
    },
    LemmaSchema {
        name: "hyp.germ-evidence",
        label: "assum:cas:hyp-germ-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function has the stated finite limit or series on its exact approach.",
        premises: "Exact function, boundary, approach, mode and finite coefficients or limit.",
    },
    LemmaSchema {
        name: "hyp.split-evidence",
        label: "assum:cas:hyp-split-evidence",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The original function has the stated finite integral or sum.",
        premises: "Exact function, domain or inclusive integer range and endpoint value.",
    },
];

impl HypothesisKind {
    /// Borrow this kind's formal name, revision, label and mathematical shape.
    #[must_use]
    pub const fn formal_schema(self) -> &'static LemmaSchema {
        match self {
            Self::DomainGuard => &HYPOTHESIS_SCHEMAS[0],
            Self::NonZero => &HYPOTHESIS_SCHEMAS[1],
            Self::StrictSign => &HYPOTHESIS_SCHEMAS[2],
            Self::NonNegative => &HYPOTHESIS_SCHEMAS[3],
            Self::IntegerRange => &HYPOTHESIS_SCHEMAS[4],
            Self::Differentiable => &HYPOTHESIS_SCHEMAS[5],
            Self::Equality => &HYPOTHESIS_SCHEMAS[6],
            Self::DeclaredZero => &HYPOTHESIS_SCHEMAS[7],
            Self::Symmetric => &HYPOTHESIS_SCHEMAS[8],
            Self::Conjunction => &HYPOTHESIS_SCHEMAS[9],
            Self::Disjunction => &HYPOTHESIS_SCHEMAS[10],
            Self::Negation => &HYPOTHESIS_SCHEMAS[11],
            Self::Universal => &HYPOTHESIS_SCHEMAS[12],
            Self::Named => &HYPOTHESIS_SCHEMAS[13],
            Self::Boolean => &HYPOTHESIS_SCHEMAS[14],
            Self::RegionMembership => &HYPOTHESIS_SCHEMAS[15],
            Self::EmptyRegion => &HYPOTHESIS_SCHEMAS[16],
            Self::Cover => &HYPOTHESIS_SCHEMAS[17],
            Self::Disjoint => &HYPOTHESIS_SCHEMAS[18],
            Self::Constructor => &HYPOTHESIS_SCHEMAS[19],
            Self::DefinedDomain => &HYPOTHESIS_SCHEMAS[20],
            Self::Compatibility => &HYPOTHESIS_SCHEMAS[21],
            Self::Elaboration => &HYPOTHESIS_SCHEMAS[22],
            Self::Incidence => &HYPOTHESIS_SCHEMAS[23],
            Self::Ownership => &HYPOTHESIS_SCHEMAS[24],
            Self::Derivative => &HYPOTHESIS_SCHEMAS[25],
            Self::Jacobian => &HYPOTHESIS_SCHEMAS[26],
            Self::SquareFree => &HYPOTHESIS_SCHEMAS[27],
            Self::Jet => &HYPOTHESIS_SCHEMAS[28],
            Self::Jump => &HYPOTHESIS_SCHEMAS[29],
            Self::BoundaryPartition => &HYPOTHESIS_SCHEMAS[30],
            Self::CaseDerivative => &HYPOTHESIS_SCHEMAS[31],
            Self::DerivativeAt => &HYPOTHESIS_SCHEMAS[32],
            Self::DerivativeNonexistence => &HYPOTHESIS_SCHEMAS[33],
            Self::Trace => &HYPOTHESIS_SCHEMAS[34],
            Self::SolveEvidence => &HYPOTHESIS_SCHEMAS[35],
            Self::DistributionEvidence => &HYPOTHESIS_SCHEMAS[36],
            Self::BoundaryEvidence => &HYPOTHESIS_SCHEMAS[37],
            Self::CaseEvidence => &HYPOTHESIS_SCHEMAS[38],
            Self::ExactPremises => &HYPOTHESIS_SCHEMAS[39],
            Self::DischargePremises => &HYPOTHESIS_SCHEMAS[40],
            Self::JacobianCells => &HYPOTHESIS_SCHEMAS[41],
            Self::SplitEvidence => &HYPOTHESIS_SCHEMAS[44],
            Self::GermEvidence => &HYPOTHESIS_SCHEMAS[43],
            Self::NonPositive => &HYPOTHESIS_SCHEMAS[42],
        }
    }
}

impl PremiseShape {
    /// Name this ordered-premise shape while retaining its exact arity in the original value.
    #[must_use]
    pub const fn hypothesis_kind(self) -> HypothesisKind {
        match self {
            Self::SolveEvidence => HypothesisKind::SolveEvidence,
            Self::DistributionEvidence => HypothesisKind::DistributionEvidence,
            Self::BoundaryEvidence => HypothesisKind::BoundaryEvidence,
            Self::CaseEvidence => HypothesisKind::CaseEvidence,
            Self::Exact(_) => HypothesisKind::ExactPremises,
            Self::AtLeast(_) => HypothesisKind::DischargePremises,
            Self::JacobianCells => HypothesisKind::JacobianCells,
        }
    }
}

impl PredicateKind {
    /// Classify the predicate constructor; a retained literal-zero equality can be more specific.
    #[must_use]
    pub const fn hypothesis_kind(&self) -> HypothesisKind {
        match self {
            Self::Defined => HypothesisKind::DomainGuard,
            Self::NonZero => HypothesisKind::NonZero,
            Self::Positive => HypothesisKind::StrictSign,
            Self::NonNegative => HypothesisKind::NonNegative,
            Self::IntegerRange => HypothesisKind::IntegerRange,
            Self::Differentiable => HypothesisKind::Differentiable,
            Self::Equal => HypothesisKind::Equality,
            Self::Symmetric => HypothesisKind::Symmetric,
            Self::And => HypothesisKind::Conjunction,
            Self::Or => HypothesisKind::Disjunction,
            Self::Not => HypothesisKind::Negation,
            Self::ForAll => HypothesisKind::Universal,
            Self::Named(_) => HypothesisKind::Named,
        }
    }
}

/// An inspected qualified hypothesis, with its kind derived from the retained frame and slot.
///
/// This value is metadata, never a certificate or an additional trusted field in a proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedHypothesis {
    /// Exact introducing frame, including its store identity.
    pub context: Handle,
    /// Original predicate slot, or local membership slot zero.
    pub slot: u32,
    /// Formal kind reconstructed from the exact retained predicate or membership binder.
    pub kind: HypothesisKind,
}

impl HypothesisRef {
    /// Inspect this frame and slot with its derived formal kind; foreign or absent slots refuse.
    ///
    /// The returned handle shares ownership without allocation. No wire field or proof authority is added.
    /// # Errors
    /// Refuses foreign frames, non-context handles, absent slots and invalid predicate references.
    pub fn qualified(&self, cas: &Cas) -> Result<QualifiedHypothesis, Fault> {
        let frame = cas.state.context(cas.checked(&self.context)?)?;
        let kind = if frame.parent.is_some() && self.slot == 0 {
            HypothesisKind::RegionMembership
        } else {
            let predicate = crate::context_check::predicate(cas, self)?;
            cas.predicate_hypothesis_kind(&predicate)?
        };
        Ok(QualifiedHypothesis {
            context: self.context.clone(),
            slot: self.slot,
            kind,
        })
    }
}
