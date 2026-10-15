// SPDX-License-Identifier: AGPL-3.0-only
//! Public complete-bundle publication and independent owner lifecycles.

use nrz_matrix::{Extent, Heap, HeapError, HeapLimits};
use nrz_units::{Descriptor, Kind, Measure, Role};

use crate::testing::{TestResult, bound, voltage};

/// Exact schema sharing never merges value identities, kinds, roles or heap scopes.
/// ´test:integration:schemas-share-without-merging-values´
#[test]
fn schemas_share_without_merging_values() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(1, 1)?;
    let quantity = voltage()?;
    let first = heap.import_represented(extent, &quantity, &[2.0_f64])?;
    let second = heap.import_represented(extent, &quantity, &[2.0_f64])?;
    let different_kind = Descriptor::new(
        Kind::Named("other".to_owned(), Box::new(quantity.dimension().clone())),
        Measure::coherent(),
        Role::Difference,
    )?;
    let third = heap.import_represented(extent, &different_kind, &[2.0_f64])?;
    let point = Descriptor::new(quantity.kind().clone(), Measure::coherent(), Role::Point)?;
    let fourth = heap.import_represented(extent, &point, &[2.0_f64])?;
    let first_view = heap.value(first.value())?;
    let shared = first_view.region().schema_id();
    assert_eq!(shared, heap.value(second.value())?.region().schema_id());
    assert_ne!(
        first_view.stamp().id(),
        heap.value(second.value())?.stamp().id()
    );
    assert_ne!(shared, heap.value(third.value())?.region().schema_id());
    assert_ne!(shared, heap.value(fourth.value())?.region().schema_id());
    let mut other = Heap::new(HeapLimits::default())?;
    let foreign = other.import_represented(extent, &quantity, &[2.0_f64])?;
    assert_ne!(shared, other.value(foreign.value())?.region().schema_id());
    assert!(matches!(
        heap.value(foreign.value()),
        Err(HeapError::ForeignHandle)
    ));
    Ok(())
}

/// Both precisions coexist, growth preserves old locators, and raw copies own no heap storage.
/// ´test:integration:precision-growth-and-raw-export´
#[test]
fn precision_growth_and_raw_export() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(16, 4, 4))?;
    let extent = Extent::new(2, 1)?;
    let quantity = voltage()?;
    let single = heap.import_declared_bounds(extent, &quantity, &[2.0_f32, 3.0], &[0.125, 0.25])?;
    let double = heap.import_declared_bounds(extent, &quantity, &[2.0_f64, 3.0], &[0.125, 0.25])?;
    let before = heap.value(single.value())?.stamp();
    assert_eq!(
        heap.companion(bound(single)?)?
            .region()
            .get(1, 0)?
            .to_bits(),
        0.25_f32.to_bits()
    );
    assert_eq!(
        heap.value(single.value())?.region().get(1, 0)?.to_bits(),
        3.0_f32.to_bits()
    );
    assert_eq!(
        heap.value(double.value())?.region().get(1, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    let mut raw = heap.value(single.value())?.region().export()?;
    if let Some(entry) = raw.first_mut() {
        *entry = 99.0;
    }
    assert_eq!(
        heap.value(single.value())?.region().get(0, 0)?.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(heap.value(single.value())?.stamp(), before);
    let empty = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        empty.value(single.value()),
        Err(HeapError::ForeignHandle)
    ));
    assert!(matches!(
        empty.companion(bound(double)?),
        Err(HeapError::ForeignHandle)
    ));
    Ok(())
}

/// A failed companion transaction consumes no visible slots, schemas or element allowance.
/// ´test:integration:bundle-capacity-failure-is-atomic´
#[test]
fn bundle_capacity_failure_is_atomic() -> TestResult {
    let extent = Extent::new(2, 1)?;
    let quantity = Descriptor::ratio();
    for limits in [
        HeapLimits::new(1, 4, 2),
        HeapLimits::new(2, 3, 2),
        HeapLimits::new(2, 4, 1),
    ] {
        let mut heap = Heap::new(limits)?;
        assert!(matches!(
            heap.import_declared_bounds(extent, &quantity, &[2.0_f64, 3.0], &[0.125, 0.25]),
            Err(HeapError::Capacity)
        ));
        let admitted = heap.import_represented(extent, &quantity, &[2.0_f64, 3.0])?;
        assert_eq!(
            heap.value(admitted.value())?.region().get(1, 0)?.to_bits(),
            3.0_f64.to_bits()
        );
    }
    for capacity in [4, 5] {
        let mut heap = Heap::new(HeapLimits::new(2, capacity, 2))?;
        let admitted =
            heap.import_declared_bounds(extent, &quantity, &[2.0_f64, 3.0], &[0.125, 0.25])?;
        assert_eq!(
            heap.companion(bound(admitted)?)?
                .region()
                .get(1, 0)?
                .to_bits(),
            0.25_f64.to_bits()
        );
    }
    Ok(())
}

/// Growing a precision's slab table never changes an earlier descriptor's interpretation.
/// ´test:integration:slab-growth-preserves-existing-regions´
#[test]
fn slab_growth_preserves_existing_regions() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(64, 64, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let first = heap.import_declared_bounds(extent, &quantity, &[7.0_f64], &[0.5])?;
    let stamp = heap.value(first.value())?.stamp();
    for _ in 0..40 {
        heap.import_represented(extent, &quantity, &[11.0_f64])?;
    }
    assert_eq!(heap.value(first.value())?.stamp(), stamp);
    assert_eq!(
        heap.value(first.value())?.region().get(0, 0)?.to_bits(),
        7.0_f64.to_bits()
    );
    assert_eq!(
        heap.companion(bound(first)?)?.region().get(0, 0)?.to_bits(),
        0.5_f64.to_bits()
    );
    Ok(())
}
