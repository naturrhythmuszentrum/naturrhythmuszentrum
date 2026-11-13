// SPDX-License-Identifier: AGPL-3.0-only
//! Packed native cell masks, immutable occupations and bounded identity-preserving compaction.

mod algebra;
mod compaction;
mod layout;
pub(crate) mod memory;
pub(crate) mod storage;
mod transport;
mod types;

pub use algebra::MaskPartition;
pub(crate) use storage::Store;
pub use types::{
    CellAxis, CellCorrespondence, CellGather, CellLayoutVersion, MaskError, MaskVersion, MaskView,
};

use nrz_bindings::{Budget, readings::algebra::Location};

/// All mask charges use one stable accounting location and the caller's existing ledger.
const AT: Location = Location::new("native heap mask");
/// Consumer family on the shared ledger's boundary receipts.
const FAMILY: &str = "matrix.mask";

/// Native counts are not physical quantities; every conversion remains checked.
fn count(value: usize) -> Result<u64, MaskError> {
    u64::try_from(value).map_err(|_| MaskError::CountOverflow)
}

/// Consume one unit per declared native step before executing it.
fn charge(budget: &mut Budget, steps: usize) -> Result<(), MaskError> {
    budget.work(count(steps)?, AT)?;
    Ok(())
}

/// The division form does not add padding to an unchecked capacity.
const fn word_count(cells: usize) -> usize {
    cells.div_ceil(64)
}

/// Clear the final unused bits without shifting by the word width.
const fn clear_padding(words: &mut [u64], cells: usize) {
    let remainder = cells % 64;
    if remainder != 0
        && let Some(last) = words.last_mut()
    {
        *last &= (1_u64 << remainder) - 1;
    }
}

#[cfg(test)]
mod tests;
