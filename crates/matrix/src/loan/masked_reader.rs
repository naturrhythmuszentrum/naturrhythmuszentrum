// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed selected views retain the original numerical pin and immutable native map.

use super::{ExecutionAccount, HostChannel, LoanError, TileReader, charge};
use crate::MaskView;
use nrz_scalar::Scalar;

/// A checked cell selection borrowing a pinned value and its complete companion bundle.
///
/// Components retain their original source meaning. The mask supplies selection and
/// cell correspondence, not numerical initialization or a mathematical branch theorem.
/// The view allocates nothing and cannot outlive either its source reader or mask.
pub struct MaskedTileReader<'a, P: Scalar> {
    /// Existing reader borrow keeps its complete bundle pin alive.
    source: &'a TileReader<'a, P>,
    /// Immutable selected words and any retained original-index map.
    selection: MaskView<'a>,
}
impl<P: Scalar> std::fmt::Debug for MaskedTileReader<'_, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MaskedTileReader")
            .field("source", &self.source)
            .field("selection", &self.selection)
            .finish()
    }
}
impl<P: Scalar + 'static> TileReader<'_, P> {
    /// Check a selection once before any selected component reads.
    ///
    /// Dense selections share the original ordered axis. Compacted selections must
    /// retain this exact source layout occupation, including through nested compaction.
    /// Required value and companion initialization remains checked on every load.
    /// # Errors
    /// Refuses another account, unrelated axes or gather origins, and exhausted Work.
    pub fn masked_view<'a>(
        &'a self,
        selection: MaskView<'a>,
        account: &mut ExecutionAccount,
    ) -> Result<MaskedTileReader<'a, P>, LoanError> {
        account.authenticate(self.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "masked reader binding", |budget| {
                charge(budget, 2)?;
                let axis = if let Some(map) = selection.gather() {
                    if map.source().layout() != self.bundle.identity.layout {
                        return Err(LoanError::LayoutMismatch);
                    }
                    map.original_axis()
                } else {
                    selection.axis()
                };
                if axis != self.bundle.declaration.axis {
                    return Err(LoanError::LayoutMismatch);
                }
                Ok(MaskedTileReader {
                    source: self,
                    selection,
                })
            })
    }
}
impl<P: Scalar + 'static> MaskedTileReader<'_, P> {
    /// Logical selected-layout capacity, including its unfilled cells.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.selection.axis().capacity()
    }

    /// Read one selected original component, preserving the original companion targets.
    /// # Errors
    /// Refuses holes before numerical loads, absent companions, bounds, foreign accounts
    /// and exhausted cumulative Work. A native selection cannot fill an input hole.
    pub fn read_component(
        &self,
        channel: HostChannel,
        cell: usize,
        component: usize,
        account: &mut ExecutionAccount,
    ) -> Result<P, LoanError> {
        account.authenticate(self.source.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "masked selected load", |budget| {
                charge(budget, 2)?;
                if !self.selection.contains(cell)? {
                    return Err(LoanError::CoverageMismatch);
                }
                let original = match self.selection.gather() {
                    Some(map) => *map.indices().get(cell).ok_or(LoanError::LayoutMismatch)?,
                    None => cell,
                };
                let owner = self.source.selected_owner(channel, original, component)?;
                charge(budget, 1)?;
                owner
                    .matrix
                    .read(original + self.source.capacity() * component)
            })
    }
}
