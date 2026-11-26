// SPDX-License-Identifier: AGPL-3.0-only
//! Native mask operations borrow the execution ledger without exposing replacement authority.
use crate::{
    CellAxis, CellFactor, CellLayoutVersion, CellProduct, EntryMap, ExecutionAccount, Extent, Heap,
    Layout, LoanError, MaskError, MaskVersion,
};
impl Heap {
    /// Create a native layout on the same original ledger used by its graph and applications.
    /// # Errors
    /// Preserves native axis, layout, capacity and cumulative resource refusals.
    pub fn create_cell_layout_with_account(
        &mut self,
        axis: CellAxis,
        components: Layout,
        account: &mut ExecutionAccount,
    ) -> Result<CellLayoutVersion, MaskError> {
        self.create_cell_layout(axis, components, &mut account.ledger)
    }
    /// Create a native demand mask without replacing or replenishing the original ledger.
    /// # Errors
    /// Preserves native layout, cell-range and cumulative resource refusals.
    pub fn create_mask_with_account(
        &mut self,
        layout: CellLayoutVersion,
        cells: &[usize],
        account: &mut ExecutionAccount,
    ) -> Result<MaskVersion, MaskError> {
        self.create_mask(layout, cells, &mut account.ledger)
    }
    /// Declare a native mapped copy using the original execution ledger and existing map checks.
    /// # Errors
    /// Preserves foreign/stale selection, incompatible extent and original resource refusals.
    pub fn cell_entry_map_with_account(
        &self,
        source: Extent,
        demand: MaskVersion,
        product: Option<(CellProduct, CellFactor)>,
        account: &mut ExecutionAccount,
    ) -> Result<EntryMap, LoanError> {
        self.cell_entry_map(source, demand, product, &mut account.ledger)
    }
    /// Retire the caller's native demand version after its protected uses have released.
    /// # Errors
    /// Preserves native stale-version, protected-use and cumulative resource refusals.
    pub fn retire_mask_with_account(
        &mut self,
        version: MaskVersion,
        account: &mut ExecutionAccount,
    ) -> Result<(), MaskError> {
        self.retire_mask(version, &mut account.ledger)
    }
    /// Retire the caller's original layout through the unchanged native ownership checks.
    /// # Errors
    /// Preserves native stale-version, protected-use and cumulative resource refusals.
    pub fn retire_cell_layout_with_account(
        &mut self,
        version: CellLayoutVersion,
        account: &mut ExecutionAccount,
    ) -> Result<(), MaskError> {
        self.retire_cell_layout(version, &mut account.ledger)
    }
}
