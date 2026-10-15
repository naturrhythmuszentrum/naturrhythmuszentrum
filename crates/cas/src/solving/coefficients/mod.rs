// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed exact coefficient arithmetic and guarded finite specialization.

#![expect(
    dead_code,
    reason = "the independent identity checker consumes the arithmetic interface in the next lane commit"
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

/// Arithmetic availability does not enable an unchecked theorem schema.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Per-subtype capability permits independently green checker slices in this leaf.
pub(crate) const fn schema_availability(_tag: u8) -> super::Availability {
    super::Availability::Unavailable
}

/// Reject every application until this lane supplies its independent typed checker.
pub(crate) fn check(
    _cas: &crate::graph::Cas,
    _draft: &crate::proof_model::ApplicationDraft,
    _premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    budget.work(
        1,
        crate::error::Location::new("unavailable solving checker"),
    )?;
    Err(crate::error::Fault::invalid(
        crate::error::InvalidCode::UnsupportedFragment,
        crate::error::Location::new("unavailable solving checker"),
    ))
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
    reason = "scoped nonzero evidence stays within the crate"
)]
pub(crate) mod nonvanishing;
