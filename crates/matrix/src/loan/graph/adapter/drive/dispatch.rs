// SPDX-License-Identifier: AGPL-3.0-only
//! Each entered call preserves its exact returned owner before further graph control.

use super::super::{
    gate,
    input::failed_probe,
    native::{Failure, Native, Transition},
    node::{Contract, Node, State},
    owner::{Borrowed, Owner},
    refusal::Class,
};
use super::{Event, phase};
use crate::loan::graph::{
    sink::{
        self,
        codec::{Capacity, Encoding, Evidence},
    },
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap, LoanError, PointwiseRequest};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Request assembly transfers only already-owned pins and original checked request parts.
    fn assemble(&mut self, slot: usize, account: &mut ExecutionAccount) -> Result<(), Error> {
        if self.backing.nodes[slot].native.is_some() {
            return Ok(());
        }
        let node = &mut self.backing.nodes[slot];
        let Contract::Mapped {
            schema,
            axes,
            parts,
            ..
        } = &mut node.contract
        else {
            return Ok(());
        };
        if parts.is_none() || node.pin.is_none() {
            return Err(Error::InvalidPhase);
        }
        crate::loan::charge(&mut account.ledger, 1)?; // N one complete request-part move.
        let parts = parts.take().ok_or(LoanError::WrongLoan)?;
        let input = node.pin.take().ok_or(LoanError::WrongLoan)?;
        node.native = Some(Native::MappedRequest {
            schema,
            axes: *axes,
            request: PointwiseRequest {
                operation: parts.operation,
                input,
                destination: parts.destination,
                demand: parts.demand,
                limits: parts.limits,
            },
        });
        Ok(())
    }

    /// Paired preparation protects consuming correspondence errors without a variable error queue.
    pub(in crate::loan::graph::adapter) fn dispatch(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        if self.backing.core.reservation.backing.nodes[handle.slot].attempts == 0 {
            return self.exhaust(handle, handle, heap, account);
        }
        self.assemble(handle.slot, account)?;
        let graph = self.resident_bytes(others, heap, account)?;
        let slot = handle.slot;
        let selected = self.backing.nodes[slot].native.as_ref().map_or(10, phase);
        let (next_probe, position) = self.backing.nodes[slot].next_probe()?;
        let encoding = Encoding::Probe;
        let (probe, returned) = if selected == 4 {
            let (probe, returned) = self
                .backing
                .sink
                .prepare_pair((position, encoding), (slot, Encoding::Refusal), account)
                .map_err(Error::Delivery)?;
            (
                probe.at_native_state(selected),
                Some(returned.at_native_state(selected)),
            )
        } else {
            (
                self.backing
                    .sink
                    .prepare(position, encoding, account)
                    .map_err(Error::Delivery)?
                    .at_native_state(selected),
                None,
            )
        };
        crate::loan::charge(&mut account.ledger, 2)?; // N saved native owner and H native-stop state.
        if !self.backing.core.enter_external(handle, handle, account)? {
            return Err(Error::InvalidPhase);
        }
        self.backing.nodes[slot].probes = next_probe;
        let mut original = self.backing.nodes[slot].native.take();
        let transition = gate::run(&self.backing.gate, graph, heap, account, |heap, account| {
            call(
                &mut original,
                &self.backing.nodes,
                slot,
                self.backing.registry,
                heap,
                account,
            )
        });
        let (owner, failure) = match transition {
            Ok(Ok(transition)) => (Some(transition.owner), transition.failure),
            Ok(Err(error)) | Err(error) => (original, Some(Failure::Native(error))),
        };
        let class = failure
            .as_ref()
            .map(|error| self.backing.gate.classify(error));
        let evidence = probe
            .commit(class.map_or(Evidence::Probe(Capacity::Fits), failed_probe))
            .map_err(|(_, error)| Error::Delivery(error));
        let (owner, failure) = match (returned, owner, failure) {
            (
                Some(returned),
                Some(Native::OmegaDeclaration(declaration)),
                Some(Failure::Preparation(error)),
            ) => {
                let returned = returned.return_refusal(declaration, error);
                self.backing.nodes[slot].returned_evidence =
                    returned.receipt.as_ref().ok().copied();
                self.backing.nodes[slot].state = if class == Some(Class::Terminal) {
                    State::Refused
                } else {
                    State::Returned
                };
                let control = if class == Some(Class::WorkStopped) {
                    self.backing.core.stop_native()
                } else if class == Some(Class::Terminal) {
                    self.backing.core.resume(handle, account)
                } else {
                    self.backing.core.park(handle, account)
                };
                return Ok(Event::PreparationRefused {
                    node: handle,
                    declaration: returned.capability,
                    error: returned.error,
                    probe: evidence,
                    evidence: returned.receipt,
                    control,
                });
            }
            (returned, owner, failure) => {
                drop(returned);
                (owner, failure)
            }
        };
        self.backing.nodes[slot].native = owner;
        self.preserved(handle, failure, class, evidence, heap, account)
    }
    /// Delivery failure cannot discard a native result or an original structured cause.
    fn preserved(
        &mut self,
        handle: Handle,
        failure: Option<Failure>,
        class: Option<Class>,
        evidence: Result<sink::Handle, Error>,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Event<'i, P, N>, Error> {
        let published = matches!(
            self.backing.nodes[handle.slot].native,
            Some(Native::MappedPublished(_) | Native::OmegaPublished(_))
        );
        if let Some(error) = failure {
            let (probe, control) = match evidence {
                Ok(probe) => (
                    Some(probe),
                    self.reject_native(handle, class.unwrap_or(Class::Terminal), heap, account),
                ),
                Err(error) => {
                    self.backing.nodes[handle.slot].state = State::Repair;
                    (None, Err(error))
                }
            };
            Ok(Event::Refused {
                node: handle,
                error,
                probe,
                control,
            })
        } else {
            self.backing.nodes[handle.slot].state = if published {
                State::Published
            } else {
                State::Pending
            };
            Ok(Event::Advanced {
                node: handle,
                probe: evidence?,
                control: self.backing.core.resume(handle, account),
            })
        }
    }

    /// Started failed execution remains cancel-only even when the underlying cause was temporary.
    fn reject_native(
        &mut self,
        handle: Handle,
        class: Class,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let slot = handle.slot;
        if class == Class::WorkStopped {
            return self.backing.core.stop_native();
        }
        if matches!(
            self.backing.nodes[slot].native,
            Some(Native::OmegaFailed(_))
        ) {
            self.backing.nodes[slot].state = State::Repair;
            return self.backing.core.park(handle, account);
        }
        self.backing.nodes[slot].state = match class {
            Class::AllocationWait | Class::ElementsWait | Class::BytesWait { .. } => {
                State::ResourceWait
            }
            Class::OwnershipWait => State::OwnershipWait,
            Class::Repair => State::Repair,
            Class::Terminal => State::Refused,
            Class::WorkStopped => return Ok(()),
        };
        if class == Class::Terminal {
            if let Some(owner) = self.backing.nodes[slot].native.take() {
                let cancelled = owner.cancel(heap, account);
                self.backing.nodes[slot].native = Some(cancelled.owner);
                if let Some(error) = cancelled.failure {
                    self.backing.nodes[slot].state = State::Repair;
                    return Err(Error::Adapter(error));
                }
            }
            Ok(())
        } else if matches!(
            class,
            Class::BytesWait { .. } | Class::ElementsWait | Class::AllocationWait
        ) {
            self.backing.core.wait_native(handle, account)
        } else {
            self.backing.core.park(handle, account)
        }
    }
}

