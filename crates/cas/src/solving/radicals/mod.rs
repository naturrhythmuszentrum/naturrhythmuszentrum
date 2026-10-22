// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic radical proposals and independently replayable finite orbit evidence.
//!
//! # Retained cubic equation inventory
//!
//! The Real one-root cubic producer omits its separate square-radical equality: the checked defining relations supply it. The mixed general cubic retains the original-polynomial equalities for its first two selected generators because separate omission tests still refuse; the last, linear root after monic division needs no emitted equality. Inventory tests pin the removed Real square equation and the two retained mixed cubic equations, and complete mixed answers replay against independently rebuilt original requests (´test:crate:single-root-square-lemma-inventory´), (´test:crate:mixed-cubic-equation-inventory´), (´test:crate:mixed-complete-answer-closure´).

#[expect(
    clippy::redundant_pub_crate,
    reason = "the scheduler consumes unsealed generic proposals"
)]
pub(crate) mod discovery;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the scheduler consumes derived joint constraints"
)]
pub(crate) mod pairing;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the scheduler supplies explicit coefficient substitutions"
)]
pub(crate) mod substitute;

/// One diagnostic location for bounded radical construction.
pub(crate) const AT: crate::error::Location = crate::error::Location::new("radical discovery");

/// Generic production uses existing replay schemas and adds no accepting checker stub.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Commit touched records on success and roll them back on failure without refunding work.
pub(crate) fn staged<T>(
    cas: &mut crate::graph::Cas,
    budget: &mut crate::budget::Budget,
    operation: impl FnOnce(
        &mut crate::graph::Cas,
        &mut crate::budget::Budget,
    ) -> Result<T, crate::error::Fault>,
) -> Result<T, crate::error::Fault> {
    cas.transaction(budget, operation)
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "generic solving construction is shared with the scheduler"
)]
pub(crate) mod construction;

#[expect(
    clippy::redundant_pub_crate,
    reason = "generic solving construction is shared with the scheduler"
)]
pub(crate) mod roots;

#[expect(
    clippy::redundant_pub_crate,
    reason = "generic solving construction is shared with the scheduler"
)]
pub(crate) mod route;

#[expect(
    clippy::redundant_pub_crate,
    reason = "generic solving construction is shared with the scheduler"
)]
pub(crate) mod proof;

mod recovery;

#[expect(
    clippy::redundant_pub_crate,
    reason = "the degree-zero search branch emits its checked identity family"
)]
pub(crate) mod family;

mod audit;

mod formal;
mod formal_base;
mod formal_combination;
mod formal_render;
mod formal_scope;

mod formal_compose;

mod real_choice;

#[expect(
    clippy::redundant_pub_crate,
    reason = "regional consequence regressions share the producer"
)]
pub(crate) mod consequence;

#[expect(
    clippy::redundant_pub_crate,
    reason = "one regional request owns its discovery scratch"
)]
pub(crate) mod cache;

mod continuation;
