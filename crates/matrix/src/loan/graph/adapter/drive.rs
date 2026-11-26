// SPDX-License-Identifier: AGPL-3.0-only
//! One bounded selection enters at most one original native phase.

mod coverage;
mod dispatch;
mod fanout;
mod terminal;
use super::super::{
    sink::{
        self,
        codec::{Encoding, Evidence, Outcome},
    },
    walk::{
        Error, Handle,
        adapter::Selection,
        control::{Control, Demand},
    },
};
use super::{
    native::{Failure, Native},
    node::{Contract, State},
    owner::{Borrowed, Owner},
};
use crate::{ExecutionAccount, Heap, LcOmegaOutput, LcShapeError, LoanError};
use nrz_scalar::Scalar;

/// Every entered probe's receipt escapes with its original native disposition.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "consuming refusal preserves the exact declaration without allocating an error box"
)]
pub enum Event<'i, P: Scalar, const N: usize> {
    /// No eligible work exists; this alone is not a completion certificate.
    Quiescent,
    /// A bounded scope or invalidation traversal must finish first.
    ControlPending,
    /// One existing notification, release or retirement step committed.
    Control,
    /// One native phase committed, with its entered probe evidence.
    Advanced {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Prepared evidence for the exact entered native probe.
        probe: sink::Handle,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// A previously requested host expectation retried through the bounded waiting set.
    Input {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Required terminal delivery or returned host request disposition.
        outcome: super::input::InputRequest<P>,
    },
    /// A terminal disposition was preserved before enabling consumer notification.
    Terminal {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: sink::Handle,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// Full native error escapes intact while its recoverable continuation remains owned.
    Refused {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Complete original typed refusal, returned without a graph-owned error queue.
        error: Failure,
        /// Prepared evidence for the exact entered native probe.
        probe: Option<sink::Handle>,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// Consuming preparation returns both exact original values rather than retaining variable evidence.
    PreparationRefused {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Identical original checked output declaration returned by the consuming boundary.
        declaration: LcOmegaOutput<'i, P, N>,
        /// Complete original typed refusal, returned without a graph-owned error queue.
        error: LcShapeError,
        /// Prepared evidence for the exact entered native probe.
        probe: Result<sink::Handle, Error>,
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: Result<sink::Handle, sink::Error>,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
    /// Original attempt exhaustion entered no native phase.
    AttemptsExhausted {
        /// Original graph occupation that entered this transition.
        node: Handle,
        /// Original bounded sink receipt, independent of native capability ownership.
        evidence: sink::Handle,
        /// Required terminal delivery or returned host request disposition.
        outcome: sink::Handle,
        /// Result of the subsequent graph control transition, after preserving native ownership.
        control: Result<(), Error>,
    },
}

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Drive one native or ordered control transition without replenishing any original allowance.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn drive_one(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.drive_one_inner(heap, account, others);
        if let Ok(event) = &result {
            self.backing.gate.event(event);
        }
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn drive_one_inner(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
        others: &[Borrowed<'_>],
    ) -> Result<Event<'i, P, N>, Error> {
        account.authenticate(self.backing.account)?;
        if heap.scope != self.backing.heap {
            return Err(LoanError::ForeignOwner.into());
        }
        let nodes = &self.backing.nodes;
        let selection = self.backing.core.select(
            account,
            |source, budget| {
                crate::loan::charge(budget, 1)?; // N original producer disposition.
                match nodes.get(source).ok_or(LoanError::StaleOccupation)?.state {
                    State::Published => Ok(Demand::Satisfied),
                    State::Absent | State::Cancelled | State::MissingInput => {
                        Ok(Demand::RequiredAbsence)
                    }
                    State::Refused
                    | State::DependencyRefused(_)
                    | State::UnsupportedPartial
                    | State::AttemptsExhausted => Ok(Demand::Refused),
                    State::Pending
                    | State::Borrowed
                    | State::Returned
                    | State::Repair
                    | State::ResourceWait
                    | State::OwnershipWait => Err(LoanError::WrongLoan),
                }
            },
            |_| {},
        )?;
        let handle = match selection {
            Selection::Quiescent => return Ok(Event::Quiescent),
            Selection::ControlPending => return Ok(Event::ControlPending),
            Selection::Control(Control::Retired(slot)) => {
                self.backing.nodes[slot].native = None;
                self.backing.nodes[slot].pin = None;
                return Ok(Event::Control);
            }
            Selection::Control(Control::Notified(_) | Control::Released(_)) => {
                return Ok(Event::Control);
            }
            Selection::Native(handle) => handle,
        };
        let input = matches!(self.backing.nodes[handle.slot].contract, Contract::Input(_));
        if input && self.backing.nodes[handle.slot].state == State::Published {
            return self.fanout(handle, heap, account, others);
        }
        if terminal(self.backing.nodes[handle.slot].state) {
            return self.finish(handle, heap, account);
        }
        if self.backing.nodes[handle.slot].state != State::Pending {
            return Ok(Event::Quiescent);
        }
        if let Some(state) = self.coverage(handle, heap, account)? {
            self.backing.nodes[handle.slot].state = state;
            return self.finish(handle, heap, account);
        }
        self.dispatch(handle, heap, account, others)
    }

    /// The closed sink record commits before the walk can notify any consumer.
    pub(super) fn finish(
        &mut self,
        handle: Handle,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Event<'i, P, N>, Error> {
        let state = self.backing.nodes[handle.slot].state;
        let evidence = if let Some(evidence) = self.backing.nodes[handle.slot].evidence {
            evidence
        } else {
            let outcome = match state {
                State::Published => Outcome::Published,
                State::Absent | State::MissingInput => Outcome::Absent,
                State::Cancelled => Outcome::Cancelled,
                State::Refused
                | State::DependencyRefused(_)
                | State::UnsupportedPartial
                | State::AttemptsExhausted => Outcome::DependencyRefused,
                State::Pending
                | State::Borrowed
                | State::Returned
                | State::Repair
                | State::ResourceWait
                | State::OwnershipWait => return Err(Error::InvalidPhase),
            };
            let prepared = self
                .backing
                .sink
                .prepare(handle.slot, Encoding::Outcome, account)
                .map_err(Error::Delivery)?;
            crate::loan::charge(&mut account.ledger, 1)?; // N saved terminal evidence reference.
            let evidence = prepared
                .commit(Evidence::Outcome(outcome))
                .map_err(|(_, error)| Error::Delivery(error))?;
            self.backing.nodes[handle.slot].evidence = Some(evidence);
            evidence
        };
        let control = self
            .settle_native(handle, heap, account)
            .and_then(|()| self.backing.core.park(handle, account))
            .and_then(|()| self.backing.core.complete(handle, account));
        Ok(Event::Terminal {
            node: handle,
            evidence,
            control,
        })
    }
}

/// Terminal graph status cannot manufacture a native numerical value.
const fn terminal(state: State) -> bool {
    matches!(
        state,
        State::Published
            | State::Absent
            | State::Cancelled
            | State::MissingInput
            | State::DependencyRefused(_)
            | State::UnsupportedPartial
            | State::AttemptsExhausted
            | State::Refused
    )
}

/// Existing native owner variants select the call; no caller supplies an opaque phase number.
const fn phase<P: Scalar, const N: usize>(native: &Native<'_, P, N>) -> u64 {
    match native {
        Native::MappedRequest { .. } => 1,
        Native::MappedReady(_) => 2,
        Native::MappedEvaluated(_) => 3,
        Native::OmegaDeclaration(_) => 4,
        Native::OmegaPrepared(_) => 5,
        Native::OmegaPending(_) => 6,
        Native::OmegaEvaluated(_) => 7,
        Native::OmegaFailed(_) => 8,
        Native::MappedPublished(_) | Native::OmegaPublished(_) | Native::Cancelled => 9,
    }
}
