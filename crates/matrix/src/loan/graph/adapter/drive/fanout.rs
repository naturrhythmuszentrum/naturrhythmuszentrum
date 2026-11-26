// SPDX-License-Identifier: AGPL-3.0-only
//! Additional consumers retain in recorded edge order before the first pin can move.

use super::super::{
    gate,
    input::failed_probe,
    native::Failure,
    node::State,
    owner::{Borrowed, Owner},
    refusal,
};
use super::Event;
use crate::loan::graph::{
    sink::codec::{Capacity, Encoding, Evidence},
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// A saved edge cursor prevents duplicate retention after either refusal or interruption.
    pub(in crate::loan::graph::adapter) fn fanout(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        let graph = self.resident_bytes(others, heap, account)?;
        let edge = self.backing.nodes[handle.slot].fanout;
        if edge == usize::MAX {
            crate::loan::charge(&mut account.ledger, 2)?; // N producer and first consumer request parts.
            let first = self.backing.core.reservation.backing.nodes[handle.slot].links[0];
            let pin = self.backing.nodes[handle.slot].pin.take();
            if first != usize::MAX {
                let consumer = self.backing.core.reservation.backing.edges[first].endpoints[1].slot;
                if self.backing.nodes[consumer].pin.is_none()
                    && !super::terminal(self.backing.nodes[consumer].state)
                {
                    self.backing.nodes[consumer].pin = pin;
                }
            }
            return self.finish(handle, heap, account);
        }
        crate::loan::charge(&mut account.ledger, 3)?; // E actual outgoing edge, N producer and consumer.
        let edge = self.backing.core.reservation.backing.edges[edge];
        let consumer = edge.endpoints[1].slot;
        let target = self.backing.core.handle(consumer, account)?;
        if super::terminal(self.backing.nodes[consumer].state) {
            crate::loan::charge(&mut account.ledger, 1)?; // N saved fan-out cursor skips a settled original consumer.
            self.backing.nodes[handle.slot].fanout = edge.links[1];
            return Ok(Event::Control);
        }
        let remaining = self.backing.core.reservation.backing.nodes[consumer].attempts;
        if remaining == 0 {
            let mut event = self.exhaust(handle, target, heap, account)?;
            self.backing.nodes[handle.slot].fanout = edge.links[1];
            self.backing.nodes[consumer].waiting_for = None;
            if let Event::AttemptsExhausted { control, .. } = &mut event
                && control.is_ok()
            {
                *control = self.backing.core.resume(handle, account);
            }
            return Ok(event);
        }
        let (next_probe, position) = self.backing.nodes[consumer].next_probe()?;
        let prepared = self
            .backing
            .sink
            .prepare(position, Encoding::Probe, account)
            .map_err(Error::Delivery)?
            .at_native_state(11);
        crate::loan::charge(&mut account.ledger, 2)?; // N saved fan-out and consumer pin records.
        if !self.backing.core.enter_external(handle, target, account)? {
            return Err(Error::InvalidPhase);
        }
        self.backing.nodes[consumer].probes = next_probe;
        let pin = self.backing.nodes[handle.slot]
            .pin
            .as_ref()
            .ok_or(LoanError::WrongLoan)?;
        let retained = gate::run(&self.backing.gate, graph, heap, account, |heap, account| {
            heap.retain_pin(pin, account)
        })
        .and_then(std::convert::identity);
        match retained {
            Ok(pin) => {
                let evidence = prepared
                    .commit(Evidence::Probe(Capacity::Fits))
                    .map_err(|(_, error)| Error::Delivery(error))?;
                self.backing.nodes[consumer].pin = Some(pin);
                self.backing.nodes[consumer].state = State::Pending;
                self.backing.nodes[consumer].waiting_for = None;
                self.backing.nodes[handle.slot].fanout = edge.links[1];
                let control = self
                    .backing
                    .core
                    .resume(target, account)
                    .and_then(|()| self.backing.core.resume(handle, account));
                Ok(Event::Advanced {
                    node: target,
                    probe: evidence,
                    control,
                })
            }
            Err(error) => {
                let class = self.backing.gate.loan(&error);
                let evidence = prepared
                    .commit(failed_probe(class))
                    .map_err(|(_, error)| Error::Delivery(error))?;
                Ok(self.retention_refused(
                    handle,
                    target,
                    edge.links[1],
                    (error, class, evidence),
                    account,
                ))
            }
        }
    }
    /// A failed additional pin retains the original cursor and the consumer's attempt identity.
    fn retention_refused(
        &mut self,
        handle: Handle,
        target: Handle,
        next: usize,
        failure: (LoanError, refusal::Class, crate::loan::graph::sink::Handle),
        account: &mut ExecutionAccount,
    ) -> Event<'i, P, N> {
        let (error, class, evidence) = failure;
        if class == refusal::Class::Terminal {
            self.backing.nodes[target.slot].state = State::Refused;
            self.backing.nodes[handle.slot].fanout = next;
        }
        let control = match class {
            refusal::Class::WorkStopped => self.backing.core.stop_native(),
            refusal::Class::Terminal => {
                self.backing.nodes[target.slot].waiting_for = None;
                self.backing
                    .core
                    .resume(target, account)
                    .and_then(|()| self.backing.core.resume(handle, account))
            }
            refusal::Class::BytesWait { .. }
            | refusal::Class::ElementsWait
            | refusal::Class::AllocationWait => {
                self.backing.nodes[target.slot].state = State::ResourceWait;
                self.backing.nodes[target.slot].waiting_for = Some(handle.slot);
                self.backing
                    .core
                    .wait_native(target, account)
                    .and_then(|()| self.backing.core.park(handle, account))
            }
            refusal::Class::OwnershipWait | refusal::Class::Repair => {
                self.backing.nodes[target.slot].state = if class == refusal::Class::Repair {
                    State::Repair
                } else {
                    State::OwnershipWait
                };
                self.backing.nodes[target.slot].waiting_for = Some(handle.slot);
                self.backing.core.park(handle, account)
            }
        };
        Event::Refused {
            node: target,
            error: Failure::Native(error),
            probe: Some(evidence),
            control,
        }
    }
}
