// SPDX-License-Identifier: AGPL-3.0-only
//! Example-only clocks reuse each narrow acceptance fixture exactly once.

#[path = "../../tests/integration/phasor_statistics/blocks.rs"]
mod blocks;
#[path = "../../src/testing/phasor/components.rs"]
mod copies;
#[path = "../../tests/integration/phasor_statistics/fixture.rs"]
mod fixture;
mod measurements;
#[path = "../../tests/integration/phasor_statistics/parameters.rs"]
mod parameters;
#[path = "../../src/testing/phasor_solve/policy.rs"]
mod resources;

use crate::reading_testing;
use copies::{components, tickets};
use nrz_matrix::{EntryTicket, ReadingAdapter};
use resources::{budget, policy};

/// One operation's observed clock and deterministic accounting; bytes retain their named scope.
#[derive(Debug)]
pub(crate) struct Observation {
    /// Stable phase name, independent of heap or execution identifiers.
    pub(crate) phase: String,
    /// Charged algorithmic work for this phase.
    pub(crate) work: u64,
    /// Observed elapsed seconds, without a performance threshold.
    pub(crate) seconds: f64,
    /// Explicit payload, wire and retained-allowance annotations for this phase.
    pub(crate) accounting: String,
}

/// Measure one owner, keeping observation output out of the shared test support.
pub(super) use measurements::measure;
