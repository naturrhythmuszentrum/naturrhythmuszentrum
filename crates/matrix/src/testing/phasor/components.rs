// SPDX-License-Identifier: AGPL-3.0-only
//! Selected scalar-copy proofs shared by phasor acceptance and owner measurements.

use super::reading_testing::{Fixture, TestResult, ledger};
use super::{EntryTicket, ReadingAdapter};
use nrz_bindings::readings::algebra::readings::{AffineMap, EntryIndex, SelectionSpec};
use nrz_bindings::readings::{
    PhysicalReading, ReadingRelation, Representation, RepresentationDefinition,
};

/// Each identity scalar view has its own proof against the specified selected storage entry.
pub(crate) fn components(
    fixture: &mut Fixture,
    whole: &PhysicalReading,
    prefix: &str,
    region: usize,
    adapters: &mut Vec<ReadingAdapter<usize>>,
    relations: &mut Vec<ReadingRelation>,
) -> TestResult {
    for index in 0..4 {
        let key = format!("{prefix}_{index}");
        let reading = fixture.define(
            whole.family(),
            &key,
            SelectionSpec::Entry(EntryIndex {
                row: index % 2,
                column: index / 2,
            }),
            AffineMap::identity(1, &mut ledger()?)?,
            vec![whole.entries()[0].clone()],
        )?;
        let symbol = fixture.symbol(&key)?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(1, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let left = whole.project_selected(&mut fixture.cas, &[index], &mut ledger()?)?;
        let right = reading.project_selected(&mut fixture.cas, &[0], &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut fixture.cas,
            left,
            right,
            &mut ledger()?,
        )?);
        adapters.push(ReadingAdapter::new(
            representation,
            tickets(region, &[u64::from(index)]),
            &mut ledger()?,
        )?);
    }
    Ok(())
}

/// Literal storage positions remain independent from logical result component indices.
pub(crate) fn tickets(region: usize, indices: &[u64]) -> Vec<EntryTicket<usize>> {
    indices
        .iter()
        .map(|&index| EntryTicket { region, index })
        .collect()
}
