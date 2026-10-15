// SPDX-License-Identifier: AGPL-3.0-only
//! Output injectivity and mutual disjointness proofs over resolved regions.

use super::{Layout, check_work};
use crate::HeapError;
use crate::storage::Region;

/// Use exact dense ranges or bounded address enumeration, never an overlapping bounding box.
pub(crate) fn validate_outputs(
    destinations: &[(Region, &Layout)],
    work_limit: usize,
) -> Result<(), HeapError> {
    if dense_disjoint(destinations)? {
        return Ok(());
    }
    let count = destinations
        .iter()
        .try_fold(0_usize, |total, (_, layout)| {
            total
                .checked_add(layout.extent().entries())
                .ok_or(HeapError::WorkLimit)
        })?;
    check_work(count, work_limit)?;
    let mut addresses = Vec::new();
    addresses
        .try_reserve_exact(count)
        .map_err(|_| HeapError::Capacity)?;
    for (region, layout) in destinations {
        for logical in 0..layout.extent().entries() {
            let address = region
                .offset
                .checked_add(layout.flat_index(logical)?)
                .ok_or(HeapError::ExtentOverflow)?;
            addresses.push((region.arena, address));
        }
    }
    addresses.sort_unstable();
    for pair in addresses.windows(2) {
        let [left, right] = pair else {
            return Err(HeapError::InvalidLayout);
        };
        if left == right {
            return Err(HeapError::OutputOverlap);
        }
    }
    Ok(())
}

/// Dense ranges prove both injectivity and disjointness without an element-work charge.
fn dense_disjoint(destinations: &[(Region, &Layout)]) -> Result<bool, HeapError> {
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(destinations.len())
        .map_err(|_| HeapError::Capacity)?;
    for (region, layout) in destinations {
        let Some((start, end)) = layout.dense_range()? else {
            return Ok(false);
        };
        if start != end {
            let lower = region
                .offset
                .checked_add(start)
                .ok_or(HeapError::ExtentOverflow)?;
            let upper = region
                .offset
                .checked_add(end)
                .ok_or(HeapError::ExtentOverflow)?;
            ranges.push((region.arena, lower, upper));
        }
    }
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        let [left, right] = pair else {
            return Err(HeapError::InvalidLayout);
        };
        let (left_arena, _, left_upper) = *left;
        let (right_arena, right_lower, _) = *right;
        if left_arena == right_arena && left_upper > right_lower {
            return Err(HeapError::OutputOverlap);
        }
    }
    Ok(true)
}
