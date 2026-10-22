// SPDX-License-Identifier: AGPL-3.0-only
//! Crate interaction with typed locator resolution.

use nrz_bindings::Descriptor;

use crate::heap::identity::Key;
use crate::{Companion, Extent, Handle, Heap, HeapError, HeapLimits, Value};

/// Runtime resolution checks complement the public category/precision compile twins.
/// ´test:crate:forged-locator-refusals´
#[test]
fn forged_locator_refusals() -> Result<(), HeapError> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let bundle =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64], &[0.5])?;
    let bound = bundle.bound().ok_or(HeapError::StaleEvidence)?;
    let wrong_category = Handle::<Value, f64>::new(bundle.value().scope, bound.key);
    assert!(matches!(
        heap.value(wrong_category),
        Err(HeapError::CategoryMismatch)
    ));
    let wrong_companion = Handle::<Companion, f64>::new(bundle.value().scope, bundle.value().key);
    assert!(matches!(
        heap.companion(wrong_companion),
        Err(HeapError::CategoryMismatch)
    ));
    let wrong_precision = Handle::<Value, f32>::new(bundle.value().scope, bundle.value().key);
    assert!(matches!(
        heap.value(wrong_precision),
        Err(HeapError::PrecisionMismatch)
    ));
    let foreign = Handle::<Value, f64>::new(
        0,
        Key {
            slot: usize::MAX,
            generation: 1,
        },
    );
    assert!(matches!(heap.value(foreign), Err(HeapError::ForeignHandle)));
    Ok(())
}

/// A full heap actually reuses descriptor positions while advancing both occupations.
/// ´test:crate:bundle-retirement-reuses-exact-slots´
#[test]
fn bundle_retirement_reuses_exact_slots() -> Result<(), HeapError> {
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    let old = heap.import_declared_bounds(extent, &quantity, &[2.0_f64], &[0.5])?;
    let old_bound = old.bound().ok_or(HeapError::StaleEvidence)?;
    heap.retire(old.value())?;
    let fresh = heap.import_declared_bounds(extent, &quantity, &[2.0_f64], &[0.5])?;
    let fresh_bound = fresh.bound().ok_or(HeapError::StaleEvidence)?;
    assert_eq!(old.value().key.slot, fresh.value().key.slot);
    assert_eq!(old_bound.key.slot, fresh_bound.key.slot);
    assert_eq!(fresh.value().key.generation, old.value().key.generation + 1);
    assert_eq!(fresh_bound.key.generation, old_bound.key.generation + 1);
    Ok(())
}
