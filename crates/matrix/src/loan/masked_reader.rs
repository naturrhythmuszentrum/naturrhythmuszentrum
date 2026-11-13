// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed selected views retain the original numerical pin and immutable native map.

use super::{ExecutionAccount, HostChannel, LoanError, TileReader, charge};
use crate::{CellFactor, CellProduct, MaskView};
use nrz_scalar::Scalar;

/// Native cell projection retains original source dependence without new descriptors.
#[derive(Clone, Copy, Debug)]
pub(super) enum Projection {
    /// Dense or original-index gathered selection.
    Original,
    /// One checked factor of an original or gathered Cartesian selection.
    Product(CellProduct, CellFactor),
}
impl Projection {
    /// Project one already selected product cell to its original factor coordinate.
    fn original(self, cell: usize) -> Result<usize, LoanError> {
        match self {
            Self::Original => Ok(cell),
            Self::Product(product, factor) => Ok(product.coordinates(cell)?[factor.index()]),
        }
    }
}
/// A checked cell selection borrowing a pinned value and its complete companion bundle.
///
/// Components retain their original source meaning. The mask supplies selection and
/// cell correspondence, not numerical initialization or a mathematical branch theorem.
/// The view allocates nothing and cannot outlive either its source reader or mask.
pub struct MaskedTileReader<'a, P: Scalar> {
    /// Existing reader borrow keeps its complete bundle pin alive.
    source: TileReader<'a, P>,
    /// Immutable selected words and any retained original-index map.
    selection: MaskView<'a>,
    /// Checked original-source projection, fixed at binding.
    projection: Projection,
}
impl<P: Scalar> std::fmt::Debug for MaskedTileReader<'_, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MaskedTileReader")
            .field("source", &self.source)
            .field("selection", &self.selection)
            .field("projection", &self.projection)
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
        MaskedTileReader::bind(self.reborrow(), selection, account)
    }
}
impl<'a, P: Scalar + 'static> MaskedTileReader<'a, P> {
    /// Shared association check for borrowed and independently retained selections.
    pub(super) fn bind(
        source: TileReader<'a, P>,
        selection: MaskView<'a>,
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        Self::bind_projected(source, selection, Projection::Original, account)
    }
    /// Validate the complete source association once for an owned mapped selection.
    pub(super) fn bind_projected(
        source: TileReader<'a, P>,
        selection: MaskView<'a>,
        projection: Projection,
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        account.authenticate(source.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "masked reader binding", |budget| {
                charge(budget, 2)?;
                match projection {
                    Projection::Original => {
                        let axis = if let Some(map) = selection.gather() {
                            if map.source().layout() != source.bundle.identity.layout {
                                return Err(LoanError::LayoutMismatch);
                            }
                            map.original_axis()
                        } else {
                            selection.axis()
                        };
                        if axis != source.bundle.declaration.axis {
                            return Err(LoanError::LayoutMismatch);
                        }
                    }
                    Projection::Product(product, factor) => {
                        charge(budget, 1)?;
                        let axis = selection
                            .gather()
                            .map_or_else(|| selection.axis(), crate::CellGather::original_axis);
                        if axis != product.axis()
                            || source.bundle.declaration.axis != product.factors()[factor.index()]
                        {
                            return Err(LoanError::LayoutMismatch);
                        }
                    }
                }
                Ok(Self {
                    source,
                    selection,
                    projection,
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
                if matches!(self.projection, Projection::Product(..)) {
                    charge(budget, 1)?;
                }
                let original = self.projection.original(original)?;
                let owner = self.source.selected_owner(channel, original, component)?;
                charge(budget, 1)?;
                owner.matrix.read(
                    self.source
                        .bundle
                        .layout
                        .components
                        .index(original, component)?,
                )
            })
    }
}
