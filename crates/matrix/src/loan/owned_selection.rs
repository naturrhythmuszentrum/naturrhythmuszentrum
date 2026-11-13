// SPDX-License-Identifier: AGPL-3.0-only
//! Owned selections retain native maps and the complete original numerical bundle.

use super::{
    BundlePin, ExecutionAccount, LoanError, MaskedTileReader, charge, masked_reader::Projection,
};
use crate::{
    CellFactor, CellProduct, Heap, MaskVersion, MaskView,
    mask::storage::{LayoutRecord, MaskRecord},
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// An independently retained selection over original values and their companions.
///
/// The original source remains pinned even after the caller releases its publication.
/// Selection words and original-index metadata cannot be replaced or retired while
/// this owner lives. No value is copied and no new source or branch theorem is created.
#[derive(Debug)]
pub struct OwnedSelectedTile<P: Scalar> {
    /// Separately accounted numerical use edge with prepaid release.
    pin: BundlePin<P>,
    /// Shared immutable selection; its registry root accounts its allocation once.
    mask: Rc<MaskRecord>,
    /// Shared complete map, including any original-index gather.
    layout: Rc<LayoutRecord>,
    /// Complete native product association or original-cell selection.
    projection: Projection,
}

impl Heap {
    /// Retain a dense or gathered selection independently of the caller's pin and heap borrow.
    ///
    /// Native owners remain charged at their protected registry roots. The additional
    /// numerical pin uses the ordinary pin reservation and cleanup inventory.
    /// # Errors
    /// Refuses stale or foreign occupations, unrelated original sources or accounts,
    /// and insufficient Work or pin capacity before returning an owned selection.
    pub fn retain_selected_tile<P: Scalar + 'static>(
        &mut self,
        pin: &BundlePin<P>,
        selection: MaskVersion,
        account: &mut ExecutionAccount,
    ) -> Result<OwnedSelectedTile<P>, LoanError> {
        self.retain_projected_tile(pin, selection, Projection::Original, account)
    }
    /// Retain original components as read-only views over one checked Cartesian factor.
    ///
    /// Dense and gathered selections retain the original product axis. Repeated reads depend on
    /// the same original source and companions; native projection proves no physical
    /// independence, numerical initialization or kernel broadcast law.
    /// # Errors
    /// Refuses unrelated factors or product axes, stale owners,
    /// foreign accounts and insufficient Work or pin capacity.
    pub fn retain_product_tile<P: Scalar + 'static>(
        &mut self,
        pin: &BundlePin<P>,
        selection: MaskVersion,
        product: CellProduct,
        factor: CellFactor,
        account: &mut ExecutionAccount,
    ) -> Result<OwnedSelectedTile<P>, LoanError> {
        self.retain_projected_tile(
            pin,
            selection,
            Projection::Product(product, factor),
            account,
        )
    }
    /// Both admitted maps share one reservation and original bundle retention path.
    fn retain_projected_tile<P: Scalar + 'static>(
        &mut self,
        pin: &BundlePin<P>,
        selection: MaskVersion,
        projection: Projection,
        account: &mut ExecutionAccount,
    ) -> Result<OwnedSelectedTile<P>, LoanError> {
        let reader = pin.historical_reader(account)?;
        let mask = self.mask(selection, selection.layout())?;
        let _checked = MaskedTileReader::bind_projected(reader, mask, projection, account)?;
        let (mask, layout) = account.ledger.with_work_phase(
            super::FAMILY,
            "selected owner retention",
            |budget| {
                // Two owner lookups, two shared acquisitions and their two prepaid releases.
                charge(budget, 6)?;
                let mask = Rc::clone(self.masks.masks.get(selection.key)?.owner()?);
                let layout = Rc::clone(self.masks.layouts.get(selection.layout().key)?.owner()?);
                Ok::<_, LoanError>((mask, layout))
            },
        )?;
        let pin = self.retain_pin(pin, account)?;
        Ok(OwnedSelectedTile {
            pin,
            mask,
            layout,
            projection,
        })
    }
}

impl<P: Scalar + 'static> OwnedSelectedTile<P> {
    /// Borrow selected original components after authenticating the retained bundle.
    /// # Errors
    /// Refuses another execution account, exhausted Work or invalid source authority.
    pub fn reader(
        &self,
        account: &mut ExecutionAccount,
    ) -> Result<MaskedTileReader<'_, P>, LoanError> {
        let source = self.pin.historical_reader(account)?;
        MaskedTileReader::bind_projected(
            source,
            MaskView {
                words: &self.mask.words,
                axis: self.layout.axis,
                components: &self.layout.components,
                gather: self.layout.gather.as_ref(),
            },
            self.projection,
            account,
        )
    }
}
