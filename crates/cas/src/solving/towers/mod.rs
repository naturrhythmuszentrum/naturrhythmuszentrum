// SPDX-License-Identifier: AGPL-3.0-only
//! Finite quotient arithmetic and independently replayed unit/component evidence.

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
    if matches!(tag, 2 | 3) {
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
