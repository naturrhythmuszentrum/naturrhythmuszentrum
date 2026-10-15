// SPDX-License-Identifier: AGPL-3.0-only
//! Independent replay of finite polynomial orbits under declared permutations.

#[expect(
    clippy::redundant_pub_crate,
    reason = "orbit internals stay within the crate"
)]
mod action;
#[expect(
    clippy::redundant_pub_crate,
    reason = "orbit internals stay within the crate"
)]
mod replay;

/// Exact finite orbit certificates have an independent checker.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Only the finite orbit schema is implemented by this leaf.
pub(crate) const fn schema_availability(tag: u8) -> super::Availability {
    if tag == 14 {
        super::Availability::Available
    } else {
        super::Availability::Unavailable
    }
}

/// Reconstruct the exact generated orbit without trusting a producer's enumeration.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &crate::proof_model::ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    replay::check(cas, draft, premises, budget)
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the next solving lane consumes generic orbit production"
    )
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "generic orbit production stays within the crate"
)]
pub(crate) mod produce;
pub(crate) use action::permutation;
