// SPDX-License-Identifier: AGPL-3.0-only
//! One borrowed binding carries captured authority from readiness into prepaid evaluation.
use crate::loan::lc_origin::{LcClosure, LcSchedule};
use crate::loan::registry::Identity;
use crate::loan::{charge, payload::bit};
use crate::mask::storage::{LayoutRecord, MaskRecord};
use crate::{LcInputInventory, LcInputProduct, LcInputReaders, LcProductError, LoanError};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;

/// Split borrows keep output buffers writable while the captured input planes are bound.
pub(super) struct Source<'a> {
    /// Original native identity.
    pub(super) identity: &'a Identity,
    /// Final liveness authority.
    pub(super) liveness: &'a Identity,
    /// Fixed admitted schedule.
    pub(super) schedule: &'a LcSchedule,
    /// Complete captured input rows.
    pub(super) closure: &'a LcClosure,
    /// Original demand geometry.
    pub(super) demand_layout: &'a LayoutRecord,
    /// Immutable selected cells.
    pub(super) demand_mask: &'a MaskRecord,
    /// Exact immutable demand identity.
    pub(super) demand: crate::MaskVersion,
}
/// Only these source borrows survive readiness; output storage remains independently mutable.
pub(super) struct Ready<'a, P: Scalar, const N: usize> {
    /// Complete original observations with resolved planes and admitted arithmetic context.
    pub(super) product: LcInputProduct<'a, P, N>,
    /// Original output capacity.
    pub(super) cells: usize,
    /// Population of the immutable demand.
    pub(super) demanded: usize,
}
/// Readiness borrows only captured source fields, never a heap or supplied tile.
pub(super) fn prepare<'a, P: Scalar + 'static, const N: usize>(
    inventory: &'a LcInputInventory<P, N>,
    source: &Source<'a>,
    budget: &mut Budget,
) -> Result<Ready<'a, P, N>, LcProductError> {
    charge(budget, 11)?;
    let cells = source.demand_layout.axis.capacity();
    let words = cells.div_ceil(64);
    if source.identity != source.liveness
        || source.schedule.format != crate::storage::Format::of::<P>()
        || source.schedule.revisions != [1; 4]
        || source.closure.rows.len() != N
        || source.demand_layout.components.extent().rows() != cells
        || source.demand_layout.components.extent().columns() != 1
        || source.demand_mask.layout != source.demand.layout()
        || source.demand_mask.words.len() != words
    {
        return Err(LoanError::WrongLoan.into());
    }
    charge(budget, words)?;
    let mut demanded = 0_usize;
    for (index, word) in source.demand_mask.words.iter().copied().enumerate() {
        if index + 1 == words && cells % 64 != 0 && word >> (cells % 64) != 0 {
            return Err(LoanError::LayoutMismatch.into());
        }
        demanded = demanded
            .checked_add(usize::try_from(word.count_ones()).map_err(|_| LoanError::CountOverflow)?)
            .ok_or(LoanError::CountOverflow)?;
    }
    let bulk = N
        .checked_mul(33)
        .and_then(|n| n.checked_add(cells))
        .and_then(|n| demanded.checked_mul(N)?.checked_mul(4)?.checked_add(n))
        .ok_or(LoanError::CountOverflow)?;
    charge(budget, bulk)?;
    let product = LcInputReaders::captured(inventory, &source.closure.rows)?
        .into_cells_prepaid()?
        .into_product_prepaid(source.schedule.positions)?;
    for cell in 0..cells {
        if bit(&source.demand_mask.words, cell) {
            product.preflight(cell)?;
        }
    }
    Ok(Ready {
        product,
        cells,
        demanded,
    })
}
