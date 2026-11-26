// SPDX-License-Identifier: AGPL-3.0-only
//! A temporary native preflight scope counts graph residence beside the exact native increment.

use super::refusal::Bytes;
use crate::heap::resources::ResourcePlan;
use crate::{ExecutionAccount, Heap, HeapError, LoanError};
use std::{cell::Cell, rc::Rc};

/// This bounded observer owns no graph, heap, native capability or variable error payload.
#[derive(Debug)]
pub(crate) struct Gate {
    /// Current borrowed graph-owner set's disjoint covered allocation bytes.
    graph: Cell<u64>,
    /// Known judgment payload already enforced by the original ledger's persistent scope.
    overlap: Cell<u64>,
    /// Original immutable retained and staged account ceilings.
    limits: [u64; 2],
    /// Exact byte decomposition is retained only for the returned refusal's classification.
    bytes: Cell<Option<Bytes>>,
    /// A reported original Work failure permanently stops ordinary graph transitions.
    stopped: Cell<bool>,
    /// Exact element obstruction, when native arithmetic remained representable.
    elements: Cell<bool>,
    /// A native fallible allocation failed after its own successful phase preflight.
    allocation: Cell<bool>,
}
impl Gate {
    /// The adapter admission accounts this fixed allocation and its owner edge.
    pub(super) const fn new(account: &ExecutionAccount) -> Self {
        Self {
            graph: Cell::new(0),
            overlap: Cell::new(0),
            limits: [
                account.limits().payload_bytes,
                account.limits().staged_payload_bytes,
            ],
            bytes: Cell::new(None),
            stopped: Cell::new(false),
            elements: Cell::new(false),
            allocation: Cell::new(false),
        }
    }

    /// Read-only delivery entry points also preserve a cumulative original Work stop.
    pub(super) fn observe<T>(&self, result: &Result<T, crate::loan::graph::walk::Error>) {
        if let Err(error) = result {
            self.observe_error(error);
        }
    }

    /// Borrow the original error rather than copying its retained payload into graph storage.
    pub(super) fn observe_error(&self, error: &crate::loan::graph::walk::Error) {
        use super::refusal::{self, Class};
        use crate::loan::graph::{sink, walk::Error};
        let stopped = match error {
            Error::Native(error) | Error::Delivery(sink::Error::Native(error)) => {
                refusal::loan(error, None) == Class::WorkStopped
            }
            Error::Adapter(error) => error.work_stopped(),
            Error::WorkStopped => true,
            _ => false,
        };
        if stopped {
            self.stopped.set(true);
        }
    }

    /// Owned event payloads preserve failures even when their outer transaction succeeded.
    pub(super) fn event<P: nrz_scalar::Scalar, const N: usize>(
        &self,
        event: &super::drive::Event<'_, P, N>,
    ) {
        use super::drive::Event;
        match event {
            Event::Input { outcome, .. } => self.input(outcome),
            Event::Refused { error, control, .. } => {
                if error.work_stopped() {
                    self.stopped.set(true);
                }
                self.observe(control);
            }
            Event::PreparationRefused {
                error,
                probe,
                evidence,
                control,
                ..
            } => {
                if super::refusal::preparation(error, None) == super::refusal::Class::WorkStopped {
                    self.stopped.set(true);
                }
                self.observe(probe);
                if let Err(crate::loan::graph::sink::Error::Native(error)) = evidence {
                    self.native(error);
                }
                self.observe(control);
            }
            Event::Advanced { control, .. }
            | Event::Terminal { control, .. }
            | Event::AttemptsExhausted { control, .. } => self.observe(control),
            Event::Quiescent | Event::ControlPending | Event::Control => {}
        }
    }

    /// Issuance carries both the native result and the subsequent graph control result.
    pub(super) fn input<P: nrz_scalar::Scalar>(&self, input: &super::input::InputRequest<P>) {
        use super::input::InputRequest;
        match input {
            InputRequest::Granted {
                evidence, control, ..
            } => {
                self.observe(evidence);
                self.observe(control);
            }
            InputRequest::AttemptsExhausted { control, .. } => self.observe(control),
            InputRequest::Refused { error, control, .. } => {
                self.native(error);
                self.observe(control);
            }
        }
    }

