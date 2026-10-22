// SPDX-License-Identifier: AGPL-3.0-only
//! Public injectivity proofs and selected-publication refusal boundaries.

use nrz_bindings::Descriptor;
use nrz_matrix::{Extent, Heap, HeapError, HeapLimits, Layout};

use crate::testing::{TestResult, bound};

/// Disjoint dense maps need no element enumeration, including reversed contiguous selections.
/// ´test:integration:dense-output-ranges-prove-disjointness´
#[test]
fn dense_output_ranges_prove_disjointness() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default().with_layout_work(0))?;
    let base = heap.import_represented(Extent::new(6, 1)?, &Descriptor::ratio(), &[1.0_f64; 6])?;
    let left = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(2, 1)?, 6, 0, 1, 0)?,
    )?;
    let right = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(2, 2)?, 6, 5, -1, -2)?,
    )?;
    heap.validate_outputs(&[left, right])?;
    assert_eq!(
        heap.validate_outputs(&[left, left]),
        Err(HeapError::OutputOverlap)
    );
    assert_eq!(
        heap.validate_outputs(&[base.value(), right]),
        Err(HeapError::OutputOverlap)
    );
    let other = heap.import_represented(Extent::new(6, 1)?, &Descriptor::ratio(), &[2.0_f64; 6])?;
    heap.validate_outputs(&[base.value(), other.value()])?;
    let empty = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(0, 2)?, 6, 99, 0, 0)?,
    )?;
    heap.validate_outputs(&[empty, empty, left])?;
    heap.validate_outputs::<f64>(&[])?;
    Ok(())
}

/// Overlapping address boxes alone prove nothing; sorted destinations resolve interleaving.
/// ´test:integration:interleaved-output-proof-honors-total-work´
#[test]
fn interleaved_output_proof_honors_total_work() -> TestResult {
    for limit in [5, 6, 7] {
        let mut heap = Heap::new(HeapLimits::default().with_layout_work(limit))?;
        let base =
            heap.import_represented(Extent::new(6, 1)?, &Descriptor::ratio(), &[1.0_f64; 6])?;
        let even = heap.alias(
            base.value(),
            &Layout::strided(Extent::new(3, 1)?, 6, 0, 2, 0)?,
        )?;
        let odd = heap.alias(
            base.value(),
            &Layout::strided(Extent::new(3, 1)?, 6, 1, 2, 0)?,
        )?;
        let expected = if limit < 6 {
            Err(HeapError::WorkLimit)
        } else {
            Ok(())
        };
        assert_eq!(heap.validate_outputs(&[even, odd]), expected);
        if limit >= 6 {
            assert_eq!(
                heap.validate_outputs(&[even, even]),
                Err(HeapError::OutputOverlap)
            );
        }
    }
    Ok(())
}

/// Repeated gathers, broadcasts and cross-output intersections cannot authorize a scatter.
/// ´test:integration:repeated-output-destinations-refuse´
#[test]
fn repeated_output_destinations_refuse() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_represented(
        Extent::new(3, 1)?,
        &Descriptor::ratio(),
        &[1.0_f32, 2.0, 3.0],
    )?;
    let gathered = heap.alias(
        base.value(),
        &Layout::gather(Extent::new(3, 1)?, 3, &[2, 0, 2], 3)?,
    )?;
    let broadcast = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(2, 2)?, 3, 1, 0, 0)?,
    )?;
    for repeated in [gathered, broadcast] {
        assert_eq!(
            heap.validate_outputs(std::slice::from_ref(&repeated)),
            Err(HeapError::OutputOverlap)
        );
    }
    let permutation = heap.alias(
        base.value(),
        &Layout::gather(Extent::new(3, 1)?, 3, &[2, 0, 1], 3)?,
    )?;
    heap.validate_outputs(std::slice::from_ref(&permutation))?;
    assert_eq!(
        heap.validate_outputs(&[base.value(), permutation]),
        Err(HeapError::OutputOverlap)
    );
    let stamp = heap.value(base.value())?.stamp();
    assert!(matches!(
        heap.replace_represented(gathered, stamp.version(), &[7.0; 3]),
        Err(HeapError::OutputOverlap)
    ));
    assert_eq!(heap.value(base.value())?.stamp(), stamp);
    assert_eq!(
        heap.value(base.value())?.region().get(2, 0)?.to_bits(),
        3.0_f32.to_bits()
    );
    Ok(())
}

