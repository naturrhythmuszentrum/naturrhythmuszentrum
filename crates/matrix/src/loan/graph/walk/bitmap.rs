// SPDX-License-Identifier: AGPL-3.0-only
//! The fixed 64-bit hierarchy uses the design's climb, descend and bounded wrap.

use crate::loan::{LoanError, charge};
use nrz_bindings::Budget;

/// Word extent at one level; checked construction already proved the backing extent.
fn extent(capacity: usize, level: usize) -> usize {
    let mut words = capacity.div_ceil(64);
    for _ in 0..level {
        words = words.div_ceil(64);
    }
    words
}
/// Offset of a level in the single packed backing.
fn offset(capacity: usize, level: usize) -> usize {
    (0..level).map(|lower| extent(capacity, lower)).sum()
}
/// Every runtime bit transition visits all levels, including unchanged summary values.
pub(super) fn height(capacity: usize) -> usize {
    if capacity == 0 {
        return 0;
    }
    let mut height = 1;
    while extent(capacity, height - 1) > 1 {
        height += 1;
    }
    height
}
/// Commit a prepaid complete path without a fallible suffix.
pub(super) fn set(words: &mut [u64], capacity: usize, slot: usize, mut value: bool) {
    let mut index = slot;
    for level in 0..height(capacity) {
        let word = offset(capacity, level) + index / 64;
        let bit = 1_u64 << (index % 64);
        if value {
            words[word] |= bit;
        } else {
            words[word] &= !bit;
        }
        value = words[word] != 0;
        index /= 64;
    }
}
/// Inspect one word and remove unused tail bits; every actual read is M.
fn read(
    words: &[u64],
    capacity: usize,
    level: usize,
    index: usize,
    budget: &mut Budget,
) -> Result<u64, LoanError> {
    charge(budget, 1)?;
    let represented = if level == 0 {
        capacity
    } else {
        extent(capacity, level - 1)
    };
    let remaining = represented - index * 64;
    let mask = if remaining < 64 {
        (1_u64 << remaining) - 1
    } else {
        u64::MAX
    };
    Ok(words[offset(capacity, level) + index] & mask)
}
/// The top word can prove that no wrap is necessary without caching lower ancestors.
struct Search {
    /// First eligible leaf in this interval.
    found: Option<usize>,
    /// The actual top word was empty, before interval masking.
    empty: bool,
}
/// Search one ordered interval; summaries keep the child containing the exclusive bound.
fn interval(
    words: &[u64],
    capacity: usize,
    start: usize,
    bound: usize,
    budget: &mut Budget,
) -> Result<Search, LoanError> {
    if start >= bound {
        return Ok(Search {
            found: None,
            empty: false,
        });
    }
    let mut level = 0;
    let mut index = start / 64;
    let mut skip = start % 64;
    loop {
        let mut word = read(words, capacity, level, index, budget)?;
        if level + 1 == height(capacity) && word == 0 {
            return Ok(Search {
                found: None,
                empty: true,
            });
        }
        word &= u64::MAX
            .checked_shl(u32::try_from(skip).map_err(|_| LoanError::CountOverflow)?)
            .unwrap_or(0);
        let scale = 64_usize.pow(u32::try_from(level).map_err(|_| LoanError::CountOverflow)?);
        let upper = if level == 0 { bound - 1 } else { bound / scale };
        if upper < index * 64 {
            word = 0;
        } else if upper - index * 64 < 63 {
            word &= (1_u64 << (upper - index * 64 + 1)) - 1;
        }
        if word != 0 {
            let child = index * 64 + word.trailing_zeros() as usize;
            if level == 0 {
                return Ok(Search {
                    found: (child < bound).then_some(child),
                    empty: false,
                });
            }
            index = child;
            level -= 1;
            skip = 0;
            continue;
        }
        if level + 1 == height(capacity) {
            return Ok(Search {
                found: None,
                empty: false,
            });
        }
        skip = index % 64 + 1;
        index /= 64;
        level += 1;
    }
}
/// Search from the saved cursor, then wrap once; the caller alone commits cursor movement.
pub(super) fn next(
    words: &[u64],
    capacity: usize,
    cursor: usize,
    budget: &mut Budget,
) -> Result<Option<usize>, LoanError> {
    if capacity == 0 {
        return Ok(None);
    }
    let forward = interval(words, capacity, cursor, capacity, budget)?;
    if forward.found.is_some() || forward.empty {
        return Ok(forward.found);
    }
    Ok(interval(words, capacity, 0, cursor, budget)?.found)
}
