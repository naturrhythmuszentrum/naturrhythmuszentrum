// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original-cell sets determine transport between compacted occupations.

use super::{TestResult, assert_mask, fixture, pattern, publish};
use crate::{HeapError, MaskError};
use nrz_bindings::{Budget, Limits};
use std::collections::BTreeSet;

/// Dense, compacted and sibling layouts agree through original associations, including empty sets.
/// ´test:unit:mask-gather-transport-independent-sets´
#[test]
fn mask_gather_transport_independent_sets() -> TestResult {
    for source_kind in 0..5 {
        for target_kind in 0..5 {
            let (mut heap, root, identity, mut budget) = fixture(65)?;
            let source_set = pattern(65, source_kind);
            let target_set = pattern(65, target_kind);
            let source = publish(&mut heap, root, &source_set, &mut budget)?;
            let target = publish(&mut heap, root, &target_set, &mut budget)?;
            let source_capacity = if source_set.is_empty() { 0 } else { 65 };
            let target_capacity = if target_set.is_empty() { 0 } else { 65 };
            let compact_source =
                heap.compact_mask(identity, source, source_capacity, 65, &mut budget)?;
            let compact_target =
                heap.compact_mask(identity, target, target_capacity, 65, &mut budget)?;
            let expected = target_set
                .iter()
                .enumerate()
                .filter_map(|(position, original)| {
                    source_set.contains(original).then_some(position)
                })
                .collect::<BTreeSet<_>>();
            for input in [source, compact_source] {
                let moved =
                    heap.transport_gather_mask(input, compact_target.layout(), &mut budget)?;
                assert_mask(&heap, moved, &expected)?;
            }
            let restored = heap.transport_gather_mask(compact_source, root, &mut budget)?;
            assert_mask(&heap, restored, &source_set)?;
            // Root retirement cannot erase original-index metadata retained by both compacted layouts.
            heap.retire_cell_layout(root, &mut budget)?;
            let moved =
                heap.transport_gather_mask(compact_source, compact_target.layout(), &mut budget)?;
            assert_mask(&heap, moved, &expected)?;
        }
    }
    Ok(())
}

/// Nested compaction retains original cells rather than interpreting intermediate row numbers.
/// ´test:unit:mask-gather-transport-nested-and-stale´
#[test]
fn mask_gather_transport_nested_and_stale() -> TestResult {
    let (mut heap, root, identity, mut budget) = fixture(65)?;
    let source = heap.create_mask(root, &[1, 7, 64], &mut budget)?;
    let compact = heap.compact_mask(identity, source, 4, 3, &mut budget)?;
    let subset = heap.create_mask(compact.layout(), &[1, 2], &mut budget)?;
    let compact_identity =
        heap.mask_correspondence(compact.layout(), compact.layout(), &mut budget)?;
    let nested = heap.compact_mask(compact_identity, subset, 3, 2, &mut budget)?;
    let restored = heap.transport_gather_mask(nested, root, &mut budget)?;
    assert_mask(&heap, restored, &[7, 64].into_iter().collect())?;
    let (foreign, foreign_layout, _, _) = fixture(65)?;
    drop(foreign);
    assert!(matches!(
        heap.transport_gather_mask(nested, foreign_layout, &mut budget),
        Err(MaskError::Heap(HeapError::ForeignHandle))
    ));
    heap.retire_mask(nested, &mut budget)?;
    assert!(matches!(
        heap.transport_gather_mask(nested, root, &mut budget),
        Err(MaskError::Heap(HeapError::StaleHandle))
    ));
    let axis = heap.create_cell_axis(65, &mut budget)?;
    let unrelated = heap.create_cell_layout(
        axis,
        crate::Layout::contiguous(crate::Extent::new(65, 1)?)?,
        &mut budget,
    )?;
    assert!(matches!(
        heap.transport_gather_mask(compact, unrelated, &mut budget),
        Err(MaskError::LayoutMismatch)
    ));
    Ok(())
}

/// Two checks, four resident slots, one initialized word, four target visits, seven
/// ordered-map comparisons for [1,3], and two publication-scan slots total twenty.
/// ´test:unit:mask-gather-transport-literal-work-boundary´
#[test]
fn mask_gather_transport_literal_work_boundary() -> TestResult {
    for (work, succeeds) in [(19, false), (20, true)] {
        let (mut heap, root, identity, mut preparation) = fixture(4)?;
        let source = heap.create_mask(root, &[1, 3], &mut preparation)?;
        let compact = heap.compact_mask(identity, source, 4, 2, &mut preparation)?;
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let before = heap.mask_storage_bytes(&mut preparation)?;
        let result = heap.transport_gather_mask(compact, root, &mut budget);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(budget.usage().work, 20);
            assert_mask(&heap, result?, &[1, 3].into_iter().collect())?;
        } else {
            assert!(matches!(result, Err(MaskError::Budget(_))));
            assert_eq!(heap.mask_storage_bytes(&mut preparation)?, before);
        }
    }
    Ok(())
}
