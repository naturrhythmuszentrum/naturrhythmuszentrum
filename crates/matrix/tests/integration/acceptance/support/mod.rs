// SPDX-License-Identifier: AGPL-3.0-only
//! Committed lifecycle inputs shared by the public tests and server measurement executable.

mod adversarial;
mod capture;
mod model;
mod resistor;
mod restore;
mod shapes;
mod truss;

pub(crate) use adversarial::{edge, shared, stagnation, stale};
pub(crate) use capture::{Case, Parameter, Sample};
pub(crate) use resistor::resistor;
pub(crate) use truss::truss;

/// Fallible public API fixture result; failures never become successful samples.
pub(crate) type ResultOf<T = ()> = Result<T, Box<dyn std::error::Error>>;