    /// Observe an original native carrier without cloning or retaining its variable payload.
    fn native(&self, error: &LoanError) {
        if super::refusal::loan(error, None) == super::refusal::Class::WorkStopped {
            self.stopped.set(true);
        }
    }

    /// A diagnostic may inspect a stopped owner, but it cannot resume native execution.
    pub(super) const fn stopped(&self) -> bool {
        self.stopped.get()
    }

    /// Native owning layers supply the overlap already present in their exact heap baseline.
    pub(crate) fn check(
        &self,
        native: usize,
        plan: ResourcePlan,
        overlap: Option<usize>,
    ) -> Result<(), HeapError> {
        let overlap = overlap.ok_or(HeapError::Capacity)?;
        let extra = plan
            .scratch
            .checked_sub(overlap)
            .ok_or(HeapError::Capacity)?;
        let baseline = u64::try_from(native)
            .map_err(|_| HeapError::Capacity)?
            .checked_add(self.graph.get())
            .ok_or(HeapError::Capacity)?;
        let retained = u64::try_from(plan.retained).map_err(|_| HeapError::Capacity)?;
        let peak = plan
            .retained
            .checked_add(extra)
            .ok_or(HeapError::Capacity)?;
        for (staged, addition, limit) in [
            (false, retained, self.limits[0]),
            (
                true,
                u64::try_from(peak).map_err(|_| HeapError::Capacity)?,
                self.limits[1],
            ),
        ] {
            let used = baseline.checked_add(addition).ok_or(HeapError::Capacity)?;
            if used > limit {
                self.bytes.set(Some(Bytes {
                    staged,
                    baseline,
                    addition,
                    limit,
                }));
                return Err(HeapError::ResourceBytes {
                    staged,
                    used,
                    limit,
                });
            }
        }
        Ok(())
    }

    /// The selected certificate is reserved by the native helper, not by both callers.
    pub(in crate::loan) fn external_bytes(&self, certificate: u64) -> Result<u64, LoanError> {
        self.graph
            .get()
            .checked_sub(self.overlap.get())
            .and_then(|bytes| bytes.checked_sub(certificate))
            .ok_or(LoanError::ByteOverflow)
    }

    /// Exact preparation scratch coexists with the complete native/graph baseline.
    pub(in crate::loan) fn preparation_bytes(
        &self,
        native: usize,
        certificate: u64,
    ) -> Result<u64, LoanError> {
        self.external_bytes(certificate)?
            .checked_add(u64::try_from(native).map_err(|_| LoanError::ByteOverflow)?)
            .ok_or(LoanError::ByteOverflow)
    }

    /// A candidate is private staging until the native join commits its disjoint retained children.
    pub(in crate::loan) fn capture(
        &self,
        native: usize,
        plan: ResourcePlan,
        overlap: usize,
        candidate: u64,
    ) -> Result<(), HeapError> {
        let baseline = u64::try_from(native)
            .map_err(|_| HeapError::Capacity)?
            .checked_add(self.graph.get())
            .ok_or(HeapError::Capacity)?;
        let retained = candidate
            .checked_sub(size_of::<nrz_bindings::LcOmegaDeclarationToken>() as u64)
            .and_then(|bytes| bytes.checked_add((2 * size_of::<usize>()) as u64))
            .and_then(|bytes| bytes.checked_add(plan.retained as u64))
            .ok_or(HeapError::Capacity)?;
        let staged = plan
            .scratch
            .checked_sub(overlap)
            .and_then(|bytes| bytes.checked_add(plan.retained))
            .ok_or(HeapError::Capacity)?;
        let staged = candidate
            .checked_add(staged as u64)
            .ok_or(HeapError::Capacity)?;
        for (staged, addition, limit) in [
            (false, retained, self.limits[0]),
            (true, staged, self.limits[1]),
        ] {
            let used = baseline.checked_add(addition).ok_or(HeapError::Capacity)?;
            if used > limit {
                self.bytes.set(Some(Bytes {
                    staged,
                    baseline,
                    addition,
                    limit,
                }));
                return Err(HeapError::ResourceBytes {
                    staged,
                    used,
                    limit,
                });
            }
        }
        Ok(())
    }

