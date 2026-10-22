// SPDX-License-Identifier: AGPL-3.0-only
//! Exact replacement refusals and failure atomicity.

use nrz_bindings::Descriptor;
use nrz_matrix::{Coverage, Extent, Heap, HeapError, HeapLimits};

use crate::testing::{TestResult, bound};

/// Invalid replacement payloads and stale expected versions leave the current bundle intact.
/// ´test:integration:replacement-refusals-preserve-current-bundle´
#[test]
fn replacement_refusals_preserve_current_bundle() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let original =
        heap.import_declared_bounds(extent, &Descriptor::ratio(), &[2.0_f64], &[0.25])?;
    let stamp = heap.value(original.value())?.stamp();
    let cases = [
        (
            &[f64::NAN][..],
            &[0.5][..],
            HeapError::NonFinite { index: 0 },
        ),
        (
            &[3.0][..],
            &[-1.0][..],
            HeapError::InvalidBound { index: 0 },
        ),
        (&[][..], &[0.5][..], HeapError::LengthMismatch),
        (&[3.0][..], &[][..], HeapError::LengthMismatch),
    ];
    for (centers, radii, expected) in cases {
        assert_eq!(
            heap.replace_declared_bounds(original.value(), stamp.version(), centers, radii)
                .map(|_| ()),
            Err(expected),
        );
        assert_eq!(heap.value(original.value())?.stamp(), stamp);
        assert_eq!(
            heap.value(original.value())?.region().get(0, 0)?.to_bits(),
            2.0_f64.to_bits()
        );
        assert_eq!(
            heap.companion(bound(original)?)?
                .region()
                .get(0, 0)?
                .to_bits(),
            0.25_f64.to_bits()
        );
    }
    let fresh = heap.replace_declared_bounds(original.value(), stamp.version(), &[3.0], &[0.5])?;
    let current = heap.value(fresh.value())?.stamp();
    assert!(
        matches!(heap.replace_represented(original.value(), stamp.version(), &[4.0]), Err(HeapError::VersionMismatch { expected, actual }) if expected == stamp.version() && actual == current.version())
    );
    assert_eq!(heap.value(fresh.value())?.stamp(), current);
    assert_eq!(heap.companion(bound(fresh)?)?.target_stamp(), current);
    Ok(())
}

/// Companion addition checks every capacity before changing the surviving occupation.
/// ´test:integration:replacement-capacity-refusals-are-atomic´
#[test]
fn replacement_capacity_refusals_are_atomic() -> TestResult {
    for limits in [
        HeapLimits::new(1, 2, 2),
        HeapLimits::new(2, 1, 2),
        HeapLimits::new(2, 2, 1),
    ] {
        let mut heap = Heap::new(limits)?;
        let original =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let stamp = heap.value(original.value())?.stamp();
        assert!(matches!(
            heap.replace_declared_bounds(original.value(), stamp.version(), &[3.0], &[0.5]),
            Err(HeapError::Capacity)
        ));
        let current = heap.value(original.value())?;
        assert_eq!(current.stamp(), stamp);
        assert_eq!(current.coverage(), Coverage::RepresentedInputs);
        assert_eq!(current.region().get(0, 0)?.to_bits(), 2.0_f64.to_bits());
        let replaced = heap.replace_represented(original.value(), stamp.version(), &[4.0])?;
        assert_eq!(heap.value(replaced.value())?.stamp().version().number(), 2);
    }
    Ok(())
}

/// A foreign or retired locator cannot mutate another live occupation with the same slot index.
/// ´test:integration:foreign-and-stale-mutations-refuse´
#[test]
fn foreign_and_stale_mutations_refuse() -> TestResult {
    let mut owner = Heap::new(HeapLimits::new(1, 1, 1))?;
    let mut other = Heap::new(HeapLimits::new(1, 1, 1))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let local = owner.import_represented(extent, &quantity, &[2.0_f64])?;
    let foreign = other.import_represented(extent, &quantity, &[3.0_f64])?;
    let version = owner.value(local.value())?.stamp().version();
    assert!(matches!(
        owner.retire(foreign.value()),
        Err(HeapError::ForeignHandle)
    ));
    assert!(matches!(
        owner.replace_represented(foreign.value(), version, &[4.0]),
        Err(HeapError::ForeignHandle)
    ));
    owner.retire(local.value())?;
    let fresh = owner.import_represented(extent, &quantity, &[5.0_f64])?;
    let stamp = owner.value(fresh.value())?.stamp();
    assert!(matches!(
        owner.replace_represented(local.value(), version, &[4.0]),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        owner.retire(local.value()),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(owner.value(fresh.value())?.stamp(), stamp);
    assert_eq!(
        owner.value(fresh.value())?.region().get(0, 0)?.to_bits(),
        5.0_f64.to_bits()
    );
    assert_eq!(
        other.value(foreign.value())?.region().get(0, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    Ok(())
}
