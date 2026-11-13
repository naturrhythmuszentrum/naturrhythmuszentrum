// SPDX-License-Identifier: AGPL-3.0-only
//! Independent Cartesian enumeration supplies projected mask and index expectations.

use super::{TestResult, assert_mask, ledger, pattern, publish};
use crate::{
    CellAxis, CellFactor, CellLayoutVersion, Extent, Heap, HeapError, HeapLimits, Layout, MaskError,
};
use nrz_bindings::Budget;
use std::collections::BTreeSet;

/// Each factor layout has separate component planes; no dimensions supply axis identity.
fn layout(heap: &mut Heap, axis: CellAxis, budget: &mut Budget) -> TestResult<CellLayoutVersion> {
    Ok(heap.create_cell_layout(
        axis,
        Layout::contiguous(Extent::new(axis.capacity(), 2)?)?,
        budget,
    )?)
}

/// Empty, sparse, partial and full Cartesian masks agree with independently enumerated tuples.
/// ´test:unit:mask-product-independent-projections´
#[test]
fn mask_product_independent_projections() -> TestResult {
    for outer_count in [0, 1, 2, 65] {
        for inner_count in [0, 1, 3] {
            for kind in 0..5 {
                projection(outer_count, inner_count, kind)?;
            }
        }
    }
    Ok(())
}

/// Nested loops supply row-major expectations without calling the map being checked.
fn projection(outer_count: usize, inner_count: usize, kind: usize) -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = ledger()?;
    let outer = heap.create_cell_axis(outer_count, &mut budget)?;
    let inner = heap.create_cell_axis(inner_count, &mut budget)?;
    let product = heap.create_cell_product(outer, inner, &mut budget)?;
    assert_eq!(product.factors(), [outer, inner]);
    let target = layout(&mut heap, product.axis(), &mut budget)?;
    for (factor, axis) in [(CellFactor::Outer, outer), (CellFactor::Inner, inner)] {
        let source_layout = layout(&mut heap, axis, &mut budget)?;
        let selected = pattern(axis.capacity(), kind);
        let source = publish(&mut heap, source_layout, &selected, &mut budget)?;
        let expanded = heap.expand_product_mask(product, factor, source, target, &mut budget)?;
        let mut expected = BTreeSet::new();
        let mut cell = 0;
        for i in 0..outer_count {
            for j in 0..inner_count {
                assert_eq!(product.coordinates(cell)?, [i, j]);
                let original = match factor {
                    CellFactor::Outer => i,
                    CellFactor::Inner => j,
                };
                if selected.contains(&original) {
                    expected.insert(cell);
                }
                cell += 1;
            }
        }
        assert_mask(&heap, expanded, &expected)?;
        assert!(matches!(
            product.coordinates(cell),
            Err(MaskError::CellOutOfBounds)
        ));
    }
    Ok(())
}

/// Instance/sample/branch composition preserves the full original tuple through two maps.
/// ´test:unit:mask-product-sample-branch-composition´
#[test]
fn mask_product_sample_branch_composition() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = ledger()?;
    let instance = heap.create_cell_axis(2, &mut budget)?;
    let sample = heap.create_cell_axis(3, &mut budget)?;
    let branch = heap.create_cell_axis(11, &mut budget)?;
    let samples = heap.create_cell_product(instance, sample, &mut budget)?;
    let branches = heap.create_cell_product(samples.axis(), branch, &mut budget)?;
    let source_layout = layout(&mut heap, sample, &mut budget)?;
    let samples_layout = layout(&mut heap, samples.axis(), &mut budget)?;
    let target = layout(&mut heap, branches.axis(), &mut budget)?;
    let source = heap.create_mask(source_layout, &[0, 2], &mut budget)?;
    let selected_samples = heap.expand_product_mask(
        samples,
        CellFactor::Inner,
        source,
        samples_layout,
        &mut budget,
    )?;
    let expanded = heap.expand_product_mask(
        branches,
        CellFactor::Outer,
        selected_samples,
        target,
        &mut budget,
    )?;
    let branch_layout = layout(&mut heap, branch, &mut budget)?;
    let selected_branches = heap.create_mask(branch_layout, &[1, 10], &mut budget)?;
    let expanded_branches = heap.expand_product_mask(
        branches,
        CellFactor::Inner,
        selected_branches,
        target,
        &mut budget,
    )?;
    let correspondence = heap.mask_correspondence(target, target, &mut budget)?;
    let joined = heap.intersect_masks(correspondence, expanded, expanded_branches, &mut budget)?;
    let mut expected = BTreeSet::new();
    let mut cell = 0;
    for i in 0..2 {
        for j in 0..3 {
            for b in 0..11 {
                let [sample_cell, branch_cell] = branches.coordinates(cell)?;
                assert_eq!(samples.coordinates(sample_cell)?, [i, j]);
                assert_eq!(branch_cell, b);
                let selected_sample = j == 0 || j == 2;
                let selected_branch = b == 1 || b == 10;
                if selected_sample && selected_branch {
                    expected.insert(cell);
                }
                cell += 1;
            }
        }
    }
    assert_mask(&heap, joined, &expected)
}

