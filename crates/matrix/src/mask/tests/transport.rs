// SPDX-License-Identifier: AGPL-3.0-only
//! Cell sets are independent of the component address maps that carry them.

use super::{TestResult, assert_mask, fixture, pattern, publish};
use crate::{Extent, Heap, HeapError, HeapLimits, Layout, MaskError};

/// Separate component planes and interleaved companions preserve the same cell sets.
/// ´test:unit:mask-component-layout-transport´
#[test]
fn mask_component_layout_transport() -> TestResult {
    for capacity in [0, 1, 63, 64, 65, 128] {
        for kind in 0..5 {
            let (mut heap, source, _, mut budget) = fixture(capacity)?;
            let expected = pattern(capacity, kind);
            let mask = publish(&mut heap, source, &expected, &mut budget)?;
            let axis = heap.mask(mask, source)?.axis();
            let extent = Extent::new(capacity, 2)?;
            let target = heap.create_cell_layout(
                axis,
                Layout::strided(extent, extent.entries(), 0, 2, 1)?,
                &mut budget,
            )?;
            let transported = heap.transport_mask(mask, target, &mut budget)?;
            assert_mask(&heap, transported, &expected)?;
            let view = heap.mask(transported, target)?;
            for cell in 0..capacity {
                for component in 0..2 {
                    assert_eq!(
                        view.components().index(cell, component)?,
                        2 * cell + component
                    );
                }
            }
            let identity = heap.mask_correspondence(target, target, &mut budget)?;
            let all = heap.create_mask(target, &(0..capacity).collect::<Vec<_>>(), &mut budget)?;
            let joined = heap.intersect_masks(identity, transported, all, &mut budget)?;
            assert_mask(&heap, joined, &expected)?;
            heap.retire_mask(mask, &mut budget)?;
            heap.retire_cell_layout(source, &mut budget)?;
            assert_mask(&heap, transported, &expected)?;
        }
    }
    Ok(())
}

/// Identical dimensions, foreign owners and old generations cannot supply a cell map.
/// ´test:unit:mask-component-transport-refusals´
#[test]
fn mask_component_transport_refusals() -> TestResult {
    let (mut heap, source, correspondence, mut budget) = fixture(65)?;
    let mask = heap.create_mask(source, &[1, 64], &mut budget)?;
    let axis = heap.mask(mask, source)?.axis();
    let unrelated_axis = heap.create_cell_axis(65, &mut budget)?;
    let unrelated = heap.create_cell_layout(
        unrelated_axis,
        Layout::contiguous(Extent::new(65, 3)?)?,
        &mut budget,
    )?;
    let before = heap.mask_storage_bytes(&mut budget)?;
    assert!(matches!(
        heap.transport_mask(mask, unrelated, &mut budget),
        Err(MaskError::LayoutMismatch)
    ));
    assert_eq!(heap.mask_storage_bytes(&mut budget)?, before);
    let mut foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        foreign.transport_mask(mask, source, &mut budget),
        Err(MaskError::Heap(HeapError::ForeignHandle))
    ));
    let compacted = heap.compact_mask(correspondence, mask, 65, 2, &mut budget)?;
    assert!(matches!(
        heap.transport_mask(compacted, compacted.layout(), &mut budget),
        Err(MaskError::NonIdentityCorrespondence)
    ));
    let target =
        heap.create_cell_layout(axis, Layout::contiguous(Extent::new(65, 1)?)?, &mut budget)?;
    let next = heap.replace_cell_layout(
        target,
        Layout::contiguous(Extent::new(65, 2)?)?,
        &mut budget,
    )?;
    assert!(matches!(
        heap.transport_mask(mask, target, &mut budget),
        Err(MaskError::Heap(HeapError::StaleHandle))
    ));
    let transported = heap.transport_mask(mask, next, &mut budget)?;
    heap.retire_mask(mask, &mut budget)?;
    assert!(matches!(
        heap.transport_mask(mask, next, &mut budget),
        Err(MaskError::Heap(HeapError::StaleHandle))
    ));
    assert_mask(&heap, transported, &[1, 64].into_iter().collect())?;
    Ok(())
}

/// Two authentication units, three resident slots, two initialized words, two copied
/// words and one mask publication scan give ten Work units for the 65-cell fixture.
/// These counts are the declared inventory, not a measurement of the operation.
/// ´test:unit:mask-component-transport-work-boundary´
#[test]
fn mask_component_transport_work_boundary() -> TestResult {
    for (remaining, succeeds) in [(9, false), (10, true)] {
        let (mut heap, source, _, mut preparation) = fixture(65)?;
        let mask = heap.create_mask(source, &[1, 64], &mut preparation)?;
        let axis = heap.mask(mask, source)?.axis();
        let target = heap.create_cell_layout(
            axis,
            Layout::contiguous(Extent::new(65, 1)?)?,
            &mut preparation,
        )?;
        let limits = nrz_bindings::Limits {
            work: remaining + 3,
            ..nrz_bindings::Limits::default()
        };
        let mut budget = nrz_bindings::Budget::new(limits.clone())?;
        budget.work(3, super::super::AT)?;
        let before = heap.mask_storage_bytes(&mut preparation)?;
        let result = heap.transport_mask(mask, target, &mut budget);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(budget.usage().work, 13);
            assert_mask(&heap, result?, &[1, 64].into_iter().collect())?;
        } else {
            assert!(matches!(result, Err(MaskError::Budget(_))));
            assert_eq!(heap.mask_storage_bytes(&mut preparation)?, before);
            assert_mask(&heap, mask, &[1, 64].into_iter().collect())?;
        }
    }
    Ok(())
}
