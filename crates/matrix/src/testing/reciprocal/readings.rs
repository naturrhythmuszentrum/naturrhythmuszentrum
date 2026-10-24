// SPDX-License-Identifier: AGPL-3.0-only
//! Complete normalized operator result readings are declared before numerical execution.

use crate::reading_testing::{Fixture, TestResult, ledger};
use crate::test_api::{EntryTicket, ReadingAdapter, ReadingCoverage};
use nrz_bindings::readings::algebra::readings::{AffineMap, SelectionSpec};
use nrz_bindings::{
    Descriptor, ReferenceDefinition,
    readings::{CoordinateFamily, EntryMeaning, Representation, RepresentationDefinition},
};

/// One four-entry ratio reading covers the entire result, including both typed structural zeros.
pub(crate) fn coverage(case: &mut Fixture) -> TestResult<ReadingCoverage<usize>> {
    let descriptor = Descriptor::ratio();
    let reference = ReferenceDefinition::unit("normalized coefficient", &descriptor)?;
    let entry = EntryMeaning::new(descriptor, reference, None, &mut ledger()?)?;
    let entries = vec![entry; 4];
    let family_symbol = case.symbol("b")?;
    let family = CoordinateFamily::new(
        &case.cas,
        &case.context,
        &family_symbol,
        entries.clone(),
        &mut ledger()?,
    )?;
    let selection = SelectionSpec::Block {
        row: 0,
        column: 0,
        rows: 2,
        columns: 2,
    };
    let reading = case.define(
        &family,
        "B",
        selection,
        AffineMap::identity(4, &mut ledger()?)?,
        entries,
    )?;
    let representation = Representation::prove(
        &mut case.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol: family_symbol,
            expansion: AffineMap::identity(4, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    let tickets = (0..4)
        .map(|index| EntryTicket { region: 0, index })
        .collect::<Vec<_>>();
    let adapter = ReadingAdapter::new(representation, tickets.clone(), &mut ledger()?)?;
    Ok(ReadingCoverage::new(
        tickets,
        vec![adapter],
        Vec::new(),
        &mut ledger()?,
    )?)
}
