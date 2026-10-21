// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed exact coefficient arithmetic and guarded finite specialization.

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
