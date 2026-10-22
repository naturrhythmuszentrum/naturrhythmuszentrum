// SPDX-License-Identifier: AGPL-3.0-only
//! Base identities, companion entry selections and owned packing lifecycles.

use nrz_bindings::Descriptor;
use nrz_matrix::{Extent, Heap, HeapError, HeapLimits, Layout};

use crate::testing::{TestResult, bound};

/// Nested views compose column-order maps for both centers and their bound entries.
/// ´test:integration:nested-aliases-select-centers-and-bounds´
#[test]
fn nested_aliases_select_centers_and_bounds() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_declared_bounds(
        Extent::new(2, 3)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0],
        &[0.125, 0.25, 0.5, 1.0, 2.0, 4.0],
    )?;
    let stamp = heap.value(base.value())?.stamp();
    let reverse = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(2, 3)?, 6, 5, -1, -2)?,
    )?;
    let nested = heap.alias(
        reverse,
        &Layout::gather(Extent::new(3, 1)?, 6, &[4, 1, 4], 3)?,
    )?;
    assert_eq!(heap.value(nested)?.stamp(), stamp);
    assert_eq!(
        heap.value(nested)?
            .region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        [2.0_f64, 5.0, 2.0].map(f64::to_bits)
    );
    let selected = heap
        .selected_bound(nested)?
        .ok_or(HeapError::StaleEvidence)?;
    assert_eq!(selected.target(), base.value());
    assert_eq!(selected.target_stamp(), stamp);
    assert_eq!(selected.region().extent(), Extent::new(3, 1)?);
    assert_eq!(
        selected
            .region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        [0.25_f64, 2.0, 0.25].map(f64::to_bits)
    );
    assert_eq!(selected.region().layout().index(1, 0)?, 4);
    let packed = heap.value(reverse)?.region().pack()?;
    heap.retire(reverse)?;
    assert_eq!(heap.value(nested)?.stamp(), stamp);
    heap.replace_declared_bounds(
        base.value(),
        stamp.version(),
        &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0],
        &[0.5; 6],
    )?;
    assert_eq!(heap.value(nested)?.stamp().version().number(), 2);
    assert_eq!(
        heap.value(nested)?.region().get(1, 0)?.to_bits(),
        11.0_f64.to_bits()
    );
    assert_eq!(
        heap.selected_bound(nested)?
            .ok_or(HeapError::StaleEvidence)?
            .region()
            .get(1, 0)?
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(packed.get(0, 0)?.to_bits(), 6.0_f64.to_bits());
    assert_eq!(packed.get(1, 2)?.to_bits(), 1.0_f64.to_bits());
    assert!(matches!(
        heap.companion(bound(base)?),
        Err(HeapError::StaleHandle)
    ));
    heap.retire(base.value())?;
    assert!(matches!(heap.value(nested), Err(HeapError::StaleHandle)));
    let fresh = heap.import_represented(Extent::new(2, 3)?, &Descriptor::ratio(), &[1.0_f64; 6])?;
    assert_ne!(heap.value(fresh.value())?.stamp().id(), stamp.id());
    assert!(matches!(heap.value(nested), Err(HeapError::StaleHandle)));
    Ok(())
}

/// Binary32 packing preserves subnormal/extreme bits and admits transposes and empty maps.
/// ´test:integration:binary32-mapped-packing-is-an-owned-copy´
#[test]
fn binary32_mapped_packing_is_an_owned_copy() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let data = [f32::from_bits(1), f32::MAX, -f32::MAX, 0.0, 2.0, 3.0];
    let base = heap.import_represented(Extent::new(2, 3)?, &Descriptor::ratio(), &data)?;
    let transpose = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(3, 2)?, 6, 0, 2, 1)?,
    )?;
    let packed = heap.value(transpose)?.region().pack()?;
    assert_eq!(packed.extent(), Extent::new(3, 2)?);
    let expected = [data[0], data[2], data[4], data[1], data[3], data[5]];
    for (logical, value) in expected.iter().enumerate() {
        assert_eq!(
            packed.get(logical % 3, logical / 3)?.to_bits(),
            value.to_bits()
        );
    }
    let empty = heap.alias(transpose, &Layout::gather(Extent::new(0, 2)?, 6, &[], 0)?)?;
    assert_eq!(heap.value(empty)?.region().export()?, []);
    assert!(heap.selected_bound(empty)?.is_none());
    assert!(matches!(
        heap.value(empty)?.region().pack()?.get(0, 0),
        Err(HeapError::InvalidLayout)
    ));
    heap.retire(base.value())?;
    assert_eq!(packed.get(0, 0)?.to_bits(), f32::from_bits(1).to_bits());
    assert!(matches!(heap.value(transpose), Err(HeapError::StaleHandle)));
    Ok(())
}

/// General view composition refuses insufficient enumeration work without changing identity.
/// ´test:integration:composition-limit-and-foreign-alias-refusals´
#[test]
fn composition_limit_and_foreign_alias_refusals() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default().with_layout_work(2))?;
    let mut other = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(3, 1)?;
    let original = heap.import_represented(extent, &Descriptor::ratio(), &[1.0_f64, 2.0, 3.0])?;
    let foreign = other.import_represented(extent, &Descriptor::ratio(), &[1.0_f64; 3])?;
    let reverse = Layout::strided(extent, 3, 2, -1, 0)?;
    assert!(matches!(
        heap.alias(foreign.value(), &reverse),
        Err(HeapError::ForeignHandle)
    ));
    let first = heap.alias(original.value(), &reverse)?;
    assert!(matches!(
        heap.alias(first, &reverse),
        Err(HeapError::WorkLimit)
    ));
    let short = Layout::gather(Extent::new(2, 1)?, 3, &[2, 0], 2)?;
    let nested = heap.alias(first, &short)?;
    assert_eq!(
        heap.value(nested)?.stamp(),
        heap.value(original.value())?.stamp()
    );
    assert_eq!(
        heap.value(nested)?.region().get(0, 0)?.to_bits(),
        1.0_f64.to_bits()
    );
    Ok(())
}
