// SPDX-License-Identifier: AGPL-3.0-only
//! Complete positional inventory authority accompanies a borrowed current-reader set.

mod cells;
pub(in crate::loan) use cells::LcKernelStage;
pub use cells::{
    LcFrequencyPrelude, LcFrequencyTarget, LcInputCell, LcInputCells, LcInputProduct,
    LcInputReading, LcProductCell, LcProductError, LcQuantityDemand, LcRequestedCell,
    LcRequestedState, LcRequestedValues,
};

use super::{LcInputInventory, LcShapeAxes, LcShapeTile};
use crate::{ExecutionAccount, Heap, LoanError, TileReader};
use nrz_scalar::Scalar;

/// Current readers for every original position of one complete supplied-input inventory.
///
/// Membership does not merge masks or establish joint physical feasibility. Each reader
/// retains its own qualified coverage and checked account; callers must check required cells.
#[derive(Debug)]
pub struct LcInputReaders<'a, P: Scalar, const N: usize> {
    /// Original complete inventory, authenticated by exact schema object identity.
    inventory: &'a LcInputInventory<P, N>,
    /// Complete common instance/sample/branch/product identities.
    axes: LcShapeAxes,
    /// Every slot is filled before publication; no native owner is allocated.
    readers: [Option<TileReader<'a, P>>; N],
}
impl<P: Scalar + 'static, const N: usize> LcInputInventory<P, N> {
    /// Authenticate exactly one current member tile per original supplied-constraint position.
    ///
    /// The borrowed heap and tiles protect the current read scope. Refusal leaves all
    /// supplied tiles with the caller and performs no numerical reads or allocations.
    /// # Errors
    /// Refuses missing/extra, standalone, foreign-inventory or reordered members,
    /// changed roles, foreign accounts/heaps, stale occupations and exhausted Work.
    pub fn readers<'a>(
        &'a self,
        heap: &'a Heap,
        tiles: &[&'a LcShapeTile<'_, P>],
        account: &mut ExecutionAccount,
    ) -> Result<LcInputReaders<'a, P, N>, LoanError> {
        let axes = self.membership(tiles, account)?;
        let mut readers = std::array::from_fn(|_| None);
        for (slot, tile) in readers.iter_mut().zip(tiles) {
            *slot = Some(tile.current_reader(heap, account)?);
        }
        Ok(LcInputReaders {
            inventory: self,
            axes,
            readers,
        })
    }

    /// The whole metadata gate precedes any per-member current native reader admission.
    pub(super) fn membership(
        &self,
        tiles: &[&LcShapeTile<'_, P>],
        account: &mut ExecutionAccount,
    ) -> Result<LcShapeAxes, LoanError> {
        if let Some(member) = self.input(0) {
            account.authenticate(member.account)?;
        }
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC inventory membership", |budget| {
                let work = N
                    .checked_mul(2)
                    .and_then(|work| work.checked_add(1))
                    .ok_or(LoanError::CountOverflow)?;
                crate::loan::charge(budget, work)?;
                if tiles.len() != N {
                    return Err(LoanError::UnsupportedContract);
                }
                self.membership_prepaid(tiles)
            })
    }

    /// Reuse the public metadata gate after the bridge has prepaid its row agreement.
    pub(in crate::loan::lc_shape) fn membership_prepaid(
        &self,
        tiles: &[&LcShapeTile<'_, P>],
    ) -> Result<LcShapeAxes, LoanError> {
        let axes = tiles.first().ok_or(LoanError::UnsupportedContract)?.axes();
        for (index, tile) in tiles.iter().enumerate() {
            let expected = self.input(index).ok_or(LoanError::TargetMismatch)?;
            if !std::ptr::eq(expected, tile.schema()) {
                return Err(LoanError::TargetMismatch);
            }
            if tile.axes() != axes {
                return Err(LoanError::LayoutMismatch);
            }
        }
        Ok(axes)
    }

    /// Build readers after the bridge's count check and positional prepayment.
    /// The original candidate residence remains reserved by the caller.
    pub(in crate::loan::lc_shape) fn readers_prepaid<'a>(
        &'a self,
        heap: &'a Heap,
        tiles: &[&'a LcShapeTile<'_, P>],
        account: u64,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<LcInputReaders<'a, P, N>, LoanError> {
        let axes = self.membership_prepaid(tiles)?;
        let mut readers = std::array::from_fn(|_| None);
        for (slot, tile) in readers.iter_mut().zip(tiles) {
            *slot = Some(tile.current_reader_on(heap, account, budget)?);
        }
        Ok(LcInputReaders {
            inventory: self,
            axes,
            readers,
        })
    }
}
impl<P: Scalar, const N: usize> LcInputReaders<'_, P, N> {
    /// The exact complete inventory authenticated at admission, not inferred from tile metadata.
    #[must_use]
    pub const fn inventory(&self) -> &LcInputInventory<P, N> {
        self.inventory
    }
    /// Shared original native roles, including all ordered product identities.
    #[must_use]
    pub const fn axes(&self) -> LcShapeAxes {
        self.axes
    }
    /// Borrow the admitted reader for one original position; holes retain their own masks.
    #[must_use]
    pub fn reader(&self, index: usize) -> Option<&TileReader<'_, P>> {
        self.readers.get(index).and_then(Option::as_ref)
    }
}

impl<'a, P: Scalar + 'static, const N: usize> LcInputReaders<'a, P, N> {
    /// Borrow the exact captured closure after readiness prepays its complete row inventory.
    pub(in crate::loan) fn captured(
        inventory: &'a LcInputInventory<P, N>,
        rows: &'a [crate::loan::lc_origin::LcSourceRow],
    ) -> Result<Self, LoanError> {
        if rows.len() != N {
            return Err(LoanError::TargetMismatch);
        }
        let axes = rows.first().ok_or(LoanError::UnsupportedContract)?.axes;
        let mut readers = std::array::from_fn(|_| None);
        for (index, (slot, row)) in readers.iter_mut().zip(rows).enumerate() {
            let original = inventory.input(index).ok_or(LoanError::TargetMismatch)?;
            if row.position != index
                || row.schema != original.specialization_key()
                || row.axes != axes
            {
                return Err(LoanError::TargetMismatch);
            }
            let reader = crate::loan::reader::checked_record_reader(&row.pin)?;
            let mapped = reader.bundle;
            let source = mapped.origin.lc_source()?;
            if mapped.identity != row.mapped
                || source.identity != row.source
                || source.projection != row.projection
                || source.identity.layout != row.layout
                || source.declaration.axis != row.axes.instances
                || reader.account != original.account
            {
                return Err(LoanError::TargetMismatch);
            }
            if !same_symbols(&row.symbols[0], &source.declaration.symbols)
                || !same_symbols(&row.symbols[1], &mapped.declaration.symbols)
                || mapped.qualified != row.companions.map(Some)
            {
                return Err(LoanError::TargetMismatch);
            }
            *slot = Some(reader);
        }
        Ok(Self {
            inventory,
            axes,
            readers,
        })
    }
}
/// Compare the exact three original symbols without cloning their owning edges.
fn same_symbols(left: &nrz_bindings::BindingSymbols, right: &nrz_bindings::BindingSymbols) -> bool {
    left.value() == right.value() && left.bound() == right.bound() && left.error() == right.error()
}
