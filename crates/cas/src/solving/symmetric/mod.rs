// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic seeds, symmetric elimination and replayable coefficient recipes.

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
