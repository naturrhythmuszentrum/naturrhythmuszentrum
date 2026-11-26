// SPDX-License-Identifier: AGPL-3.0-only
//! Terminal evidence and native cleanup precede dependency notification.

use super::super::{
    native::Native,
    node::{Contract, State},
    owner::Owner,
};
use super::Event;
use crate::loan::graph::{
    sink::codec::{Encoding, Evidence, Outcome},
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap, LoanError, ProducerOutcome};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Both fixed destinations are funded before zero-attempt traversal or cleanup can enter.
    pub(in crate::loan::graph::adapter) fn exhaust(
        &mut self,
        selected: Handle,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Event<'i, P, N>, Error> {
        let slot = handle.slot;
        if self.backing.core.reservation.backing.nodes[slot].attempts != 0 {
            return Err(Error::InvalidPhase);
        }
        let position = self.backing.nodes[slot]
            .probe_base
            .checked_add(
                usize::try_from(self.backing.nodes[slot].probes)
                    .map_err(|_| LoanError::CountOverflow)?,
            )
            .ok_or(LoanError::CountOverflow)?;
        let (exhaustion, outcome) = self
            .backing
            .sink
            .prepare_pair(
                (position, Encoding::Exhaustion),
                (slot, Encoding::Outcome),
                account,
            )
            .map_err(Error::Delivery)?;
        crate::loan::charge(&mut account.ledger, 3)?; // N terminal state, saved evidence and native continuation.
        if self
            .backing
            .core
            .enter_external(selected, handle, account)?
        {
            return Err(Error::InvalidPhase);
        }
        let evidence = exhaustion
            .commit(Evidence::Exhaustion)
            .map_err(|(_, error)| Error::Delivery(error))?;
        let outcome = outcome
            .commit(Evidence::Outcome(Outcome::DependencyRefused))
            .map_err(|(_, error)| Error::Delivery(error))?;
        self.backing.nodes[slot].evidence = Some(outcome);
        self.backing.nodes[slot].state = State::AttemptsExhausted;
        let control = self
            .settle_native(handle, heap, account)
            .and_then(|()| self.backing.core.resume(handle, account));
        Ok(Event::AttemptsExhausted {
            node: handle,
            evidence,
            outcome,
            control,
        })
    }

    /// A native terminal failure remains owned and can be retried without rerunning execution.
    pub(in crate::loan::graph::adapter) fn settle_native(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let slot = handle.slot;
        crate::loan::charge(&mut account.ledger, 2)?; // N saved continuation and exact producer delivery state.
        if self.backing.nodes[slot].state != State::Published {
            if let Some(owner) = self.backing.nodes[slot].native.take() {
                let cancelled = owner.cancel(heap, account);
                self.backing.nodes[slot].native = Some(cancelled.owner);
                if let Some(error) = cancelled.failure {
                    return Err(Error::Adapter(error));
                }
            }
            if let Some(pin) = self.backing.nodes[slot].pin.take()
                && let Err((error, pin)) = heap.release_pin(pin, account)
            {
                self.backing.nodes[slot].pin = Some(pin);
                return Err(error.into());
            }
            match &mut self.backing.nodes[slot].contract {
                Contract::Input(declaration) => *declaration = None,
                Contract::Mapped { parts, .. } => *parts = None,
                Contract::Omega { .. } | Contract::Delivery { .. } => {}
            }
        }
        self.deliver_native(handle, heap, account)
    }

    /// Native acknowledgement does not acknowledge or destroy the independent graph evidence.
    fn deliver_native(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let node = &mut self.backing.nodes[handle.slot];
        if node.native_delivery {
            return Ok(());
        }
        if node.loan.is_none() {
            node.loan = match &node.native {
                Some(Native::MappedPublished(tile)) => Some(tile.receipt().ticket),
                Some(Native::OmegaPublished(result)) => Some(result.ticket()),
                _ => None,
            };
        }
        if let Some(ticket) = node.loan
            && matches!(
                heap.take_producer_outcome(ticket, account)?,
                ProducerOutcome::Pending
            )
        {
            return Err(Error::InvalidPhase);
        }
        node.native_delivery = true;
        Ok(())
    }
}
