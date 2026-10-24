// SPDX-License-Identifier: AGPL-3.0-only
//! Execution leases own shared numerical resources independently of immutable history.

mod lease;
mod record;
mod registry;
mod trace;

pub(crate) use lease::{ErasedFactor, Lease};
pub use record::{ExecutionInventory, SharedFactorAllocation};
pub(crate) use registry::Registry;
pub(crate) use trace::Trace;

#[cfg(test)]
mod tests;
