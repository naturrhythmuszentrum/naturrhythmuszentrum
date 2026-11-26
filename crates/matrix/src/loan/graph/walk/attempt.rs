// SPDX-License-Identifier: AGPL-3.0-only
//! Evidence destinations are prepared before traversal or attempt counts change.

use super::{Capacity, Error, Handle, waiting::ScanReceipt};
use crate::LoanError;
use nrz_bindings::Budget;

/// A caller retains its prepared destination across the entered probe and receipt commit.
pub(crate) trait Attempt {
    /// Fund and reserve the fixed probe or exhaustion destination before native entry.
    fn prepare(
        &mut self,
        handle: Handle,
        exhausted: bool,
        budget: &mut Budget,
    ) -> Result<(), Error>;
    /// Enter the existing capacity calculation on the original ledger.
    fn probe(&mut self, slot: usize, budget: &mut Budget) -> Result<Capacity, LoanError>;
    /// Installation uses the destination already funded by `prepare`.
    fn receipt(&mut self, receipt: ScanReceipt);
}

/// Core controls have no required external delivery owner.
#[cfg(test)]
pub(super) struct Fixture<P, R> {
    /// Existing native capacity fixture.
    pub(super) probe: P,
    /// Fixture observation, never an adapter delivery mechanism.
    pub(super) receipt: R,
}
#[cfg(test)]
impl<P, R> Attempt for Fixture<P, R>
where
    P: FnMut(usize, &mut Budget) -> Result<Capacity, LoanError>,
    R: FnMut(ScanReceipt),
{
    fn prepare(&mut self, _: Handle, _: bool, _: &mut Budget) -> Result<(), Error> {
        Ok(())
    }
    fn probe(&mut self, slot: usize, budget: &mut Budget) -> Result<Capacity, LoanError> {
        (self.probe)(slot, budget)
    }
    fn receipt(&mut self, receipt: ScanReceipt) {
        (self.receipt)(receipt);
    }
}
