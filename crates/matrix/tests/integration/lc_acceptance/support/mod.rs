// SPDX-License-Identifier: AGPL-3.0-only
//! State and owner fixtures shared by acceptance tests and the standalone measurement binary.

pub(super) mod owners;
pub(super) mod state_fixture;

use crate::reading_testing;
use nrz_bindings::readings::{EntryMeaning, algebra::Rational};
use nrz_bindings::{ReferenceDefinition, Scale};
use reading_testing::{TestResult, ledger, meaning};

/// Exact fixed state references match the original spectral declarations without constructing roots.
pub(super) fn references() -> TestResult<[EntryMeaning; 2]> {
    Ok([
        reference("voltage", [2, 1, -3, -1, 0, 0, 0], "5")?,
        reference("current", [0, 0, 0, 1, 0, 0, 0], "1/10")?,
    ])
}

/// Declare a source-free reference from its exact spelling, never from a heap value.
fn reference(name: &str, dimensions: [i64; 7], text: &str) -> TestResult<EntryMeaning> {
    let entry = meaning(name, dimensions)?;
    let reference = ReferenceDefinition::new(
        name,
        entry.descriptor(),
        Scale::Exact(Rational::parse(text, &mut ledger()?)?),
    )?;
    Ok(EntryMeaning::new(
        entry.descriptor().clone(),
        reference,
        None,
        &mut ledger()?,
    )?)
}
