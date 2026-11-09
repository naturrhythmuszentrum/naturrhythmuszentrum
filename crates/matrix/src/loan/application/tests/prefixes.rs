// SPDX-License-Identifier: AGPL-3.0-only
//! Independent physical allocation schedule for the C=k=2, three-channel growth fixture.
use super::TestResult;

/// Acquisitions: three registry replacements; three key/replacement pairs; debit staging;
/// nine channel buffers; layout/bundle/pin/witness; filled/outcomes/extent;
/// descriptor/layout/mask staging; application box. Only six table replacements survive.
const RETAINED: [usize; 30] = [
    0, 1, 2, 3, 3, 4, 4, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
];
/// Private keys, staging and owners are destroyed on failure; no table is in this inventory.
const DESTROYED: [usize; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    22, 23,
];
/// Literal deltas: 3 group rows at 888, 9 debit rows at 248, one weak row at 40,
/// then 3 descriptor rows at 256 and one native row at 32 for each native table.
const DELTAS: [usize; 10] = [0, 2664, 4896, 4936, 4936, 5704, 5704, 5736, 5736, 5768];

/// Assert physical table capacity and unchanged occupations independently of the prefix guard.
pub(super) fn check(heap: &crate::Heap, prefix: usize, before: usize) -> TestResult {
    assert_eq!(
        crate::loan::fault::Injection::prefix(),
        [prefix, RETAINED[prefix], DESTROYED[prefix]]
    );
    assert_eq!(heap.resource_bytes()?, before + DELTAS[prefix.min(9)]);
    assert_eq!(
        [
            heap.loans.groups.len(),
            heap.loans.debits.len(),
            heap.loans.pins.len()
        ],
        [3, 15, 1]
    );
    assert_eq!(
        heap.loans.groups.capacity(),
        if prefix >= 1 { 6 } else { 3 }
    );
    assert_eq!(
        heap.loans.debits.capacity(),
        if prefix >= 2 { 24 } else { 15 }
    );
    assert_eq!(heap.loans.pins.capacity(), if prefix >= 3 { 2 } else { 1 });
    assert_eq!(
        heap.descriptors.slots.owned_bytes()?,
        if prefix >= 5 { 1536 } else { 768 }
    );
    assert_eq!(
        heap.masks.layouts.owned_bytes()?,
        if prefix >= 7 { 64 } else { 32 }
    );
    assert_eq!(
        heap.masks.masks.owned_bytes()?,
        if prefix >= 9 { 96 } else { 64 }
    );
    assert_eq!(
        [
            heap.descriptors.slots.scan_work(),
            heap.masks.layouts.scan_work(),
            heap.masks.masks.scan_work()
        ],
        [3, 1, 2]
    );
    Ok(())
}
