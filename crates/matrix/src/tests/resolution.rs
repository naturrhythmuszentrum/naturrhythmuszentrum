// SPDX-License-Identifier: AGPL-3.0-only
//! Crate interaction with typed locator resolution.

use nrz_units::Descriptor;

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
