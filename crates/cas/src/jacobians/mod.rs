// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered exact partials over declared source coordinates and retained whole-family domains.

mod build;
mod key;
mod model;
mod storage;

pub use key::SourceJacobianKey;
pub use model::{SourceCoordinates, SourceJacobian};

use crate::error::{Fault, InvalidCode, Location};

/// Location for source contracts, construction and retained storage.
const AT: Location = Location::new("source Jacobian");

/// A named contract refusal never substitutes a sampled conclusion.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}