/// Native publication is the only source of a sealed result or mapped filled scope.
fn call<'i, P: Scalar + 'static, const N: usize>(
    original: &mut Option<Native<'i, P, N>>,
    nodes: &[Node<'i, P, N>],
    slot: usize,
    registry: &nrz_bindings::Registry,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<Transition<'i, P, N>, LoanError> {
    match &nodes[slot].contract {
        Contract::Mapped { .. } => Ok(original
            .take()
            .ok_or(LoanError::WrongLoan)?
            .advance_mapped(heap, account)),
        Contract::Omega {
            sources,
            demand,
            certificate,
            ..
        } => {
            if matches!(original, Some(Native::OmegaDeclaration(_))) {
                let tiles = super::coverage::members(nodes, sources)?;
                let owner = original.take().ok_or(LoanError::WrongLoan)?;
                Ok(owner.prepare_omega(heap, &tiles, registry, *certificate, account))
            } else {
                Ok(original
                    .take()
                    .ok_or(LoanError::WrongLoan)?
                    .advance_omega(heap, *demand, account))
            }
        }
        Contract::Delivery { source } => {
            let Some(Native::OmegaPublished(result)) = &nodes[*source].native else {
                return Err(LoanError::WrongLoan);
            };
            Ok(Transition::accepted(Native::OmegaPublished(
                result.retain(heap, account)?,
            )))
        }
        Contract::Input(_) => Err(LoanError::WrongLoan),
    }
}
