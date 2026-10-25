// SPDX-License-Identifier: AGPL-3.0-only
//! Construction cost regressions separate declaration order from proof-header closure.

use super::{TestResult, exact, original, outputs, source};
use nrz_bindings::{
    Budget, Limits,
    readings::{
        CoordinateNormalization, PositiveReciprocalMap,
        algebra::{
            Handle,
            domain::HeaderDraft,
            syntax::{Draft, Environment},
        },
    },
};

/// Attached-before, separate-after and separate-before inventories use unchanged per-operation policies.
pub(crate) fn construction(mode: usize, limits: [u64; 5]) -> TestResult<[u64; 7]> {
    let (mut source, _, _) = original(Limits::default())?;
    let mut declaration = phase(limits[4])?;
    if mode == 2 {
        extra_header(&mut source, &mut declaration)?;
    }
    let mut admission = phase(limits[0])?;
    let context = source::real::context(&mut source, mode == 0, &mut admission)?;
    let mut references = phase(limits[1])?;
    let entries = [
        exact::meaning("voltage", [2, 1, -3, -1, 0, 0, 0], "5", &mut references)?,
        exact::meaning("current", [0, 0, 0, 1, 0, 0, 0], "1/10", &mut references)?,
    ];
    let state = CoordinateNormalization::new(&mut source.cas, &context, &entries, &mut references)?;
    let time = exact::meaning("duration", [0, 0, 1, 0, 0, 0, 0], "1/2000", &mut references)?;
    let mut preparation = phase(limits[2])?;
    let (frame, definition) =
        exact::proposal(&mut source, &context, &state, &time, &mut preparation)?;
    let mut correspondence = phase(limits[3])?;
    let result =
        PositiveReciprocalMap::prove(&mut source.cas, &frame, definition, &mut correspondence)?;
    if mode == 1 {
        extra_header(&mut source, &mut declaration)?;
    }
    let work = [
        admission.usage().work,
        references.usage().work,
        preparation.usage().work,
        correspondence.usage().work,
        declaration.usage().work,
    ];
    Ok([
        work[0],
        work[1],
        work[2],
        work[3],
        work[4],
        u64::try_from(result.witness().exported_byte_len())?,
        work.into_iter().sum(),
    ])
}

/// A fresh declaration header is admitted without changing the original proof's immutable header.
fn extra_header(source: &mut source::Fixture, budget: &mut Budget) -> TestResult<Handle> {
    source::admit(
        &mut source.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations: outputs::inventory(),
            ..Environment::default()
        })),
        budget,
    )
}

/// Each phase keeps the default policy except for its exact caller Work boundary.
fn phase(work: u64) -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work,
        ..Limits::default()
    })?)
}