    /// Zero-addition phases still inspect the complete simultaneous graph and heap baseline.
    fn baseline(&self, native: usize) -> Result<(), HeapError> {
        let baseline = u64::try_from(native)
            .map_err(|_| HeapError::Capacity)?
            .checked_add(self.graph.get())
            .ok_or(HeapError::Capacity)?;
        for (staged, limit) in [(false, self.limits[0]), (true, self.limits[1])] {
            if baseline > limit {
                self.bytes.set(Some(Bytes {
                    staged,
                    baseline,
                    addition: 0,
                    limit,
                }));
                return Err(HeapError::ResourceBytes {
                    staged,
                    used: baseline,
                    limit,
                });
            }
        }
        Ok(())
    }

    /// Record a native precision-specific failure only after its sum was checked representable.
    pub(crate) fn element_failure(
        &self,
        legacy: [usize; 2],
        managed: [usize; 2],
        retired: [usize; 2],
        addition: [usize; 2],
        limit: usize,
    ) {
        let fits = (0..2).all(|index| {
            let total = legacy[index]
                .checked_add(managed[index])
                .and_then(|n| n.checked_sub(retired[index]))
                .and_then(|n| n.checked_add(addition[index]));
            total.is_some() && addition[index] <= limit
        });
        self.elements.set(fits);
    }

    /// The returned cause is matched against this exact preflight, never unrelated residents.
    pub(super) const fn bytes(&self) -> Option<Bytes> {
        self.bytes.get()
    }

    /// Only an owning allocation site may establish this otherwise unavailable classification.
    pub(crate) fn allocation_failure(&self) -> HeapError {
        self.allocation.set(true);
        HeapError::Capacity
    }

    /// Exact element evidence supplements only its matching native capacity carrier.
    pub(super) const fn loan(&self, error: &LoanError) -> super::refusal::Class {
        if self.elements.get() && super::refusal::loan_capacity(error) {
            super::refusal::Class::ElementsWait
        } else if self.allocation.get() && super::refusal::loan_capacity(error) {
            super::refusal::Class::AllocationWait
        } else {
            super::refusal::loan(error, self.bytes())
        }
    }

    /// Preserve the original nested carrier while classifying its own phase evidence.
    pub(super) const fn classify(&self, error: &super::native::Failure) -> super::refusal::Class {
        if self.elements.get() && super::refusal::capacity(error) {
            super::refusal::Class::ElementsWait
        } else if self.allocation.get() && super::refusal::capacity(error) {
            super::refusal::Class::AllocationWait
        } else {
            super::refusal::classify(error, self.bytes())
        }
    }
}

/// Temporary heap attachment clears on both normal return and unwinding, without another ledger.
struct Active<'a> {
    /// The exclusive borrow serializes native execution against scope changes.
    heap: &'a mut Heap,
}
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.heap.graph_gate = None;
    }
}

/// Prepay a finite observer transaction before lending the scope to one native phase.
pub(super) fn run<T>(
    gate: &Rc<Gate>,
    graph: (usize, u64),
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    operation: impl FnOnce(&mut Heap, &mut ExecutionAccount) -> T,
) -> Result<T, LoanError> {
    if heap.graph_gate.is_some() {
        return Err(LoanError::WrongLoan);
    }
    let (graph, overlap) = graph;
    let graph = u64::try_from(graph).map_err(|_| LoanError::ByteOverflow)?;
    // H activation and native scope inspection; refusal fields are metadata, not T records.
    // M graph, persistent overlap and three cleared refusal fields;
    // P one temporary shared edge and its clearing header transition.
    crate::loan::charge(&mut account.ledger, 2 + 5 + 2)?;
    gate.graph.set(graph);
    gate.overlap.set(overlap);
    gate.bytes.set(None);
    gate.elements.set(false);
    gate.allocation.set(false);
    crate::loan::charge(&mut account.ledger, heap.resource_work()?)?;
    gate.baseline(heap.resource_bytes()?)?;
    heap.graph_gate = Some(Rc::clone(gate));
    let active = Active { heap };
    Ok(operation(active.heap, account))
}
