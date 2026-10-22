// SPDX-License-Identifier: AGPL-3.0-only
//! Independent replay of finite polynomial orbits under declared permutations.
//!
//! # Checked orbit coverage
//!
//! `orbit.cover` (tag 14) independently checks finite polynomial orbits under declared sort-preserving permutation bijections. Replay compares every action using exact coefficient arithmetic, rejects duplicate normalized members, checks closure and follows a reaching word for every member. The conclusion names the generated group only; full symmetric invariance requires the adjacent-transposition generators. Orbit actions, monomial visits and member peaks consume their separate budgets. Missing members, false tables and unreachable additions refuse atomically, and portable orbit witnesses replay in an empty store.

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

#[expect(
    clippy::redundant_pub_crate,
    reason = "generic orbit production stays within the crate"
)]
pub(crate) mod produce;
pub(crate) use action::permutation;
