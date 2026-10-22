// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed exact coefficient arithmetic and guarded finite specialization.
//!
//! # Coefficient arithmetic and identities
//!
//! The coefficient leaf supplies a sealed, fallible arithmetic interface over Q, Q(i) and rational functions. Sparse operations reserve uncancelled term bounds, charge exponent comparisons and recursively meter scalar operations; normalization uses deterministic merge sorting and exact polynomial factor reconstruction. Canonical fractions retain an existing operand through addition of zero and multiplication by zero or one. Equal-denominator addition still normalizes its resulting numerator; multiplication of two polynomials over unit denominators retains that canonical denominator. Zero numerators and nonzero ground denominators have direct canonical pairs; other normalizations retain exact gcd reconstruction. Operand compatibility, all actual comparisons, copies and scalar divisions remain metered. Canonical nontrivial fraction products cancel opposite numerator/denominator gcds before expansion. The shared gcd removes coefficient-polynomial content after each pseudo-division step and, when one operand omits a variable, computes the gcd with the other operand’s coefficients in that variable before using the general remainder sequence. The original coordinate order and exact input factor reconstructions remain mandatory; these formal reductions assert no regional unit. Arithmetic results and proposed inverse evidence remain unsealed. A formally nonzero parameter numerator is not a unit on every parameter fiber: regional inversion retains a nonvanishing obligation unless exact scoped evidence decides it. Finite exact specialization checks every named localization denominator and the evaluated value’s denominator, and can map a nonzero formal parameter to zero. Injective constant inclusions and distinct independent-variable renamings are separate map modes. The independent coefficient identity checker validates explicit maps, denominator premises and exact polynomial differences; branch-sensitive or unsupported expressions refuse.
//!
//! The multivariate gcd keeps its recursive remainders primitive over the integers by clearing rational denominators and dividing positive ground content after exact polynomial-content removal. Gaussian coefficients use both rational components. A deterministic internal variable permutation starts with the smallest main degree and is reversed before monic normalization and both input reconstructions. Exact division by a monomial checks each term directly. Canonical fraction sums cancel only the shared denominator gcd before forming the final denominator; polynomial summands preserve the other denominator. A reciprocal swaps the coprime pair and rescales it to a monic denominator after the caller separately proves a regional unit. These operations use the existing arithmetic and coefficient limits.
//!
//! Scoped equality predicates can justify coefficient relation multiples through `ScopedCoefficientRelation`: an exact earlier premise, two independently matched polynomial endpoints and an explicit ordered root-term interpretation. Formal polynomial slots remain distinct from scalar atoms. An optional `RootRelation` anchor requests existential discharge only for a complete Vieta system over a complex bound frame, with independent root-existence evidence for the exact polynomial, degree, leading coefficient and region. The checker reconstructs every elementary symmetric equation and refuses incomplete systems or conclusions, maps, regions and enclosing assumptions that still depend on the formal roots. This removes only the qualified Vieta hypotheses; other conditions and all proof dependencies remain. A local formal-root frame may close that discharged, root-independent identity into its exact original unscoped parent. Closure checks a bounded structural erasure of the complex binder, preserves every denominator and retained condition, and keeps the expanded identity, Vieta relations and independent existence proof reachable. The result is an ordinary coefficient equality usable by later unscoped rules. Ordinary scoped relations retain their hypotheses, and relations justified by earlier coefficient equalities keep their existing checking route. No formal root enters the computational tower. Bound contexts require every predicate and local restriction to match the exact region scope. Their executable proof route is limited to scoped coefficient identities, hypothesis introduction and closure, region construction, and independently checked root-independent unit/existence support. Scalar proof/search, automatic request producers and all other native rules refuse these contexts with `UnsupportedFragment`; empty-scope behavior is unchanged.
//!
//! Coefficient identities can compare a polynomial or value over a source quotient with its image in another coefficient domain through an explicit specialization map. Replay checks the source defining polynomial at the exact target generator image, transports source coefficients, and reads target coefficients directly. Both sides retain the same ordered formal variable frame, exact region and denominator evidence; an unused slot still belongs to that frame. The finite modulus check establishes a homomorphism and does not assert injectivity.
//!
//! Exact zero and nonzero guards on the validated evaluation region decide the matching canonical parameter polynomial. The checker follows guard bases and intersections, compares full headers and ordered declaration keys, and accepts total polynomial expressions or polynomial coefficient literals with canonical parameter maps. A fresh zero/nonzero split therefore permits inversion only on its nonzero cell, without adding a contextual hypothesis. Proved nonzero guards and scoped nonzero premises are closed under products and positive powers by repeated exact polynomial division down to a nonzero ground constant. Each successful division strictly lowers total degree, uses the existing coefficient and arithmetic limits, and rejects a nonzero remainder; overlapping proved factors retain alternative division routes. Thus the nonzero cell for a also defines 1/a² and 1/a³, while a product a·b requires both factors to be proved nonzero. No factorization or nonvanishing guess from polynomial spelling is added. Independent identity and denominator replay use this same checked closure; exact zero decisions, actual value denominators and finite localization obligations remain mandatory. Original membership uses this denominator closure when interpreting a fraction through a fully checked canonical scalar parameter map, including inside a selected root equation. Unions, arbitrary positivity and partial guard arithmetic do not become general region decisions.
//!
//! Regional Euclidean division decides effective degree before inverting a leading coefficient. A coefficient proved zero on the exact cell is removed from the local polynomial extent; an undecided coefficient retains its explicit obligation. Unit replay checks the supplied divisibilities, Bézout identity and inverse products modulo the contained zero guards, with all original denominators and exact region/context checks retained. Primitive polynomial gcd remainders may be rescaled by nonzero ground constants to control intermediate rational growth; their final exact factor reconstructions are unchanged.
//!
//! `IdentityMode::NonzeroPower(k)` enables an explicit regional nonvanishing consequence in `coeff.identity`: for positive `k`, the certificate supplies `f`, a product `u` of independently justified nonzero factors, and a finite identity `f^k - u = sum(h_j g_j)` where every `g_j` is zero on the exact cell. The checker recomputes the supplied identity and factors and concludes the exact mapped `NonZero(f)` predicate. It never searches exponents or cofactors. This uses the field law that a positive power equal to a nonzero product has a nonzero base. Wire mode 2 carries the exponent after its mode byte; its factor vector is the explicit product inventory, including repetitions. Modes 0 and 1 retain their prior meaning, the current wire remains `NRZCAS04`, and the logical policy remains version 10 with the existing identity weight.

