// SPDX-License-Identifier: AGPL-3.0-only
//! Compact empty-region DAGs retain checked covers and exact local contradictions.

mod check;
mod facts;
mod leaves;
mod model;
mod trim;
mod wire;

pub(crate) use check::check;
pub use model::{
    EmptinessDag, EmptyEvidence, EmptyFact, EmptyFactor, EmptyFactorIdentity, EmptyLeaf, EmptyNode,
};
pub(crate) use wire::{decode, encode, measure};

use crate::error::{Fault, InvalidCode, Location};

/// Exact emptiness certificate checking and trimming location.
const AT: Location = Location::new("empty region DAG");

/// Unproved, mis-scoped or incomplete emptiness evidence.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}
