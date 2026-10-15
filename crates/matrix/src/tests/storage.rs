// SPDX-License-Identifier: AGPL-3.0-only
//! Crate interaction with the private storage facade.

use crate::heap::identity::Key;
use crate::storage::{Format, PreparedStorage, Region, Storage};
use crate::{HeapError, HeapLimits};

/// Arena generations and allocated ranges are rechecked before any entry read.
/// ´test:crate:arena-resolution-refuses-corrupt-ranges´
#[test]
fn arena_resolution_refuses_corrupt_ranges() -> Result<(), HeapError> {
    let mut storage = Storage::new();
    let keys = storage.reserve::<f64>(1, 2, HeapLimits::default())?;
    let arena = *keys.first().ok_or(HeapError::Capacity)?;
    storage.publish(PreparedStorage::new(&[1.0_f64, 2.0], None)?);
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
