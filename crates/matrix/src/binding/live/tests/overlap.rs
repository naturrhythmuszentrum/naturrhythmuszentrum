// SPDX-License-Identifier: AGPL-3.0-only
//! Partially overlapping readers use exact intersection equations rather than matching schemas.

use super::*;
use nrz_bindings::readings::CoordinateFamily;

/// Two length-two windows cover a three-entry region and share exactly one coordinate.
fn windows(wrong_order: bool) -> TestResult<Plan> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(3, 1)),
        ("x", matrix(2, 1)),
        ("y", matrix(2, 1)),
        ("sx", matrix(2, 1)),
        ("sy", matrix(2, 1)),
    ])?;
    let meaning = crate::binding::adapters::fixtures::meaning("signal", [0; 7])?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![meaning.clone(); 3],
        &mut ledger()?,
    )?;
    let mut adapters = Vec::new();
    let mut readings = Vec::new();
    for (row, name, representation) in [(0, "x", "sx"), (1, "y", "sy")] {
        let reading = fixture.define(
            &family,
            name,
            SelectionSpec::Block {
                row,
                column: 0,
                rows: 2,
                columns: 1,
            },
            AffineMap::identity(2, &mut ledger()?)?,
            vec![meaning.clone(); 2],
        )?;
        let symbol = fixture.symbol(representation)?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(2, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let indices = if wrong_order && row == 1 {
            vec![0, 2]
        } else {
            vec![u64::from(row), u64::from(row + 1)]
        };
        let entries = indices
            .into_iter()
            .map(|index| EntryTicket { region: 0, index })
            .collect();
        adapters.push(ReadingAdapter::new(
            representation,
            entries,
            &mut ledger()?,
        )?);
        readings.push(reading);
    }
    let left = readings[0].project_selected(&mut fixture.cas, &[1], &mut ledger()?)?;
    let right = readings[1].project_selected(&mut fixture.cas, &[0], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut fixture.cas, left, right, &mut ledger()?)?;
    let universe = (0..3)
        .map(|index| EntryTicket { region: 0, index })
        .collect();
    let coverage = ReadingCoverage::new(universe, adapters, vec![relation], &mut ledger()?)?;
    Ok((Rc::new(fixture.cas), coverage))
}

/// Neither reader contains the other; a shared-coordinate write invalidates both exact views.
/// ´test:unit:live-partial-overlap-requires-exact-intersection´
#[test]
fn live_partial_overlap_requires_exact_intersection() -> TestResult {
    assert!(windows(true).is_err());
    for selected in [0, 1] {
        let (graph, coverage) = windows(false)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let regions = [ReadingRegion {
            extent: Extent::new(3, 1)?,
            values: vec![1.0_f64, 2.0, 3.0],
        }];
        let imported = heap.import_readings(
            graph,
            coverage,
            Vec::new(),
            &regions,
            DependencyPolicy::Entries,
            &mut ledger()?,
        )?;
        assert_eq!(
            heap.reading_entries(&imported.bindings()[0])?,
            vec![1.0, 2.0]
        );
        assert_eq!(
            heap.reading_entries(&imported.bindings()[1])?,
            vec![2.0, 3.0]
        );
        heap.replace_reading_entries(&imported.bindings()[selected], &[4.0, 5.0], &mut ledger()?)?;
        for reading in imported.bindings() {
            assert_eq!(reading.state(), SymbolState::Unavailable);
            assert!(heap.reading_entries(reading).is_err());
        }
    }
    Ok(())
}

/// Layout, shape identity and entry revisions are independent captured obligations.
/// ´test:unit:live-reading-refuses-layout-schema-and-version-mutations´
#[test]
fn live_reading_refuses_layout_schema_and_version_mutations() -> TestResult {
    for mutation in 0..3 {
        let (mut heap, imported) = imported::<f64>(DependencyPolicy::Entries)?;
        let reading = &imported.bindings()[0];
        let key = reading.group.tickets[0].key;
        assert!(heap.reading_entries(reading).is_ok());
        if mutation == 2 {
            heap.symbols.cover(key)?.versions[0].set(2);
        } else {
            for (actual, record) in heap.descriptors.iter_mut() {
                if actual == key {
                    if mutation == 0 {
                        record.layout = crate::Layout::contiguous(Extent::new(1, 2)?)?;
                    } else {
                        record.schema += 1;
                    }
                }
            }
        }
        assert!(matches!(
            heap.reading_entries(reading),
            Err(LiveBindingError::Heap(HeapError::StaleEvidence))
        ));
    }
    Ok(())
}
