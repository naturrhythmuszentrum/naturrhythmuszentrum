// SPDX-License-Identifier: AGPL-3.0-only
//! Outside readers retain only admitted evidence storage and its account debit.

use super::{Backing, Error, ExecutionAccount, Handle, Sink, Status, charge};
use crate::LoanError;
use std::rc::Rc;

/// Non-cloneable, individually admitted edge; its destruction never borrows a ledger.
#[derive(Debug)]
pub struct Witness {
    /// No strong reference to the graph owner exists.
    backing: Rc<Backing>,
    /// Original record occupation; acknowledgement does not erase witnessed evidence.
    handle: Handle,
    /// Separately reserved witness occupancy slot.
    slot: usize,
}
impl Sink {
    /// Fund the edge and its final fixed updates before an outside reader can escape.
    pub(crate) fn witness(
        &self,
        handle: Handle,
        account: &mut ExecutionAccount,
    ) -> Result<Witness, Error> {
        self.authenticate(handle, account)?;
        let mut vacant = None;
        for (slot, used) in self.backing.witnesses.iter().enumerate() {
            charge(&mut account.ledger, 1)?; // M actual inspected witness slot.
            if used.get() == 0 {
                vacant = Some(slot);
                break;
            }
        }
        let slot = vacant.ok_or(Error::Full)?;
        // T prepared edge, M two occupancy updates, P edge and two final updates.
        charge(&mut account.ledger, 6)?;
        let record = &self.backing.slots[handle.slot];
        self.backing.witnesses[slot].set(1);
        record.witnesses.set(record.witnesses.get() + 1);
        Ok(Witness {
            backing: Rc::clone(&self.backing),
            handle,
            slot,
        })
    }
    /// Status is an authenticated observation, not numerical completion authority.
    pub(crate) fn status(&self, account: &mut ExecutionAccount) -> Result<Status, Error> {
        account.authenticate(self.backing.account)?;
        charge(&mut account.ledger, 2)?; // H entry + M fixed status.
        Ok(self.backing.status.get())
    }
}
impl Witness {
    /// Abandonment stays visible after the graph edge disappears.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn status(&self, account: &mut ExecutionAccount) -> Result<Status, Error> {
        account.authenticate(self.backing.account)?;
        charge(&mut account.ledger, 2)?;
        Ok(self.backing.status.get())
    }
    /// Inspect immutable evidence while its admitted retaining edge prevents slot reuse.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn inspect<T>(
        &self,
        account: &mut ExecutionAccount,
        inspect: impl FnOnce(&[u64], usize) -> T,
    ) -> Result<T, Error> {
        account.authenticate(self.backing.account)?;
        charge(&mut account.ledger, 4)?; // H + M generation, destination offset and length.
        let record = &self.backing.slots[self.handle.slot];
        if record.generation.get() != self.handle.generation {
            return Err(LoanError::StaleOccupation.into());
        }
        let words = self
            .backing
            .words
            .try_borrow()
            .map_err(|_| Error::Borrowed)?;
        let length = record.length.get();
        let end = record.offset + length.div_ceil(8);
        Ok(inspect(&words[record.offset..end], length))
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        self.backing.witnesses[self.slot].set(0);
        let record = &self.backing.slots[self.handle.slot];
        record.witnesses.set(record.witnesses.get() - 1);
    }
}
