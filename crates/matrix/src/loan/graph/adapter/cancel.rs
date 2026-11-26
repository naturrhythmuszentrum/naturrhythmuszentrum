// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit cancellation acts only on graph-held native capabilities.

use super::{
    drive::Event,
    native::Failure,
    node::{Contract, State},
    owner::Owner,
};
use crate::loan::graph::{
    sink::codec::{Encoding, Evidence, Outcome},
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Caller-held host loans require their genuine native return or cancellation receipt.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn cancel_node(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Event<'i, P, N>, Error> {
        let result = self.cancel_inner(handle, heap, account);
        if let Ok(event) = &result {
            self.backing.gate.event(event);
        }
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Fund terminal evidence before entering any original prepaid cancellation path.
    fn cancel_inner(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Event<'i, P, N>, Error> {
        self.authenticate(handle, heap, account)?;
        let slot = handle.slot;
        let state = self.backing.nodes[slot].state;
        if matches!(state, State::Borrowed | State::Published) {
            return Err(Error::InvalidPhase);
        }
        if self.backing.nodes[slot].evidence.is_some() {
            return self.finish(handle, heap, account);
        }
        let prepared = self
            .backing
            .sink
            .prepare(slot, Encoding::Outcome, account)
            .map_err(Error::Delivery)?;
        crate::loan::charge(&mut account.ledger, 2)?; // N exact cancellation owner and saved terminal evidence.
        if let Some(owner) = self.backing.nodes[slot].native.take() {
            let cancelled = owner.cancel(heap, account);
            self.backing.nodes[slot].native = Some(cancelled.owner);
            if let Some(error) = cancelled.failure {
                self.backing.nodes[slot].state = State::Repair;
                return Ok(Event::Refused {
                    node: handle,
                    error,
                    probe: None,
                    control: Ok(()),
                });
            }
        }
        self.backing.nodes[slot].state = State::Cancelled;
        if let Some(pin) = self.backing.nodes[slot].pin.take()
            && let Err((error, pin)) = heap.release_pin(pin, account)
        {
            self.backing.nodes[slot].pin = Some(pin);
            return Ok(Event::Refused {
                node: handle,
                error: Failure::Native(error),
                probe: None,
                control: Ok(()),
            });
        }
        match &mut self.backing.nodes[slot].contract {
            Contract::Input(declaration) => *declaration = None,
            Contract::Mapped { parts, .. } => *parts = None,
            Contract::Omega { .. } | Contract::Delivery { .. } => {}
        }
        let evidence = prepared
            .commit(Evidence::Outcome(Outcome::Cancelled))
            .map_err(|(_, error)| Error::Delivery(error))?;
        self.backing.nodes[slot].evidence = Some(evidence);
        if let Err(error) = self.backing.core.resume(handle, account) {
            return Ok(Event::Terminal {
                node: handle,
                evidence,
                control: Err(error),
            });
        }
        self.finish(handle, heap, account)
    }

    /// A genuine receipt matches the exact issued ticket; a notification cannot manufacture one.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn input_cancelled(
        &mut self,
        handle: Handle,
        receipt: crate::CancellationReceipt,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let result = (|| {
            self.authenticate(handle, heap, account)?;
            let slot = handle.slot;
            let node = &self.backing.nodes[slot];
            if node.state != State::Borrowed
                || node.loan != Some(receipt.ticket)
                || receipt.reason != crate::AbsenceReason::Cancelled
            {
                return Err(LoanError::WrongLoan.into());
            }
            let prepared = self
                .backing
                .sink
                .prepare(slot, Encoding::Outcome, account)
                .map_err(Error::Delivery)?;
            crate::loan::charge(&mut account.ledger, 2)?; // N genuine cancellation and evidence reference.
            let evidence = prepared
                .commit(Evidence::Outcome(Outcome::Cancelled))
                .map_err(|(_, error)| Error::Delivery(error))?;
            self.backing.nodes[slot].state = State::Cancelled;
            self.backing.nodes[slot].native_delivery = true;
            self.backing.nodes[slot].evidence = Some(evidence);
            self.backing.core.resume(handle, account)
        })();
        self.backing.gate.observe(&result);
        result
    }
}
