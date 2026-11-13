// SPDX-License-Identifier: AGPL-3.0-only
//! Independent native-set, correspondence, resource and movement controls.

mod algebra;
mod compaction;
mod gather_transport;
mod product;
mod resources;
mod trace;
mod transport;
mod versions;

use super::{CellCorrespondence, CellLayoutVersion, MaskVersion};
use crate::{Extent, Heap, HeapLimits, Layout};
use nrz_bindings::{Budget, Limits};
use std::collections::BTreeSet;

/// Tests preserve typed failures without unwrapping resource outcomes.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// One ordinary default ledger; individual boundary controls supply narrower policies.
fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::default())?)
}

/// Native layout fixture deliberately carries several components per cell.
fn fixture(capacity: usize) -> TestResult<(Heap, CellLayoutVersion, CellCorrespondence, Budget)> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut budget = ledger()?;
    let axis = heap.create_cell_axis(capacity, &mut budget)?;
    let layout = heap.create_cell_layout(
        axis,
        Layout::contiguous(Extent::new(capacity, 3)?)?,
        &mut budget,
    )?;
    let correspondence = heap.mask_correspondence(layout, layout, &mut budget)?;
    Ok((heap, layout, correspondence, budget))
}

/// Independent logical-set oracle checks every cell and every padding bit.
fn assert_mask(heap: &Heap, version: MaskVersion, expected: &BTreeSet<usize>) -> TestResult {
    let view = heap.mask(version, version.layout())?;
    let capacity = view.axis().capacity();
    assert_eq!(view.words().len(), capacity.div_ceil(64));
    for cell in 0..capacity {
        assert_eq!(view.contains(cell)?, expected.contains(&cell));
    }
    for cell in capacity..view.words().len() * 64 {
        let word = view.words()[cell / 64];
        let padding_is_clear = word & (1_u64 << (cell % 64)) == 0;
        assert!(padding_is_clear);
    }
    assert!(view.contains(capacity).is_err());
    Ok(())
}

/// Empty, single, partial, sparse and full populations vary independently of storage words.
fn pattern(capacity: usize, kind: usize) -> BTreeSet<usize> {
    (0..capacity)
        .filter(|&cell| match kind {
            0 => false,
            1 => cell == capacity / 2,
            2 => cell < capacity.saturating_sub(2),
            3 => cell % 7 == 1 || cell % 13 == 4,
            _ => true,
        })
        .collect()
}

/// Copy a test-owned set into the public native initialization interface.
fn publish(
    heap: &mut Heap,
    layout: CellLayoutVersion,
    selected: &BTreeSet<usize>,
    budget: &mut Budget,
) -> TestResult<MaskVersion> {
    let cells: Vec<_> = selected.iter().copied().collect();
    Ok(heap.create_mask(layout, &cells, budget)?)
}
