// SPDX-License-Identifier: AGPL-3.0-only
//! Named mathematical contracts for every closed primitive, without additional checker authority.

use crate::emptiness::LemmaSchema;
use crate::rule::{PremiseShape, Primitive};
use crate::solving::SolveSchema;

/// A formal primitive statement and its existing typed ordered-premise contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveSchema {
    /// Named theorem or reserved hypothesis schema with its estate label.
    pub lemma: LemmaSchema,
    /// Exact premise shape used by the compiled checker dispatch.
    pub premises: PremiseShape,
}

/// Bind documentation to existing keys, revisions and premise shapes without changing them.
const fn named(
    primitive: Primitive,
    label: &'static str,
    statement: &'static str,
    premises: &'static str,
) -> PrimitiveSchema {
    let rule = primitive.specification();
    PrimitiveSchema {
        lemma: LemmaSchema {
            name: rule.key,
            label,
            version: rule.version,
            standing: if rule.available {
                crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked)
            } else {
                crate::proof::Standing::Hypothesis
            },
            statement,
            premises,
        },
        premises: rule.premises,
    }
}

/// Every native and solving primitive in the same ASCII key order as `PRIMITIVES`.
///
/// Unavailable schemas remain represented assumptions and cannot certify an application.
pub static PRIMITIVE_SCHEMAS: [PrimitiveSchema; 80] = [
    named(
        Primitive::CaseCongruence,
        "thm:cas:primitive-case-congruence",
        "Local agreement or complete case and domain congruence preserves the stated case relation.",
        "Mode-specific ordered local equalities, compatibility and complete case/domain evidence.",
    ),
    named(
        Primitive::CaseDomain,
        "thm:cas:primitive-case-domain",
        "The exact defined domain of a compatible case is determined by its retained arms.",
        "Mode-specific case compatibility and domain evidence for all retained arms.",
    ),
    named(
        Primitive::CaseEliminate,
        "thm:cas:primitive-case-eliminate",
        "A complete checked cover joins matching restricted conclusions into the stated conclusion.",
        "A checked cover and matching closed conclusions for every member, with every inherited condition retained.",
    ),
    named(
        Primitive::CaseOrdered,
        "thm:cas:primitive-case-ordered",
        "Ordered source clauses elaborate to native cases on their exact reached regions.",
        "Mode-specific ordered case evidence and complete source-clause, reach and association inventory.",
    ),
    named(
        Primitive::CaseSelect,
        "thm:cas:primitive-case-select",
        "A selected case arm agrees locally with the case, or determines its actual point ownership.",
        "Mode-specific association, containment and point-ownership evidence for the exact function and region.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::CoefficientEvaluation),
        "thm:cas:primitive-coeff-evaluate",
        "Exact substitution evaluates a coefficient expression to the stated value under its original domain.",
        "Typed evaluation certificate with exact coefficient algebra, ordered substitutions and all definedness and divisor evidence.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::CoefficientIdentity),
        "thm:cas:primitive-coeff-identity",
        "Identical exact coefficient values or transported coefficient expressions satisfy the stated defined identity.",
        "Typed coefficient certificate and its ordered identity, mapping, projection and nonzero evidence, retaining every original divisor.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ConjugationTransport),
        "thm:cas:primitive-complex-conjugate",
        "Structural complex conjugation transports the stated expression identity under the original declarations.",
        "Typed conjugation certificate with exact expression induction and compatible selected-root evidence; independent root choices remain distinct.",
    ),
    named(
        Primitive::ContextClose,
        "thm:cas:primitive-ctx-close",
        "Closing a local frame preserves its conclusion and every undischarged predicate as a residual.",
        "A local theorem and its exact region-constructor theorem, followed only by consumed predicate discharge proofs.",
    ),
    named(
        Primitive::ContextHypothesis,
        "thm:cas:primitive-ctx-hypothesis",
        "A qualified current or ancestor hypothesis holds under that exact frame.",
        "No ordered proof premises; an exact introducing frame and predicate slot, or local membership slot zero.",
    ),
    named(
        Primitive::ContextWeaken,
        "thm:cas:primitive-ctx-weaken",
        "A theorem remains valid under its checked restriction or semantic embedding.",
        "The original theorem followed by exact subset or family projection evidence; every used assumption is retained or explicitly transported.",
    ),
    named(
        Primitive::DifferentiateAdd,
        "thm:cas:primitive-diff-add",
        "The derivative of an ordered sum is the ordered sum of its derivatives.",
        "Two ordered operand derivatives on the identical domain and coordinate.",
    ),
    named(
        Primitive::CaseBoundary,
        "thm:cas:primitive-diff-case-boundary",
        "Actual point ownership and matching incident jets determine a point derivative or its finite obstruction.",
        "The closed boundary mode's ownership and incident jet premises for the identical function, point and sides.",
    ),
    named(
        Primitive::CaseInterior,
        "thm:cas:primitive-diff-case-interior",
        "A checked local case equality transports a derivative on an interior region.",
        "The closed boundary mode's local equality, incidence and derivative premises on the exact region.",
    ),
    named(
        Primitive::DifferentiateConstant,
        "thm:cas:primitive-diff-constant",
        "A rational or named constant has zero derivative in a declared Real coordinate.",
        "No ordered proof premises; the exact constant and declared coordinate.",
    ),
    named(
        Primitive::DifferentiatePower,
        "thm:cas:primitive-diff-integer-power",
        "A fixed integer power has the checked power-rule derivative on its strict domain.",
        "One base derivative on the identical domain and coordinate, with literal exponent and strict base requirements.",
    ),
    named(
        Primitive::Jacobian,
        "thm:cas:primitive-diff-jacobian",
        "An ordered matrix of checked partial derivatives is the Jacobian of the ordered output vector.",
        "One ordered derivative premise per output and coordinate cell, with matching declarations and domain.",
    ),
    named(
        Primitive::DifferentiateNeg,
        "thm:cas:primitive-diff-neg",
        "The derivative of a negation is the negated derivative.",
        "One derivative of the original operand on the identical domain and coordinate.",
    ),
    named(
        Primitive::DifferentiateProduct,
        "thm:cas:primitive-diff-product",
        "The derivative of a scalar product is the sum of the two ordered product-rule terms.",
        "Two ordered operand derivatives on the identical domain and coordinate, retaining original definedness.",
    ),
    named(
        Primitive::DifferentiateQuotient,
        "thm:cas:primitive-diff-quotient",
        "The derivative of a quotient is the quotient-rule numerator over the squared denominator.",
        "Two ordered operand derivatives on the identical domain and coordinate, retaining the original nonzero denominator.",
    ),
    named(
        Primitive::DifferentiateSub,
        "thm:cas:primitive-diff-sub",
        "The derivative of an ordered difference is the ordered difference of its derivatives.",
        "Two ordered operand derivatives on the identical domain and coordinate.",
    ),
    named(
        Primitive::DerivativeTransport,
        "thm:cas:primitive-diff-transport",
        "A derivative result can be replaced by a defined equal result.",
        "One derivative followed by a defined equality from its result, on the same domain.",
    ),
    named(
        Primitive::DifferentiateVariable,
        "thm:cas:primitive-diff-variable",
        "A declared scalar coordinate has derivative one in itself and zero in another declared Real coordinate.",
        "No ordered proof premises; exact declaration identities and coordinate.",
    ),
    named(
        Primitive::DistAe,
        "thm:cas:primitive-dist-ae",
        "Functions equal away from a checked finite exceptional set induce equal regular distributions.",
        "Closed almost-everywhere mode evidence for both regular embeddings and the complete finite exceptional-set equality.",
    ),
    named(
        Primitive::DistAffinePullback,
        "thm:cas:primitive-dist-affine-pullback",
        "An invertible affine change pulls a delta back with the reciprocal absolute Jacobian.",
        "No ordered proof premises; both exact spaces, affine inverse, support point and nonzero slope certificate.",
    ),
    named(
        Primitive::DistDeltaMultiply,
        "thm:cas:primitive-dist-delta-multiply",
        "Multiplication of a delta derivative by a smooth function obeys the finite signed derivative formula.",
        "Closed multiplication-mode smoothness and support evidence, including every required derivative at the exact support point.",
    ),
    named(
        Primitive::DistDeltaPair,
        "thm:cas:primitive-dist-delta-pair",
        "A delta derivative paired with a smooth test equals the signed test derivative at its support point.",
        "Closed pairing-mode membership and smooth test evidence at the exact interior support point and derivative order.",
    ),
    named(
        Primitive::DistDerivative,
        "thm:cas:primitive-dist-derivative",
        "Distribution differentiation respects the checked linear derivative identities in the stated space.",
        "Closed derivative-mode ordered distribution and derivative premises with exact positive order and coordinate.",
    ),
    named(
        Primitive::DistJump,
        "thm:cas:primitive-dist-jump",
        "The distribution derivative of a piecewise polynomial includes the exact jumps at its retained boundaries.",
        "Closed jump-mode regular embedding, complete partition and oriented jet premises for every contributing boundary.",
    ),
    named(
        Primitive::DistLinear,
        "thm:cas:primitive-dist-linear",
        "Exact linear combinations preserve distribution membership, equality and scalar action in one space.",
        "Closed linear-mode ordered membership, equality or action premises with exact scalar coefficients.",
    ),
    named(
        Primitive::DistPrimitiveUnique,
        "thm:cas:primitive-dist-primitive-unique",
        "Distributions with equal first derivatives on a connected open interval differ by a constant distribution.",
        "Two derivative premises, with a third equality premise exactly when their derivative expressions differ.",
    ),
    named(
        Primitive::DistRegular,
        "thm:cas:primitive-dist-regular",
        "A function with checked local integrability defines a regular distribution on its exact space.",
        "Closed regularity-mode evidence for the original function and space, including every required partition or jet premise.",
    ),
    named(
        Primitive::DistSimpleZeroPullback,
        "thm:cas:primitive-dist-simple-zero-pullback",
        "A polynomial delta pullback is the sum over all simple preimages weighted by reciprocal absolute slopes.",
        "No ordered proof premises; complete rational-root factorization, distinct simple preimages, exact spaces and inverse-Jacobian evidence.",
    ),
    named(
        Primitive::DistSmooth,
        "thm:cas:primitive-dist-smooth",
        "A checked smooth function is an admissible distribution multiplier on its exact space.",
        "Closed smoothness-mode polynomial or jet evidence with the required domain and order.",
    ),
    named(
        Primitive::DistSupportAction,
        "thm:cas:primitive-dist-support-action",
        "A supported distribution acts only through the checked test data on its exact support.",
        "Closed support-action mode premises for the distribution, test function and exact support relation.",
    ),
    named(
        Primitive::AddAssociate,
        "thm:cas:primitive-eq-add-associate",
        "An ordered scalar sum equals its right-associated form.",
        "No ordered proof premises; three original scalar operands and their domain requirements.",
    ),
    named(
        Primitive::AddCommute,
        "thm:cas:primitive-eq-add-commute",
        "Scalar addition commutes on the common defined domain.",
        "No ordered proof premises; both original scalar operands and their domain requirements.",
    ),
    named(
        Primitive::AddInverse,
        "thm:cas:primitive-eq-add-inverse",
        "A scalar plus its negation equals zero on its defined domain.",
        "No ordered proof premises; the identical original scalar and its negation.",
    ),
    named(
        Primitive::AddZero,
        "thm:cas:primitive-eq-add-zero",
        "Adding a right zero preserves a defined scalar.",
        "No ordered proof premises; the original scalar and literal zero.",
    ),
    named(
        Primitive::Cancel,
        "thm:cas:primitive-eq-cancel",
        "The quotient of an identical nonzero numerator and denominator equals one.",
        "No ordered proof premises; the identical original scalar with its nonzero requirement.",
    ),
    named(
        Primitive::Congruence,
        "thm:cas:primitive-eq-congruence",
        "Replacing an operand by a defined equal operand preserves the enclosing scalar expression.",
        "One equality and an exact ordered operand path, with all original domain requirements.",
    ),
    named(
        Primitive::Distribute,
        "thm:cas:primitive-eq-distribute",
        "A left scalar factor distributes over a sum.",
        "No ordered proof premises; the original factor and summands with their domain requirements.",
    ),
    named(
        Primitive::Power,
        "thm:cas:primitive-eq-integer-power",
        "Strict integer powers obey the checked recurrence on their original defined domain.",
        "No ordered proof premises; literal integer exponent and all strict base requirements, including a nonzero base for exponent zero.",
    ),
    named(
        Primitive::MulAssociate,
        "thm:cas:primitive-eq-mul-associate",
        "An ordered scalar product equals its right-associated form.",
        "No ordered proof premises; three original scalar operands and their domain requirements.",
    ),
    named(
        Primitive::MulCommute,
        "thm:cas:primitive-eq-mul-commute",
        "Scalar multiplication commutes on the common defined domain.",
        "No ordered proof premises; both original scalar operands and their domain requirements.",
    ),
    named(
        Primitive::MulOne,
        "thm:cas:primitive-eq-mul-one",
        "Multiplying by a right one preserves a defined scalar.",
        "No ordered proof premises; the original scalar and literal one.",
    ),
    named(
        Primitive::MulZero,
        "thm:cas:primitive-eq-mul-zero",
        "A defined scalar multiplied by a right zero equals zero.",
        "No ordered proof premises; the original scalar remains required to be defined.",
    ),
    named(
        Primitive::Reflexivity,
        "thm:cas:primitive-eq-refl",
        "Every expression equals itself wherever its original operands are defined.",
        "No ordered proof premises; original definedness requirements.",
    ),
    named(
        Primitive::Substitute,
        "thm:cas:primitive-eq-substitute",
        "Simultaneous substitution preserves an equality together with all transported domain requirements.",
        "One equality and a simultaneous substitution of its endpoints and every original requirement.",
    ),
    named(
        Primitive::Symmetry,
        "thm:cas:primitive-eq-sym",
        "Defined equality is symmetric on the same domain.",
        "One defined equality with the endpoints reversed.",
    ),
    named(
        Primitive::Transitivity,
        "thm:cas:primitive-eq-trans",
        "Two defined equalities with the same middle expression compose on the same domain.",
        "Two ordered equalities with identical middle expression and domain.",
    ),
    named(
        Primitive::Exact,
        "thm:cas:primitive-exact-rational",
        "A supported operation on rational literals equals its exact rational result.",
        "No ordered proof premises; exact literal operands and defined arithmetic.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RootAdjunction),
        "thm:cas:primitive-extension-adjoin",
        "Adjoining a checked root satisfies its monic defining polynomial in the exact extension.",
        "Typed root-adjunction certificate with exact base algebra, defining polynomial and selected generator evidence.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ComponentSplit),
        "thm:cas:primitive-extension-split",
        "A complete coprime factorization splits the exact extension into its stated components.",
        "Typed split certificate with complete factor identity, coprimality and component projection evidence.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ExtensionUnit),
        "thm:cas:primitive-extension-unit",
        "A checked inverse product or Bezout identity proves an extension element is a unit.",
        "Typed unit certificate with exact algebra, candidate inverse and independently checked product or Bezout evidence.",
    ),
    named(
        Primitive::IntegralCaseSplit,
        "thm:cas:primitive-integral-case-split",
        "A complete finite polynomial interval partition integrates to the sum of its endpoint differences.",
        "No ordered proof premises; complete cover, finite-point extensions, polynomial antiderivatives and exact endpoint certificates.",
    ),
    named(
        Primitive::JetPolynomial,
        "thm:cas:primitive-jet-polynomial",
        "Exact polynomial pieces determine a complete finite partition and oriented jets.",
        "Mode-specific partition, incidence and polynomial coefficient evidence with exact side and boundary roles.",
    ),
    named(
        Primitive::LimitCaseRestrict,
        "thm:cas:primitive-limit-case-restrict",
        "Matching polynomial traces on all required approaches determine the stated finite punctured limit.",
        "One checked trace per required side, with identical function, boundary, order and value.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ExactLinearEquivalence),
        "thm:cas:primitive-linear-equivalent",
        "Exact reversible row and coordinate transformations preserve the original linear solution set.",
        "Typed linear certificate with full matrices, checked inverse transformations, unit pivots and strict original domains.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::PermutationOrbitCoverage),
        "thm:cas:primitive-orbit-cover",
        "Checked permutation images of retained representatives cover the stated solution orbit.",
        "Typed orbit certificate with exact action, representatives, every image and complete coverage evidence.",
    ),
    named(
        Primitive::Jets,
        "thm:cas:primitive-poly-jets-equal",
        "Matching finite coefficient maps prove defined equality in the supported scalar, ordered matrix or affine reading grammar.",
        "No ordered proof premises; independently reconstructed original expressions and all domain requirements.",
    ),
    named(
        Primitive::RationalIdentity,
        "thm:cas:primitive-poly-rational-equal",
        "Equal cross-multiplied finite coefficient maps prove equality of defined rational expressions.",
        "No ordered proof premises; every original divisor remains a nonzero requirement.",
    ),
    named(
        Primitive::SquareFree,
        "thm:cas:primitive-poly-square-free",
        "Exact coprime square-free layers reconstruct the original polynomial with their stated multiplicities.",
        "No ordered proof premises; exact polynomial identities, GCD and Bezout certificates, and original domain requirements.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RealCellExists),
        "thm:cas:primitive-real-cell-exists",
        "A checked Real root count establishes existence of a selected root in its exact cell.",
        "Typed cell-existence certificate with the corresponding checked count, exact cell and root selection.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RealRootCount),
        "thm:cas:primitive-real-root-count",
        "An exact Sturm variation certificate determines the number of Real polynomial roots in the stated cell.",
        "Typed root-count certificate with exact polynomial, endpoint signs, variation evidence and all unit requirements.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RealSign),
        "thm:cas:primitive-real-sign",
        "Checked ordered-field sign inferences establish the stated sign of a defined Real subject.",
        "Typed sign certificate with exact ordered sign premises, original strict guards and supported arithmetic or polynomial evidence.",
    ),
    named(
        Primitive::RegionConstruct,
        "thm:cas:primitive-region-construct",
        "A region constructor satisfies its exact defining membership law.",
        "No ordered proof premises; the original typed region constructor.",
    ),
    named(
        Primitive::RegionDecide,
        "thm:cas:primitive-region-decide",
        "Exact finite cells or a checked emptiness DAG establish the stated regional relation.",
        "Zero or more earlier proofs consumed by the certificate, including complete covers and qualified sign, equality or empty-region facts.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::BinomialCoverage),
        "thm:cas:primitive-root-binomial",
        "A checked binomial root construction supplies the complete stated finite root set.",
        "Typed binomial certificate with exact order, leading coefficient, root selections and complete zero or nonzero radicand evidence.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RealIsolationAndOrder),
        "assum:cas:primitive-root-isolate",
        "The represented isolation schema would certify exact Real root intervals and their order.",
        "Reserved typed isolation evidence; no application is accepted because this schema has no executable checker.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::RootSelectionRefinement),
        "thm:cas:primitive-root-select",
        "Exact selection refinement preserves a root with checked existence and its defining relation.",
        "Typed refinement certificate with original root selection, existence, coefficient relation and every required sign or compatibility premise.",
    ),
    named(
        Primitive::SeriesCaseRestrict,
        "thm:cas:primitive-series-case-restrict",
        "Checked side jets and exact polynomial remainders determine the stated finite series.",
        "One jet per required approach, centre ownership when required, and exact polynomial remainder certificates.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ParameterImageCoverage),
        "thm:cas:primitive-solve-family",
        "Exact parameter images with checked domains and complete branches cover the original family relation.",
        "Typed family certificate with universal substitution, exact maps, parameter domains, original predicate and complete coverage evidence.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ImplicitFunctionBranch),
        "assum:cas:primitive-solve-local-branch",
        "The represented implicit-function schema would certify a local solution branch under its regularity hypotheses.",
        "Reserved typed branch and regularity evidence; no application is accepted because this schema has no executable checker.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::Membership),
        "thm:cas:primitive-solve-member",
        "A substituted tuple belongs to the original solution set when its predicate, domain and original divisors hold.",
        "Typed membership certificate with ordered tuple, original predicate and domain evidence, selected-root existence and every divisor requirement.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::MultiplicityTransfer),
        "thm:cas:primitive-solve-multiplicity",
        "The first nonzero derivative order at a root determines its polynomial multiplicity.",
        "Typed derivative-multiplicity certificate with exact membership and every derivative through the first nonzero order.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::ProductCompleteness),
        "thm:cas:primitive-solve-product",
        "A complete polynomial factor identity and checked root memberships exhaust the admissible roots.",
        "Typed product certificate with every positive-multiplicity factor, original divisor inventory and exact exclusion evidence for discarded roots.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::TotalPredicateSplit),
        "thm:cas:primitive-solve-split",
        "A total predicate partition or checked empty region yields complete regional solution coverage.",
        "Typed split certificate with definedness and complete branch evidence, or the exact checked empty-region premise.",
    ),
    named(
        Primitive::Solve(crate::solving::SolveSchema::SolutionTransport),
        "thm:cas:primitive-solve-substitute",
        "Checked inverse coordinate maps transport membership and complete solution coverage.",
        "Typed transport certificate with both map directions, inverse compositions, strict domains and original solution evidence.",
    ),
    named(
        Primitive::SumCaseSplit,
        "thm:cas:primitive-sum-case-split",
        "A complete finite inclusive integer partition sums to the checked forward-difference endpoint expression.",
        "No ordered proof premises; complete range cover, exact forward differences and endpoint certificates.",
    ),
];