/// Axis identities, factor order and target generations are checked before publication.
/// ´test:unit:mask-product-identity-and-overflow-refusals´
#[test]
fn mask_product_identity_and_overflow_refusals() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = ledger()?;
    let outer = heap.create_cell_axis(2, &mut budget)?;
    let inner = heap.create_cell_axis(2, &mut budget)?;
    let product = heap.create_cell_product(outer, inner, &mut budget)?;
    let source_layout = layout(&mut heap, outer, &mut budget)?;
    let target = layout(&mut heap, product.axis(), &mut budget)?;
    let source = heap.create_mask(source_layout, &[1], &mut budget)?;
    let before = heap.mask_storage_bytes(&mut budget)?;
    assert!(matches!(
        heap.expand_product_mask(product, CellFactor::Inner, source, target, &mut budget),
        Err(MaskError::LayoutMismatch)
    ));
    let duplicate = heap.create_cell_product(outer, inner, &mut budget)?;
    assert!(matches!(
        heap.expand_product_mask(duplicate, CellFactor::Outer, source, target, &mut budget),
        Err(MaskError::LayoutMismatch)
    ));
    assert_eq!(heap.mask_storage_bytes(&mut budget)?, before);
    let foreign = Heap::new(HeapLimits::default())?;
    let foreign_axis = foreign.create_cell_axis(2, &mut budget)?;
    assert!(matches!(
        heap.create_cell_product(outer, foreign_axis, &mut budget),
        Err(MaskError::Heap(HeapError::ForeignHandle))
    ));
    let enormous = heap.create_cell_axis(usize::MAX, &mut budget)?;
    assert!(matches!(
        heap.create_cell_product(enormous, inner, &mut budget),
        Err(MaskError::CountOverflow)
    ));
    let next =
        heap.replace_cell_layout(target, Layout::contiguous(Extent::new(4, 1)?)?, &mut budget)?;
    assert!(matches!(
        heap.expand_product_mask(product, CellFactor::Outer, source, target, &mut budget),
        Err(MaskError::Heap(HeapError::StaleHandle))
    ));
    let result = heap.expand_product_mask(product, CellFactor::Outer, source, next, &mut budget)?;
    heap.retire_mask(source, &mut budget)?;
    assert!(matches!(
        heap.expand_product_mask(product, CellFactor::Outer, source, next, &mut budget),
        Err(MaskError::Heap(HeapError::StaleHandle))
    ));
    assert_mask(&heap, result, &[2, 3].into_iter().collect())
}

/// Three checks, three resident slots, one initialized word, six projected cells and
/// one mask publication scan require fourteen Work units, derived from the inventory.
/// ´test:unit:mask-product-literal-work-boundary´
#[test]
fn mask_product_literal_work_boundary() -> TestResult {
    for (work, succeeds) in [(13, false), (14, true)] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut preparation = ledger()?;
        let outer = heap.create_cell_axis(2, &mut preparation)?;
        let inner = heap.create_cell_axis(3, &mut preparation)?;
        let product = heap.create_cell_product(outer, inner, &mut preparation)?;
        let source_layout = layout(&mut heap, outer, &mut preparation)?;
        let target = layout(&mut heap, product.axis(), &mut preparation)?;
        let source = heap.create_mask(source_layout, &[1], &mut preparation)?;
        let mut budget = Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        let before = heap.mask_storage_bytes(&mut preparation)?;
        let result =
            heap.expand_product_mask(product, CellFactor::Outer, source, target, &mut budget);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(budget.usage().work, 14);
            assert_mask(&heap, result?, &[3, 4, 5].into_iter().collect())?;
        } else {
            assert!(matches!(result, Err(MaskError::Budget(_))));
            assert_eq!(heap.mask_storage_bytes(&mut preparation)?, before);
        }
    }
    Ok(())
}
