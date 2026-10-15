// SPDX-License-Identifier: AGPL-3.0-only
//! Versioned replacement and retirement through the public heap boundary.

use nrz_matrix::{Coverage, Extent, Heap, HeapError, HeapLimits};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

use crate::testing::{TestResult, bound};

/// Reuse a full two-slot bundle while preserving stale-handle refusal in either format.
fn retirement<P: Scalar + 'static>() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let original = heap.import_declared_bounds(extent, &quantity, &[P::ONE], &[P::ZERO])?;
    let original_bound = bound(original)?;
    let stamp = heap.value(original.value())?.stamp();
    let schema = heap.value(original.value())?.region().schema_id();
    heap.retire(original.value())?;
    assert!(matches!(
        heap.value(original.value()),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        heap.companion(original_bound),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        heap.retire(original.value()),
        Err(HeapError::StaleHandle)
    ));
    let fresh = heap.import_declared_bounds(extent, &quantity, &[P::ONE], &[P::ZERO])?;
    assert_ne!(fresh.value(), original.value());
    assert_ne!(bound(fresh)?, original_bound);
    assert_ne!(heap.value(fresh.value())?.stamp().id(), stamp.id());
    assert_eq!(heap.value(fresh.value())?.stamp().version().number(), 1);
    assert_eq!(heap.value(fresh.value())?.region().schema_id(), schema);
    assert!(matches!(
        heap.value(original.value()),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        heap.companion(original_bound),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(heap.companion(bound(fresh)?)?.target(), fresh.value());
    Ok(())
}

/// Binary32 retirement releases complete bundles and cannot revive old generations.
/// ´test:integration:retire-and-reuse-binary32´
#[test]
fn retire_and_reuse_binary32() -> TestResult {
    retirement::<f32>()
}

/// Binary64 retirement has the same identity and capacity contract.
/// ´test:integration:retire-and-reuse-binary64´
#[test]
fn retire_and_reuse_binary64() -> TestResult {
    retirement::<f64>()
}

/// Replacement keeps the value occupation, advances exactly once, and retires old bounds.
/// ´test:integration:replacement-advances-value-and-bound-targets´
#[test]
fn replacement_advances_value_and_bound_targets() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 4, 2))?;
    let extent = Extent::new(2, 1)?;
    let quantity = Descriptor::ratio();
    let original =
        heap.import_declared_bounds(extent, &quantity, &[2.0_f64, 3.0], &[0.125, 0.25])?;
    let copied_handle = original.value();
    let stamp = heap.value(copied_handle)?.stamp();
    let raw = heap.value(copied_handle)?.region().export()?;
    let updated =
        heap.replace_declared_bounds(copied_handle, stamp.version(), &[5.0, 7.0], &[0.5, 1.0])?;
    let current = heap.value(copied_handle)?;
    assert_eq!(updated.value(), copied_handle);
    assert_eq!(current.stamp().id(), stamp.id());
    assert_eq!(
        current.stamp().version().number(),
        stamp.version().number() + 1
    );
    assert_eq!(current.region().get(0, 0)?.to_bits(), 5.0_f64.to_bits());
    assert_eq!(current.region().get(1, 0)?.to_bits(), 7.0_f64.to_bits());
    assert_eq!(
        raw.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        [2.0_f64, 3.0].map(f64::to_bits)
    );
    assert!(matches!(
        heap.companion(bound(original)?),
        Err(HeapError::StaleHandle)
    ));
    let companion = heap.companion(bound(updated)?)?;
    assert_eq!(companion.target_stamp(), current.stamp());
    assert_eq!(companion.region().get(1, 0)?.to_bits(), 1.0_f64.to_bits());
    Ok(())
}

/// A changed declaration with identical bits advances the base version; unrelated values survive.
/// ´test:integration:bound-declaration-change-invalidates-evidence´
#[test]
fn bound_declaration_change_invalidates_evidence() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(4, 4, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let first = heap.import_declared_bounds(extent, &quantity, &[1.0_f64], &[0.125])?;
    let other = heap.import_declared_bounds(extent, &quantity, &[7.0_f64], &[0.25])?;
    let before = heap.value(first.value())?.stamp();
    let unrelated = heap.value(other.value())?.stamp();
    let changed = heap.replace_declared_bounds(first.value(), before.version(), &[1.0], &[0.5])?;
    let current = heap.value(changed.value())?.stamp();
    assert_eq!(current.id(), before.id());
    assert_eq!(current.version().number(), 2);
    assert!(matches!(
        heap.companion(bound(first)?),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(heap.companion(bound(changed)?)?.target_stamp(), current);
    assert_eq!(heap.value(other.value())?.stamp(), unrelated);
    assert_eq!(heap.companion(bound(other)?)?.target_stamp(), unrelated);
    heap.retire(changed.value())?;
    assert_eq!(heap.value(other.value())?.stamp(), unrelated);
    assert_eq!(
        heap.companion(bound(other)?)?.region().get(0, 0)?.to_bits(),
        0.25_f64.to_bits()
    );
    Ok(())
}

/// Explicit represented replacement reduces coverage, then fresh bounds restore declared coverage.
/// ´test:integration:replacement-explicitly-changes-coverage´
#[test]
fn replacement_explicitly_changes_coverage() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let original = heap.import_declared_bounds(extent, &quantity, &[2.0_f32], &[0.25])?;
    let stamp = heap.value(original.value())?.stamp();
    let represented = heap.replace_represented(original.value(), stamp.version(), &[2.0])?;
    assert!(represented.bound().is_none());
    let current = heap.value(represented.value())?;
    assert_eq!(current.coverage(), Coverage::RepresentedInputs);
    assert_eq!(current.stamp().version().number(), 2);
    assert!(matches!(
        heap.companion(bound(original)?),
        Err(HeapError::StaleHandle)
    ));
    let version = current.stamp().version();
    let restored = heap.replace_declared_bounds(original.value(), version, &[3.0], &[0.5])?;
    let value = heap.value(restored.value())?;
    assert_eq!(value.coverage(), Coverage::DeclaredBounds);
    assert_eq!(value.stamp().version().number(), 3);
    assert_eq!(value.region().get(0, 0)?.to_bits(), 3.0_f32.to_bits());
    assert_eq!(
        heap.companion(bound(restored)?)?
            .region()
            .get(0, 0)?
            .to_bits(),
        0.5_f32.to_bits()
    );
    Ok(())
}

/// Slot reuse may change category or precision without reviving any previous locator.
/// ´test:integration:reused-slot-does-not-reinterpret-old-precision´
#[test]
fn reused_slot_does_not_reinterpret_old_precision() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let old = heap.import_declared_bounds(extent, &quantity, &[1.0_f64], &[0.5])?;
    heap.retire(old.value())?;
    let single = heap.import_represented(extent, &quantity, &[2.0_f32])?;
    let another = heap.import_represented(extent, &quantity, &[3.0_f32])?;
    assert_eq!(
        heap.value(single.value())?.region().get(0, 0)?.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(
        heap.value(another.value())?.region().get(0, 0)?.to_bits(),
        3.0_f32.to_bits()
    );
    assert!(matches!(
        heap.value(old.value()),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        heap.companion(bound(old)?),
        Err(HeapError::StaleHandle)
    ));
    Ok(())
}
