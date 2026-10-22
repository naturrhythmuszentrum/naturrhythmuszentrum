// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic seeds, symmetric elimination and replayable coefficient recipes.
//!
//! # Computed resolvents and request-local reuse
//!
//! The symmetric leaf generates cyclic-character powers up to their computed character order, then partition seeds in restricted-growth-string order. It enumerates full adjacent-transposition orbits and expands only strictly smaller resolvents. Leading-lex subtraction computes every invariant coefficient in elementary symmetric functions, without a formula cache. Derived recipes retain explicit root and parameter images, coefficient maps and map hypotheses; `coeff.identity` independently checks the nested expansion and simultaneous coefficient substitution. Symmetry, expansion and substitution evidence is published together only after replay, with no implied solution coverage.
//!
//! Within one symbolic request, already computed complete orbit/resolvent scratch may be reused only with the identical coefficient algebra, ordered formal-root frame, primitive element and recursion depth. Computed cross-character pairings use the same algebra, frame and primitive checks and the identical enumeration bound. Copies are metered. Every regional identity, selected-root existence, unit obligation and coverage theorem is rebuilt and independently checked. The cache starts empty for each request and contains no stored formula or theorem authority.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the next solving lane consumes these generic producer interfaces"
    )
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "formal polynomial scratch is shared with orbit production"
)]
pub(crate) mod algebra;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the generic scheduler consumes the seed cursor"
)]
pub(crate) mod cursor;
#[expect(
    clippy::redundant_pub_crate,
    reason = "symmetric reductions are unsealed producer proposals"
)]
pub(crate) mod reduction;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the generic scheduler consumes strictly smaller resolvents"
)]
pub(crate) mod resolvent;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the generic scheduler consumes seed generation"
)]
pub(crate) mod seeds;

/// Every seed, subtraction and emission shares the caller's finite ledger.
pub(crate) const AT: crate::error::Location = crate::error::Location::new("symmetric resolvent");

/// Generic production is available; equality still uses the existing coefficient checker.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "derived recipes return only independently replayed evidence"
)]
pub(crate) mod proof;
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof syntax is shared only by derived symmetric recipes"
)]
pub(crate) mod render;
