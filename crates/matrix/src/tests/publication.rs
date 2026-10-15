// SPDX-License-Identifier: AGPL-3.0-only
//! Crate interaction with private prepared occupation changes.

use crate::HeapError;
use crate::heap::slots::Slots;

/// A refused payload count or capacity leaves the original occupation readable.
/// ´test:crate:prepared-links-refuse-incomplete-payloads´
#[test]
fn prepared_links_refuse_incomplete_payloads() -> Result<(), HeapError> {
    let mut slots = Slots::new();
    let plan = slots.prepare(1, 1)?;
    let key = *plan.keys().first().ok_or(HeapError::Capacity)?;
    slots.publish(plan.bind(vec![7])?);
    let change = slots.change(1, None, std::slice::from_ref(&key), 1)?;
    assert!(matches!(
        change.bind(Vec::<i32>::new()),
        Err(HeapError::LengthMismatch)
    ));
    assert_eq!(slots.get(key), Ok(&7));
    assert!(matches!(slots.prepare(1, 1), Err(HeapError::Capacity)));
    assert!(matches!(
        slots.change(1, Some(key), std::slice::from_ref(&key), 1),
        Err(HeapError::InvalidLayout)
    ));
    assert!(matches!(
        slots.change(0, None, &[key, key], 1),
        Err(HeapError::InvalidLayout)
    ));
    assert_eq!(slots.get(key), Ok(&7));
    Ok(())
}