#![expect(
    dead_code,
    reason = "later solving producers consume the sealed arithmetic and staging interfaces"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod arithmetic;
mod evaluation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod fraction;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod gcd;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod interface;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient maps stay within the crate"
)]
pub(crate) mod maps;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod polynomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod read;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod scalar;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient arithmetic stays within the crate"
)]
pub(crate) mod storage;
#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient transport is shared by private replay modules"
)]
mod transport;

/// Arithmetic availability does not enable an unchecked theorem schema.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Per-subtype capability permits independently green checker slices in this leaf.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if tag == 0 {
        super::Availability::Available
    } else {
        super::Availability::Unavailable
    }
}

/// Independently replay the typed coefficient identity certificate.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    identity::check(cas, draft, premises, budget)
}

/// Independently verify coprimality and the monic denominator during graph admission.
pub(crate) fn canonical_fraction(
    state: &crate::graph::State,
    numerator: &crate::graph::Handle,
    denominator: &crate::graph::Handle,
    budget: &mut crate::budget::Budget,
) -> Result<(), crate::error::Fault> {
    read::canonical_fraction(state, numerator, denominator, budget)
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent coefficient replay stays within the crate"
)]
pub(crate) mod proof;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent coefficient replay stays within the crate"
)]
pub(crate) mod forms;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent coefficient replay stays within the crate"
)]
pub(crate) mod normalize;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent coefficient replay stays within the crate"
)]
pub(crate) mod identity;

#[expect(
    clippy::redundant_pub_crate,
    reason = "explicit map hypotheses stay within the crate"
)]
pub(crate) mod homomorphism;

#[expect(
    clippy::redundant_pub_crate,
    reason = "scoped nonzero evidence stays within the crate"
)]
pub(crate) mod nonvanishing;

#[expect(
    clippy::redundant_pub_crate,
    reason = "regional coefficient checks stay within the crate"
)]
pub(crate) mod guard_polynomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "regional coefficient checks stay within the crate"
)]
pub(crate) mod regional;

#[expect(
    clippy::redundant_pub_crate,
    reason = "checked nonzero closure stays within the crate"
)]
pub(crate) mod nonzero;

#[expect(
    clippy::redundant_pub_crate,
    reason = "scoped coefficient evidence stays within the crate"
)]
mod scoped;

#[expect(
    clippy::redundant_pub_crate,
    reason = "existential coefficient evidence stays within the crate"
)]
mod independent;

#[expect(
    clippy::redundant_pub_crate,
    reason = "existential coefficient evidence stays within the crate"
)]
mod vieta;

#[expect(
    clippy::redundant_pub_crate,
    reason = "scoped context support is checked before native dispatch"
)]
pub(crate) mod scope;

#[expect(
    clippy::redundant_pub_crate,
    reason = "formal closure is shared with context admission and checking"
)]
pub(crate) mod formal_close;
#[expect(
    clippy::redundant_pub_crate,
    reason = "formal erasure is shared by the private context bridge"
)]
pub(crate) mod formal_shape;

#[expect(
    clippy::redundant_pub_crate,
    reason = "regional certificate production stays within the crate"
)]
pub(crate) mod consequence;
mod consequence_check;
