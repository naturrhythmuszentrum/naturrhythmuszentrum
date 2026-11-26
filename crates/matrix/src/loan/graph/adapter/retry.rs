// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit epochs and original-closure repair never replace attempts or refill Work.

use super::{
    drive::Event,
    native::Native,
    node::{Contract, State},
    owner::{Borrowed, Owner},
};
use crate::loan::graph::walk::{Error, Handle};
use crate::{ExecutionAccount, Heap};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Capture one finite waiting-set watermark; newly appended work belongs to a later epoch.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn begin_capacity_epoch(&mut self, account: &mut ExecutionAccount) -> Result<(), Error> {
        let result = (|| {
            account.authenticate(self.backing.account)?;
            if self.backing.gate.stopped() {
                return Err(Error::WorkStopped);
            }

            self.backing.core.begin_capacity_epoch(account)
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// Cancel only the saved waiting scan; membership, attempts and prior Work remain spent.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn cancel_capacity_scan(&mut self, account: &mut ExecutionAccount) -> Result<(), Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.backing.core.cancel_capacity_scan(account);
        self.backing.gate.observe(&result);
        result
    }

    /// A waiting retry enters its real native phase and preserves its full result beside the probe.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn drive_wait_one(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.drive_wait_one_inner(heap, account, others);
        if let Ok(event) = &result {
            self.backing.gate.event(event);
        }
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn drive_wait_one_inner(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        let Some(handle) = self.backing.core.waiting_native(account)? else {
            return Ok(Event::Control);
        };
        self.authenticate(handle, heap, account)?;
        crate::loan::charge(&mut account.ledger, 1)?; // N original saved wait continuation.
        let node = &self.backing.nodes[handle.slot];
        if node.state != State::ResourceWait {
            return Err(Error::InvalidPhase);
        }
        if let Some(producer) = node.waiting_for {
            let producer = self.backing.core.handle(producer, account)?;
            return self.fanout(producer, heap, account, others);
        }
        if matches!(node.contract, Contract::Input(_)) {
            return Ok(Event::Input {
                node: handle,
                outcome: self.request_input(handle, heap, account, others)?,
            });
        }
        self.dispatch(handle, heap, account, others)
    }

    /// Repair retries only the still-owned original closure; a started failed traversal is cancel-only.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn retry_original(
        &mut self,
        handle: Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.retry_original_inner(handle, heap, account);
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn retry_original_inner(
        &mut self,
        handle: Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        self.authenticate(handle, heap, account)?;
        crate::loan::charge(&mut account.ledger, 1)?; // N saved original continuation and disposition.
        let node = &self.backing.nodes[handle.slot];
        if !matches!(node.state, State::Repair | State::OwnershipWait) {
            return Err(Error::InvalidPhase);
        }
        if matches!(node.native, Some(Native::OmegaFailed(_))) {
            return Err(Error::InvalidPhase);
        }
        if let Some(producer) = node.waiting_for {
            let producer = self.backing.core.handle(producer, account)?;
            self.backing.core.resume(producer, account)?;
        } else if !matches!(node.contract, Contract::Input(_)) {
            self.backing.core.resume(handle, account)?;
        }
        self.backing.nodes[handle.slot].state = State::Pending;
        Ok(())
    }
}
