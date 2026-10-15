// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor indirection and typed storage interaction during compaction.

use nrz_units::Descriptor;

use crate::storage::{Format, Region, Storage};
use crate::{Extent, Heap, HeapError, HeapLimits, Layout};

/// Compaction removes arena holes without relocating descriptor slots or changing alias roots.
/// ´test:crate:compaction-densifies-arena-indirection´
#[test]
fn compaction_densifies_arena_indirection() -> Result<(), HeapError> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(1, 1)?;
    let first = heap.import_represented(extent, &Descriptor::ratio(), &[1.0_f64])?;
    let live = heap.import_represented(extent, &Descriptor::ratio(), &[2.0_f64])?;
    let alias = heap.alias(live.value(), &Layout::contiguous(extent)?)?;
    let before = heap.owning_region(heap.record(live.value())?)?;
    assert_eq!(before.arena.slot, 1);
    heap.retire(first.value())?;
    heap.compact()?;
    let moved = heap.owning_region(heap.record(live.value())?)?;
    let selected = heap.owning_region(heap.record(alias)?)?;
    assert_eq!(moved.arena.slot, 0);
    assert_eq!(moved.epoch, before.epoch + 1);
    assert_eq!(selected.arena, moved.arena);
    assert_eq!(selected.epoch, moved.epoch);
    assert_eq!(
        heap.value(alias)?.stamp(),
        heap.value(live.value())?.stamp()
    );
    Ok(())
}

/// Even an identical arena slot/generation cannot make a pre-relocation physical map readable.
/// ´test:crate:relocation-epoch-refuses-cached-physical-region´
#[test]
fn relocation_epoch_refuses_cached_physical_region() -> Result<(), HeapError> {
    let mut storage = Storage::new();
    let prepared = storage.prepare::<f64>(&[&[1.0]], &[], HeapLimits::default())?;
    let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
    storage.publish(prepared);
    let old = Region {
        epoch: storage.epoch(),
        arena,
        format: Format::Binary64,
        offset: 0,
        length: 1,
    };
    let mut replacement = storage.replacement()?;
    let prepared = replacement.prepare::<f64>(&[&[2.0]], &[], HeapLimits::default())?;
    assert_eq!(prepared.keys().first(), Some(&arena));
    replacement.publish(prepared);
    assert!(matches!(
        replacement.resolve(old),
        Err(HeapError::StaleLayout)
    ));
    assert_eq!(
        replacement
            .resolve(Region {
                epoch: replacement.epoch(),
                ..old
            })?
            .entry::<f64>(0)?
            .to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        storage.resolve(old)?.entry::<f64>(0)?.to_bits(),
        1.0_f64.to_bits()
    );
    Ok(())
}

/// Physical output disjointness includes each owning region's offset inside a shared slab.
/// ´test:crate:output-proof-respects-region-offsets´
#[test]
fn output_proof_respects_region_offsets() -> Result<(), HeapError> {
    let mut storage = Storage::new();
    let prepared = storage.prepare::<f64>(&[&[1.0; 6]], &[], HeapLimits::default())?;
    let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
    storage.publish(prepared);
    let left = Region {
        epoch: storage.epoch(),
        arena,
        format: Format::Binary64,
        offset: 0,
        length: 2,
    };
    let right = Region { offset: 3, ..left };
    let layout = Layout::contiguous(Extent::new(2, 1)?)?;
    storage.resolve(left)?;
    storage.resolve(right)?;
    crate::layout::validate_outputs(&[(left, &layout), (right, &layout)], 0)?;
    let overlap = Region { offset: 1, ..left };
    assert_eq!(
        crate::layout::validate_outputs(&[(left, &layout), (overlap, &layout)], 0),
        Err(HeapError::OutputOverlap)
    );
    assert_eq!(
        Layout::strided(Extent::new(1, 1)?, right.length, 2, 0, 0),
        Err(HeapError::InvalidLayout)
    );
    Ok(())
}
