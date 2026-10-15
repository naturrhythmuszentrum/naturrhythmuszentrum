// SPDX-License-Identifier: AGPL-3.0-only
//! Crate interaction with the private storage facade.

use crate::heap::identity::Key;
use crate::storage::{Format, Region, Storage};
use crate::{HeapError, HeapLimits};

/// Arena generations and allocated ranges are rechecked before any entry read.
/// ´test:crate:arena-resolution-refuses-corrupt-ranges´
#[test]
fn arena_resolution_refuses_corrupt_ranges() -> Result<(), HeapError> {
    let mut storage = Storage::new();
    let prepared = storage.prepare::<f64>(&[&[1.0, 2.0]], &[], HeapLimits::default())?;
    let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
    storage.publish(prepared);
    let region = Region {
        arena,
        format: Format::Binary64,
        offset: 0,
        length: 2,
    };
    let slab = storage.resolve(region)?;
    assert_eq!(slab.entry::<f64>(1)?.to_bits(), 2.0_f64.to_bits());
    assert_eq!(slab.entry::<f32>(0), Err(HeapError::PrecisionMismatch));
    assert_eq!(slab.entry::<f64>(2), Err(HeapError::InvalidLayout));
    assert!(matches!(
        storage.resolve(Region {
            offset: 1,
            ..region
        }),
        Err(HeapError::InvalidLayout)
    ));
    assert!(matches!(
        storage.resolve(Region {
            offset: usize::MAX,
            ..region
        }),
        Err(HeapError::ExtentOverflow)
    ));
    let stale = Key {
        slot: arena.slot,
        generation: arena.generation + 1,
    };
    assert!(matches!(
        storage.resolve(Region {
            arena: stale,
            ..region
        }),
        Err(HeapError::StaleHandle)
    ));
    Ok(())
}

/// Reusing an arena occupation cannot make a cached old physical region readable.
/// ´test:crate:arena-reuse-invalidates-old-regions´
#[test]
fn arena_reuse_invalidates_old_regions() -> Result<(), HeapError> {
    let mut storage = Storage::new();
    let limits = HeapLimits::new(1, 1, 1);
    let prepared = storage.prepare::<f64>(&[&[1.0]], &[], limits)?;
    let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
    storage.publish(prepared);
    let region = Region {
        arena,
        format: Format::Binary64,
        offset: 0,
        length: 1,
    };
    let changed = storage.prepare::<f64>(&[&[2.0]], std::slice::from_ref(&region), limits)?;
    let replacement = *changed.keys().first().ok_or(HeapError::Capacity)?;
    storage.publish(changed);
    assert_eq!(replacement.slot, arena.slot);
    assert_eq!(replacement.generation, arena.generation + 1);
    assert!(matches!(
        storage.resolve(region),
        Err(HeapError::StaleHandle)
    ));
    assert_eq!(
        storage
            .resolve(Region {
                arena: replacement,
                ..region
            })?
            .entry::<f64>(0)?
            .to_bits(),
        2.0_f64.to_bits()
    );
    Ok(())
}