impl Primitive {
    /// Borrow the formal contract of this exact primitive without allocation or proof checking.
    #[must_use]
    pub const fn formal_schema(self) -> &'static PrimitiveSchema {
        match self {
            Self::Solve(schema) => solving(schema),
            Self::CaseCongruence => &PRIMITIVE_SCHEMAS[0],
            Self::CaseDomain => &PRIMITIVE_SCHEMAS[1],
            Self::CaseEliminate => &PRIMITIVE_SCHEMAS[2],
            Self::CaseOrdered => &PRIMITIVE_SCHEMAS[3],
            Self::CaseSelect => &PRIMITIVE_SCHEMAS[4],
            Self::ContextClose => &PRIMITIVE_SCHEMAS[8],
            Self::ContextHypothesis => &PRIMITIVE_SCHEMAS[9],
            Self::ContextWeaken => &PRIMITIVE_SCHEMAS[10],
            Self::DifferentiateAdd => &PRIMITIVE_SCHEMAS[11],
            Self::CaseBoundary => &PRIMITIVE_SCHEMAS[12],
            Self::CaseInterior => &PRIMITIVE_SCHEMAS[13],
            Self::DifferentiateConstant => &PRIMITIVE_SCHEMAS[14],
            Self::DifferentiatePower => &PRIMITIVE_SCHEMAS[15],
            Self::Jacobian => &PRIMITIVE_SCHEMAS[16],
            Self::DifferentiateNeg => &PRIMITIVE_SCHEMAS[17],
            Self::DifferentiateProduct => &PRIMITIVE_SCHEMAS[18],
            Self::DifferentiateQuotient => &PRIMITIVE_SCHEMAS[19],
            Self::DifferentiateSub => &PRIMITIVE_SCHEMAS[20],
            Self::DerivativeTransport => &PRIMITIVE_SCHEMAS[21],
            Self::DifferentiateVariable => &PRIMITIVE_SCHEMAS[22],
            Self::DistAe => &PRIMITIVE_SCHEMAS[23],
            Self::DistAffinePullback => &PRIMITIVE_SCHEMAS[24],
            Self::DistDeltaMultiply => &PRIMITIVE_SCHEMAS[25],
            Self::DistDeltaPair => &PRIMITIVE_SCHEMAS[26],
            Self::DistDerivative => &PRIMITIVE_SCHEMAS[27],
            Self::DistJump => &PRIMITIVE_SCHEMAS[28],
            Self::DistLinear => &PRIMITIVE_SCHEMAS[29],
            Self::DistPrimitiveUnique => &PRIMITIVE_SCHEMAS[30],
            Self::DistRegular => &PRIMITIVE_SCHEMAS[31],
            Self::DistSimpleZeroPullback => &PRIMITIVE_SCHEMAS[32],
            Self::DistSmooth => &PRIMITIVE_SCHEMAS[33],
            Self::DistSupportAction => &PRIMITIVE_SCHEMAS[34],
            Self::AddAssociate => &PRIMITIVE_SCHEMAS[35],
            Self::AddCommute => &PRIMITIVE_SCHEMAS[36],
            Self::AddInverse => &PRIMITIVE_SCHEMAS[37],
            Self::AddZero => &PRIMITIVE_SCHEMAS[38],
            Self::Cancel => &PRIMITIVE_SCHEMAS[39],
            Self::Congruence => &PRIMITIVE_SCHEMAS[40],
            Self::Distribute => &PRIMITIVE_SCHEMAS[41],
            Self::Power => &PRIMITIVE_SCHEMAS[42],
            Self::MulAssociate => &PRIMITIVE_SCHEMAS[43],
            Self::MulCommute => &PRIMITIVE_SCHEMAS[44],
            Self::MulOne => &PRIMITIVE_SCHEMAS[45],
            Self::MulZero => &PRIMITIVE_SCHEMAS[46],
            Self::Reflexivity => &PRIMITIVE_SCHEMAS[47],
            Self::Substitute => &PRIMITIVE_SCHEMAS[48],
            Self::Symmetry => &PRIMITIVE_SCHEMAS[49],
            Self::Transitivity => &PRIMITIVE_SCHEMAS[50],
            Self::Exact => &PRIMITIVE_SCHEMAS[51],
            Self::IntegralCaseSplit => &PRIMITIVE_SCHEMAS[55],
            Self::JetPolynomial => &PRIMITIVE_SCHEMAS[56],
            Self::LimitCaseRestrict => &PRIMITIVE_SCHEMAS[57],
            Self::Jets => &PRIMITIVE_SCHEMAS[60],
            Self::RationalIdentity => &PRIMITIVE_SCHEMAS[61],
            Self::SquareFree => &PRIMITIVE_SCHEMAS[62],
            Self::RegionConstruct => &PRIMITIVE_SCHEMAS[66],
            Self::RegionDecide => &PRIMITIVE_SCHEMAS[67],
            Self::SeriesCaseRestrict => &PRIMITIVE_SCHEMAS[71],
            Self::SumCaseSplit => &PRIMITIVE_SCHEMAS[79],
        }
    }
}

