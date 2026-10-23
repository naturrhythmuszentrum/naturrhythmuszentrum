// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered exact partials over declared source coordinates and retained whole-family domains.

mod build;
mod cache;
mod composition;
mod composition_model;
mod key;
mod model;
mod reading_values;
mod storage;

pub use cache::SourceJacobianCache;
pub use composition_model::{ComposedJacobian, JacobianReadings};
pub use key::SourceJacobianKey;
pub use model::{SourceCoordinates, SourceJacobian};

use crate::error::{Fault, InvalidCode, Location};

/// Location for source contracts, construction and retained storage.
const AT: Location = Location::new("source Jacobian");

/// A named contract refusal never substitutes a sampled conclusion.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}

#[cfg(test)]
mod metrics;
