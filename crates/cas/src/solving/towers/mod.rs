// SPDX-License-Identifier: AGPL-3.0-only
//! Finite quotient arithmetic and independently replayed unit/component evidence.
//!
//! # Quotient arithmetic and root existence
//!
//! The tower leaf enables `coeff.identity`, `extension.unit` and `extension.split` (tags 0, 2 and 3). Monic finite quotients use parent arithmetic followed by descending reduction, and Euclidean inversion returns a unit, zero, a nonvanishing obligation or a component split. An earlier parent split takes precedence over a child computation. Replay independently verifies gcd divisibilities, Bézout identities and both inverse products. Component covers additionally reconstruct the proper factor product, coprimality, derivative-unit square-freeness, inductive parent reducedness and the actual child-generator projections. Component staging rebuilds dependent coefficient domains, polynomials and values from parent to child on both branches in one private transaction. Selection transport preserves the same ordered joint predicate and every unchanged relation slot on both children; mismatched constraints or a root sent to the wrong factor refuse. No quotient is called a field merely because its defining polynomial is square-free.
//!
//! `extension.adjoin` (tag 1) checks an exact leading-unit premise and a well-defined scalar coefficient map before asserting root existence. Complex FTA and algebraic closure of C support unconstrained root relations/selections and checked degree-decreasing splitting chains. Primitive-unity existence reconstructs the cyclotomic polynomial from the divisor product of Xⁿ−1; no numerical root or orientation is selected. Real odd binomials admit real roots, while positive/nonnegative real square-root selections require exact radicand signs or matching scoped sign premises. A selection’s principal or isolated metadata cannot supply existence or uniqueness. Caller-named fields remain unavailable without a separately specified closure interpretation.

#![expect(
    dead_code,
    reason = "later solving producers consume the tower arithmetic and component staging interfaces"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "tower arithmetic is shared within the crate"
)]
pub(crate) mod arithmetic;
#[expect(
    clippy::redundant_pub_crate,
    reason = "parent polynomial arithmetic is shared within the crate"
)]
pub(crate) mod dense;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Euclidean work is shared within the crate"
)]
pub(crate) mod euclid;
#[expect(
    clippy::redundant_pub_crate,
    reason = "unit decisions are shared within the crate"
)]
pub(crate) mod units;

#[expect(
    clippy::redundant_pub_crate,
    reason = "Sibling coefficient and tower modules share checked internals."
)]
pub(crate) mod transport;

/// Unsealed split proposal, always naming the earliest level needing a split.
#[derive(Debug)]
pub(crate) struct SplitCertificate {
    /// Exact parent extension level.
    pub(crate) level: crate::graph::Handle,
    /// Monic proper first factor.
    pub(crate) left: dense::Dense,
    /// Monic proper complementary factor.
    pub(crate) right: dense::Dense,
    /// First coprimality multiplier.
    pub(crate) first: dense::Dense,
    /// Second coprimality multiplier.
    pub(crate) second: dense::Dense,
}

/// Finite quotient arithmetic, units and component covers have independent replay checkers.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Per-subtype capability permits independently green checker slices in this leaf.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if matches!(tag, 1..=3) {
        super::Availability::Available
    } else {
        super::Availability::Unavailable
    }
}

/// Dispatch only independently implemented typed tower schemas.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    use crate::proof_model::Certificate;
    use crate::solving::{SolveCertificate, SolveSchema};
    let inherited = crate::solving::coefficients::proof::conditions(cas, draft, premises, budget)?;
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(crate::solving::coefficients::storage::invalid());
    };
    match (draft.rule, certificate.as_ref()) {
        (
            crate::rule::Primitive::Solve(SolveSchema::RootAdjunction),
            SolveCertificate::RootAdjunction(value),
        ) => adjoin::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::ExtensionUnit),
            SolveCertificate::ExtensionUnit(value),
        ) => check_unit::check(cas, draft, value, premises, budget)?,
        (
            crate::rule::Primitive::Solve(SolveSchema::ComponentSplit),
            SolveCertificate::ComponentSplit(value),
        ) => check_split::check(cas, draft, value, premises, budget)?,
        _ => return Err(crate::solving::coefficients::storage::invalid()),
    }
    Ok(inherited)
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent tower replay stays within the crate"
)]
pub(crate) mod check_unit;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent tower replay stays within the crate"
)]
pub(crate) mod check_split;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent tower replay stays within the crate"
)]
pub(crate) mod selection;

#[expect(
    clippy::redundant_pub_crate,
    reason = "root existence replay stays within the crate"
)]
pub(crate) mod cyclotomic;

#[expect(
    clippy::redundant_pub_crate,
    reason = "root existence replay stays within the crate"
)]
pub(crate) mod splitting;

#[expect(
    clippy::redundant_pub_crate,
    reason = "root existence replay stays within the crate"
)]
pub(crate) mod real_roots;

#[expect(
    clippy::redundant_pub_crate,
    reason = "root existence replay stays within the crate"
)]
pub(crate) mod adjoin;

#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient producers share atomic component staging"
)]
pub(crate) mod materialize;

#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient producers share atomic component staging"
)]
pub(crate) mod rebuild;

#[expect(
    clippy::redundant_pub_crate,
    reason = "coefficient producers share atomic component staging"
)]
pub(crate) mod staging;