/// Exhaustive solving-schema lookup keeps unavailable entries visible without enabling them.
const fn solving(schema: SolveSchema) -> &'static PrimitiveSchema {
    match schema {
        SolveSchema::CoefficientEvaluation => &PRIMITIVE_SCHEMAS[5],
        SolveSchema::CoefficientIdentity => &PRIMITIVE_SCHEMAS[6],
        SolveSchema::ConjugationTransport => &PRIMITIVE_SCHEMAS[7],
        SolveSchema::RootAdjunction => &PRIMITIVE_SCHEMAS[52],
        SolveSchema::ComponentSplit => &PRIMITIVE_SCHEMAS[53],
        SolveSchema::ExtensionUnit => &PRIMITIVE_SCHEMAS[54],
        SolveSchema::ExactLinearEquivalence => &PRIMITIVE_SCHEMAS[58],
        SolveSchema::PermutationOrbitCoverage => &PRIMITIVE_SCHEMAS[59],
        SolveSchema::RealCellExists => &PRIMITIVE_SCHEMAS[63],
        SolveSchema::RealRootCount => &PRIMITIVE_SCHEMAS[64],
        SolveSchema::RealSign => &PRIMITIVE_SCHEMAS[65],
        SolveSchema::BinomialCoverage => &PRIMITIVE_SCHEMAS[68],
        SolveSchema::RealIsolationAndOrder => &PRIMITIVE_SCHEMAS[69],
        SolveSchema::RootSelectionRefinement => &PRIMITIVE_SCHEMAS[70],
        SolveSchema::ParameterImageCoverage => &PRIMITIVE_SCHEMAS[72],
        SolveSchema::ImplicitFunctionBranch => &PRIMITIVE_SCHEMAS[73],
        SolveSchema::Membership => &PRIMITIVE_SCHEMAS[74],
        SolveSchema::MultiplicityTransfer => &PRIMITIVE_SCHEMAS[75],
        SolveSchema::ProductCompleteness => &PRIMITIVE_SCHEMAS[76],
        SolveSchema::TotalPredicateSplit => &PRIMITIVE_SCHEMAS[77],
        SolveSchema::SolutionTransport => &PRIMITIVE_SCHEMAS[78],
    }
}

/// Closed constant theorems with independently reconstructed rational certificates.
pub const CONSTANT_SCHEMAS: [LemmaSchema; 1] = [LemmaSchema {
    name: "constant.pi-machin-bounds",
    label: "thm:cas:constant-pi-machin-bounds",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "The claimed closed rational interval contains mathematical pi.",
    premises: "Exact finite alternating sums for atan(1/5) and atan(1/239), their signed next-term remainders, Machin's identity and outward rational endpoint comparisons.",
}];
