// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-base evidence invalidation after selected scratch publication.

use nrz_bindings::Descriptor;
use nrz_matrix::{Coverage, Extent, Heap, HeapError, HeapLimits, Layout};

use crate::testing::{TestResult, bound};

/// A selected write advances every base alias and its bound targets, including disjoint selections.
/// ´test:integration:selected-write-invalidates-the-whole-base´
#[test]
fn selected_write_invalidates_the_whole_base() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_declared_bounds(
        Extent::new(3, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 3.0],
        &[0.125, 0.25, 0.5],
    )?;
    let other =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[7.0_f64], &[1.0])?;
    let original = heap.value(base.value())?.stamp();
    let unrelated = heap.value(other.value())?.stamp();
    let first = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 3, 0, 0, 0)?,
    )?;
    let last = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 3, 2, 0, 0)?,
    )?;
    let repeated = heap.alias(first, &Layout::gather(Extent::new(2, 1)?, 1, &[0, 0], 2)?)?;
    let packed = heap.value(base.value())?.region().pack()?;
    let changed = heap.replace_declared_bounds(first, original.version(), &[5.0], &[2.0])?;
    assert_eq!(changed.value(), first);
    let current = heap.value(first)?.stamp();
    assert_eq!(current.id(), original.id());
    assert_eq!(current.version().number(), original.version().number() + 1);
    assert_eq!(heap.value(last)?.stamp(), current);
    assert_eq!(heap.value(repeated)?.stamp(), current);
    assert_eq!(heap.value(base.value())?.stamp(), current);
    assert_eq!(
        heap.value(repeated)?.region().get(1, 0)?.to_bits(),
        5.0_f64.to_bits()
    );
    assert_eq!(
        heap.value(last)?.region().get(0, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(
        heap.selected_bound(last)?
            .ok_or(HeapError::StaleEvidence)?
            .region()
            .get(0, 0)?
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(heap.companion(bound(changed)?)?.target_stamp(), current);
    assert_eq!(heap.companion(bound(changed)?)?.target(), base.value());
    assert!(matches!(
        heap.companion(bound(base)?),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(heap.value(other.value())?.stamp(), unrelated);
    assert_eq!(heap.companion(bound(other)?)?.target_stamp(), unrelated);
    assert_eq!(packed.get(0, 0)?.to_bits(), 1.0_f64.to_bits());
    Ok(())
}

/// Binary32 permutation scatter follows logical order and reuses the same base identity.
/// ´test:integration:binary32-permutation-scatter-keeps-base-identity´
#[test]
fn binary32_permutation_scatter_keeps_base_identity() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[1.0_f32, 2.0, 3.0, 4.0],
    )?;
    let permutation = heap.alias(
        base.value(),
        &Layout::gather(Extent::new(2, 2)?, 4, &[3, 1, 2, 0], 4)?,
    )?;
    let before = heap.value(base.value())?.stamp();
    heap.replace_represented(
        permutation,
        before.version(),
        &[f32::MAX, f32::from_bits(1), -f32::MAX, -0.0],
    )?;
    let after = heap.value(base.value())?;
    assert_eq!(after.stamp().id(), before.id());
    assert_eq!(
        after
            .region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        [0.0_f32, f32::from_bits(1), -f32::MAX, f32::MAX].map(f32::to_bits)
    );
    heap.compact()?;
    let current = heap.value(permutation)?.stamp();
    heap.replace_represented(permutation, current.version(), &[4.0, 3.0, 2.0, 1.0])?;
    assert_eq!(
        heap.value(base.value())?
            .region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        [1.0_f32, 3.0, 2.0, 4.0].map(f32::to_bits)
    );
    heap.retire(base.value())?;
    assert!(matches!(
        heap.value(permutation),
        Err(HeapError::StaleHandle)
    ));
    Ok(())
}

/// Explicit represented publication through an alias reduces every alias's declared coverage.
/// ´test:integration:selected-represented-write-reduces-base-coverage´
#[test]
fn selected_represented_write_reduces_base_coverage() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0],
        &[0.5; 2],
    )?;
    let selected = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 2, 1, 0, 0)?,
    )?;
    let stamp = heap.value(base.value())?.stamp();
    let represented = heap.replace_represented(selected, stamp.version(), &[9.0])?;
    assert!(represented.bound().is_none());
    for handle in [base.value(), selected] {
        assert_eq!(heap.value(handle)?.coverage(), Coverage::RepresentedInputs);
        assert_eq!(heap.value(handle)?.stamp().version().number(), 2);
        assert!(heap.selected_bound(handle)?.is_none());
    }
    assert_eq!(
        heap.value(base.value())?.region().get(0, 0)?.to_bits(),
        1.0_f64.to_bits()
    );
    assert!(matches!(
        heap.companion(bound(base)?),
        Err(HeapError::StaleHandle)
    ));
    Ok(())
}
