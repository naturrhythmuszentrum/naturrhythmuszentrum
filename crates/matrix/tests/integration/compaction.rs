// SPDX-License-Identifier: AGPL-3.0-only
//! Stable mathematical identities and refreshed physical epochs across relocation.

use nrz_matrix::{Extent, Heap, HeapError, HeapLimits, Layout};
use nrz_units::Descriptor;

use crate::testing::{TestResult, bound};

/// Both formats, companions and composed aliases retain their identities and bits on relocation.
/// ´test:integration:compaction-preserves-bundles-and-aliases´
#[test]
fn compaction_preserves_bundles_and_aliases() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = Descriptor::ratio();
    let double = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &quantity,
        &[f64::from_bits(1), f64::MAX],
        &[0.25, 0.5],
    )?;
    let single = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &quantity,
        &[f32::MAX],
        &[f32::from_bits(1)],
    )?;
    let reverse = heap.alias(
        double.value(),
        &Layout::strided(Extent::new(2, 1)?, 2, 1, -1, 0)?,
    )?;
    let repeat = heap.alias(
        reverse,
        &Layout::gather(Extent::new(3, 1)?, 2, &[0, 1, 0], 3)?,
    )?;
    let double_stamp = heap.value(double.value())?.stamp();
    let single_stamp = heap.value(single.value())?.stamp();
    let schema = heap.value(reverse)?.region().schema_id();
    let epoch = heap.value(reverse)?.region().layout_epoch();
    heap.compact()?;
    assert_eq!(heap.value(double.value())?.stamp(), double_stamp);
    assert_eq!(heap.value(single.value())?.stamp(), single_stamp);
    assert_eq!(heap.value(repeat)?.stamp(), double_stamp);
    assert_eq!(heap.value(reverse)?.region().schema_id(), schema);
    assert_eq!(heap.value(reverse)?.region().layout_epoch(), epoch + 1);
    assert_eq!(
        heap.companion(bound(single)?)?.region().layout_epoch(),
        epoch + 1
    );
    assert_eq!(heap.companion(bound(double)?)?.target_stamp(), double_stamp);
    assert_eq!(
        heap.value(repeat)?.region().get(1, 0)?.to_bits(),
        f64::from_bits(1).to_bits()
    );
    assert_eq!(
        heap.value(repeat)?.region().get(2, 0)?.to_bits(),
        f64::MAX.to_bits()
    );
    assert_eq!(
        heap.selected_bound(reverse)?
            .ok_or(HeapError::StaleEvidence)?
            .region()
            .get(0, 0)?
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        heap.companion(bound(single)?)?
            .region()
            .get(0, 0)?
            .to_bits(),
        f32::from_bits(1).to_bits()
    );
    Ok(())
}

/// Relocation accounts for copied centers, bounds and explicit alias maps as one transaction.
/// ´test:integration:compaction-work-limit-is-atomic´
#[test]
fn compaction_work_limit_is_atomic() -> TestResult {
    for limit in [9, 10, 11] {
        let mut heap = Heap::new(HeapLimits::default().with_layout_work(limit))?;
        let base = heap.import_declared_bounds(
            Extent::new(4, 1)?,
            &Descriptor::ratio(),
            &[1.0_f64, 2.0, 3.0, 4.0],
            &[0.5; 4],
        )?;
        let selected = heap.alias(
            base.value(),
            &Layout::gather(Extent::new(2, 1)?, 4, &[3, 1], 2)?,
        )?;
        let stamp = heap.value(selected)?.stamp();
        let epoch = heap.value(selected)?.region().layout_epoch();
        if limit < 10 {
            assert_eq!(heap.compact(), Err(HeapError::WorkLimit));
            assert_eq!(heap.value(selected)?.region().layout_epoch(), epoch);
        } else {
            heap.compact()?;
            assert_eq!(heap.value(selected)?.region().layout_epoch(), epoch + 1);
        }
        assert_eq!(heap.value(selected)?.stamp(), stamp);
        assert_eq!(heap.companion(bound(base)?)?.target_stamp(), stamp);
        assert_eq!(
            heap.value(selected)?.region().get(0, 0)?.to_bits(),
            4.0_f64.to_bits()
        );
    }
    Ok(())
}

/// Empty allocations have no address to relocate and still retain their checked target relations.
/// ´test:integration:empty-compaction-preserves-no-address-contract´
#[test]
fn empty_compaction_preserves_no_address_contract() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(3, 0, 2).with_layout_work(0))?;
    heap.compact()?;
    let base =
        heap.import_declared_bounds(Extent::new(0, 2)?, &Descriptor::ratio(), &[] as &[f64], &[])?;
    let empty = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(3, 0)?, 0, isize::MAX, 0, 0)?,
    )?;
    let stamp = heap.value(empty)?.stamp();
    heap.compact()?;
    assert_eq!(heap.value(empty)?.stamp(), stamp);
    assert_eq!(heap.value(empty)?.region().layout_epoch(), 3);
    assert_eq!(heap.value(empty)?.region().export()?, []);
    assert_eq!(
        heap.selected_bound(empty)?
            .ok_or(HeapError::StaleEvidence)?
            .region()
            .export()?,
        []
    );
    assert!(matches!(
        heap.value(empty)?.region().get(0, 0),
        Err(HeapError::InvalidLayout)
    ));
    heap.retire(base.value())?;
    assert!(matches!(heap.value(empty), Err(HeapError::StaleHandle)));
    Ok(())
}
