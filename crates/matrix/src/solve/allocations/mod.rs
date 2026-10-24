// SPDX-License-Identifier: AGPL-3.0-only
//! A factor retains its original allocating binding independently of its origin execution.

mod buffer;
mod record;
mod run;

pub(crate) use buffer::{Backend, Buffer};
pub use record::{FactorAllocation, FactorAllocationStage, FactorInventory};
pub(crate) use run::Run;