/// Failed bounds, payload lengths and expected versions leave every alias and bound current.
/// ´test:integration:selected-replacement-refusals-are-atomic´
#[test]
fn selected_replacement_refusals_are_atomic() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0],
        &[0.125, 0.25],
    )?;
    let selected = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 2, 1, 0, 0)?,
    )?;
    let stamp = heap.value(selected)?.stamp();
    assert!(matches!(
        heap.replace_declared_bounds(selected, stamp.version(), &[3.0], &[-1.0]),
        Err(HeapError::InvalidBound { index: 0 })
    ));
    assert!(matches!(
        heap.replace_represented(selected, stamp.version(), &[f64::NAN]),
        Err(HeapError::NonFinite { index: 0 })
    ));
    assert!(matches!(
        heap.replace_represented(selected, stamp.version(), &[]),
        Err(HeapError::LengthMismatch)
    ));
    assert_eq!(heap.value(selected)?.stamp(), stamp);
    assert_eq!(heap.companion(bound(base)?)?.target_stamp(), stamp);
    let changed = heap.replace_declared_bounds(selected, stamp.version(), &[3.0], &[0.5])?;
    assert!(matches!(
        heap.replace_represented(selected, stamp.version(), &[4.0]),
        Err(HeapError::VersionMismatch { .. })
    ));
    assert_eq!(
        heap.value(changed.value())?.region().get(0, 0)?.to_bits(),
        3.0_f64.to_bits()
    );
    Ok(())
}

/// Partial bounds cannot fill unknown entries, while a bijective alias can declare the whole base.
/// ´test:integration:selected-bounds-require-complete-base-coverage´
#[test]
fn selected_bounds_require_complete_base_coverage() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let base =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64, 2.0])?;
    let selected = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 2, 1, 0, 0)?,
    )?;
    let stamp = heap.value(base.value())?.stamp();
    assert!(matches!(
        heap.replace_declared_bounds(selected, stamp.version(), &[3.0], &[0.5]),
        Err(HeapError::MissingCoverage)
    ));
    assert_eq!(heap.value(base.value())?.stamp(), stamp);
    let reverse = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(2, 1)?, 2, 1, -1, 0)?,
    )?;
    heap.replace_declared_bounds(reverse, stamp.version(), &[3.0, 4.0], &[0.5, 1.0])?;
    assert_eq!(
        heap.value(base.value())?.region().get(0, 0)?.to_bits(),
        4.0_f64.to_bits()
    );
    assert_eq!(
        heap.selected_bound(base.value())?
            .ok_or(HeapError::StaleEvidence)?
            .region()
            .get(0, 0)?
            .to_bits(),
        1.0_f64.to_bits()
    );
    Ok(())
}

/// Work and schema refusals after selecting an alias leave the complete base unchanged.
/// ´test:integration:selected-scatter-preparation-limits-are-atomic´
#[test]
fn selected_scatter_preparation_limits_are_atomic() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default().with_layout_work(2))?;
    let base = heap.import_represented(
        Extent::new(3, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 3.0],
    )?;
    let selected = heap.alias(
        base.value(),
        &Layout::strided(Extent::new(1, 1)?, 3, 1, 0, 0)?,
    )?;
    let stamp = heap.value(base.value())?.stamp();
    assert!(matches!(
        heap.replace_represented(selected, stamp.version(), &[7.0]),
        Err(HeapError::WorkLimit)
    ));
    assert_eq!(heap.value(selected)?.stamp(), stamp);
    assert_eq!(
        heap.value(selected)?.region().get(0, 0)?.to_bits(),
        2.0_f64.to_bits()
    );
    let mut limited = Heap::new(HeapLimits::new(3, 4, 1))?;
    let whole =
        limited.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64, 2.0])?;
    let reverse = limited.alias(
        whole.value(),
        &Layout::strided(Extent::new(2, 1)?, 2, 1, -1, 0)?,
    )?;
    let before = limited.value(whole.value())?.stamp();
    assert!(matches!(
        limited.replace_declared_bounds(reverse, before.version(), &[7.0, 8.0], &[0.5; 2]),
        Err(HeapError::Capacity)
    ));
    assert_eq!(limited.value(reverse)?.stamp(), before);
    assert!(limited.selected_bound(reverse)?.is_none());
    assert_eq!(
        limited.value(reverse)?.region().get(0, 0)?.to_bits(),
        2.0_f64.to_bits()
    );
    Ok(())
}
