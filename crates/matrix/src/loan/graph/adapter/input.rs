// SPDX-License-Identifier: AGPL-3.0-only
//! Host access escapes only as the existing owned loan; notification cannot cancel it.

use super::super::{
    sink::{
        self,
        codec::{Capacity, Encoding, Evidence, ProbeFailure},
    },
    walk::{Error, Handle},
};
use super::{
    node::{Contract, State},
    owner::Owner,
    refusal::Class,
};
use crate::{ExecutionAccount, HandbackOutcome, Heap, LoanError, OwnedLoan};
use nrz_scalar::Scalar;

/// Issuance reports its required probe receipt beside the native host capability.
#[derive(Debug)]
pub enum InputRequest<P: Scalar> {
    /// The caller alone owns the writable loan until genuine return or cancellation.
    Granted {
        /// Identical writable native loan held exclusively by the caller.
        loan: OwnedLoan<P>,
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: Result<sink::Handle, Error>,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// The finite original attempt count reached zero without entering native admission.
    AttemptsExhausted {
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: sink::Handle,
        /// Required terminal delivery or returned host request disposition.
        outcome: sink::Handle,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// The same declaration remains with the graph on refusal.
    Refused {
        /// Complete original typed refusal, returned without a graph-owned error queue.
        error: LoanError,
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: Option<sink::Handle>,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
}

/// Rejected handback never consumes the rightful host capability.
#[derive(Debug)]
pub enum InputReturn<P: Scalar> {
    /// Native publication preceded the graph's pending fan-out work.
    Published {
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// The identical original loan remains repairable by the host.
    Rejected {
        /// Identical writable native loan held exclusively by the caller.
        loan: OwnedLoan<P>,
        /// Complete original typed refusal, returned without a graph-owned error queue.
        error: Error,
    },
}
impl<P: Scalar + 'static, const N: usize> Owner<'_, P, N> {
    /// Explicitly issue one admitted no-loan expectation using the native preserving transaction.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn request_input(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[super::owner::Borrowed<'_>],
    ) -> Result<InputRequest<P>, Error> {
        let result = self.request_input_inner(handle, heap, account, others);
        if let Ok(input) = &result {
            self.backing.gate.input(input);
        }
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn request_input_inner(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[super::owner::Borrowed<'_>],
    ) -> Result<InputRequest<P>, Error> {
        self.authenticate(handle, heap, account)?;
        let graph_bytes = self.resident_bytes(others, heap, account)?;
        let slot = handle.slot;
        if !matches!(
            self.backing.nodes[slot].state,
            State::Pending | State::ResourceWait | State::OwnershipWait
        ) {
            return Err(Error::InvalidPhase);
        }
        let Contract::Input(declaration) = &self.backing.nodes[slot].contract else {
            return Err(Error::InvalidPhase);
        };
        let declaration = declaration.as_ref().ok_or(LoanError::WrongLoan)?;
        let extent = declaration.extent;
        let remaining = self.backing.core.reservation.backing.nodes[slot].attempts;
        if remaining == 0 {
            let super::drive::Event::AttemptsExhausted {
                evidence,
                outcome,
                control,
                ..
            } = self.exhaust(handle, handle, heap, account)?
            else {
                return Err(Error::InvalidPhase);
            };
            return Ok(InputRequest::AttemptsExhausted {
                evidence,
                outcome,
                control,
            });
        }
        let (next_probe, position) = self.backing.nodes[slot].next_probe()?;
        let destination = self
            .backing
            .sink
            .prepare(position, Encoding::Probe, account)
            .map_err(Error::Delivery)?
            .at_native_state(0);
        crate::loan::charge(&mut account.ledger, 2)?; // N adapter record and H native-stop preservation.
        let entered = self.backing.core.enter_external(handle, handle, account)?;
        if !entered {
            return Err(Error::InvalidPhase);
        }
        self.backing.nodes[slot].probes = next_probe;
        let Contract::Input(declaration) = &mut self.backing.nodes[slot].contract else {
            return Err(Error::InvalidPhase);
        };
        let result = super::gate::run(
            &self.backing.gate,
            graph_bytes,
            heap,
            account,
            |heap, account| {
                let layout = crate::Layout::contiguous(extent).map_err(LoanError::from)?;
                crate::loan::lifecycle::admission::request_preserving(
                    heap,
                    declaration,
                    layout,
                    account,
                )
            },
        )
        .and_then(std::convert::identity);
        let evidence = match &result {
            Ok(_) => Evidence::Probe(Capacity::Fits),
            Err(error) => failed_probe(self.backing.gate.loan(error)),
        };
        let evidence = destination
            .commit(evidence)
            .map_err(|(_, error)| Error::Delivery(error));
        Ok(self.finish_request(handle, result, evidence, account))
    }

    /// Save the exact host capability or typed refusal before any subsequent control debit.
    fn finish_request(
        &mut self,
        handle: Handle,
        result: Result<OwnedLoan<P>, LoanError>,
        evidence: Result<sink::Handle, Error>,
        account: &mut ExecutionAccount,
    ) -> InputRequest<P> {
        let slot = handle.slot;
        match result {
            Ok(loan) => {
                self.backing.nodes[slot].loan = Some(loan.ticket());
                self.backing.nodes[slot].state = State::Borrowed;
                InputRequest::Granted {
                    loan,
                    evidence,
                    control: self.backing.core.park(handle, account),
                }
            }
            Err(error) => {
                let class = self.backing.gate.loan(&error);
                let control = if class == Class::WorkStopped {
                    self.backing.core.stop_native()
                } else {
                    self.backing.nodes[slot].state = match class {
                        Class::BytesWait { .. } | Class::ElementsWait | Class::AllocationWait => {
                            State::ResourceWait
                        }
                        Class::OwnershipWait => State::OwnershipWait,
                        Class::Repair => State::Repair,
                        Class::Terminal => State::Refused,
                        Class::WorkStopped => State::Pending,
                    };
                    match class {
                        Class::Terminal => self.backing.core.resume(handle, account),
                        Class::BytesWait { .. } | Class::ElementsWait | Class::AllocationWait => {
                            self.backing.core.wait_native(handle, account)
                        }
                        Class::OwnershipWait | Class::Repair | Class::WorkStopped => {
                            self.backing.core.park(handle, account)
                        }
                    }
                };
                let (evidence, control) = match evidence {
                    Ok(evidence) => (Some(evidence), control),
                    Err(error) => (None, Err(error)),
                };
                InputRequest::Refused {
                    error,
                    evidence,
                    control,
                }
            }
        }
    }

    /// Handback requires both the graph occupation and the identical issued native ticket.
    pub fn return_input(
        &mut self,
        handle: Handle,
        loan: OwnedLoan<P>,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> InputReturn<P> {
        let result = self.return_input_inner(handle, loan, heap, account);
        match &result {
            InputReturn::Published { control } => self.backing.gate.observe(control),
            InputReturn::Rejected { error, .. } => self.backing.gate.observe_error(error),
        }
        result
    }

    /// Rejection leaves the identical writable host loan with its rightful caller.
    fn return_input_inner(
        &mut self,
        handle: Handle,
        loan: OwnedLoan<P>,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> InputReturn<P> {
        if let Err(error) = self.authenticate(handle, heap, account) {
            return InputReturn::Rejected { loan, error };
        }
        let node = &self.backing.nodes[handle.slot];
        if node.state != State::Borrowed || node.loan != Some(loan.ticket()) {
            return InputReturn::Rejected {
                loan,
                error: LoanError::WrongLoan.into(),
            };
        }
        // N exact native publication slot and saved fan-out cursor, before native handback.
        if let Err(error) = crate::loan::charge(&mut account.ledger, 2) {
            return InputReturn::Rejected {
                loan,
                error: error.into(),
            };
        }
        match heap.handback(loan, account) {
            HandbackOutcome::Published(tile, _) => {
                let first = self.backing.core.reservation.backing.nodes[handle.slot].links[0];
                let node = &mut self.backing.nodes[handle.slot];
                node.pin = Some(tile.into_pin());
                node.state = State::Published;
                node.fanout = if first == usize::MAX {
                    first
                } else {
                    self.backing.core.reservation.backing.edges[first].links[1]
                };
                InputReturn::Published {
                    control: self.backing.core.resume(handle, account),
                }
            }
            HandbackOutcome::Repair(error, loan) | HandbackOutcome::ReturnRejected(error, loan) => {
                InputReturn::Rejected {
                    loan,
                    error: error.into(),
                }
            }
        }
    }
}

/// No scalar required/available claim is fabricated for a failed non-byte probe.
pub(super) const fn failed_probe(class: Class) -> Evidence {
    match class {
        Class::BytesWait {
            required,
            available,
        } => Evidence::Probe(Capacity::Wait {
            required,
            available,
        }),
        Class::WorkStopped => Evidence::ProbeFailure(ProbeFailure::WorkStopped),
        Class::AllocationWait => Evidence::ProbeFailure(ProbeFailure::Allocation),
        Class::ElementsWait => Evidence::ProbeFailure(ProbeFailure::Elements),
        Class::OwnershipWait => Evidence::ProbeFailure(ProbeFailure::Ownership),
        Class::Repair => Evidence::ProbeFailure(ProbeFailure::Repair),
        Class::Terminal => Evidence::ProbeFailure(ProbeFailure::Terminal),
    }
}
