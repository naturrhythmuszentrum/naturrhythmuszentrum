// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic selected lattice attachments share one monotone dependency authority.

use crate::{DecisionSnapshotError, FrozenLatticeEnclosure, Heap, HeapError, LiveDecisionError};
use nrz_bindings::readings::algebra::{Location, lattice::LatticeEnclosure, limits::BudgetKind};
use nrz_bindings::{Budget, LatticeCoordinate, LatticeInputs, SelectedLattice};
use nrz_scalar::Scalar;
use std::{cell::Cell, rc::Rc};

/// Only lattice attachment consumers pay these ownership and current-access charges.
const AT: Location = Location::new("matrix live lattice publication");

/// Complete privately validated tuple and the live enclosure supporting its exact original box.
#[derive(Debug)]
pub struct PreparedLatticeDecision<P: Scalar> {
    /// Every live source, value, total radius and numerical companion version.
    enclosure: FrozenLatticeEnclosure<P>,
    /// The complete immutable mathematical selection and physical outputs.
    selected: SelectedLattice,
}

/// One lifetime authority shared by a selected tuple and every dependent descriptor/lowering.
#[derive(Debug)]
struct Data<P: Scalar> {
    /// Complete original statement and native dependency closure.
    prepared: PreparedLatticeDecision<P>,
    /// Once stale, an equal later tuple or restored center cannot revive this publication.
    retired: Cell<bool>,
}

/// Complete selected tuple atomically published after final lease validation.
#[derive(Clone, Debug)]
pub struct PublishedLatticeDecision<P: Scalar>(Rc<Data<P>>);

/// Original-coordinate representative access retains its parent's complete live authority.
#[derive(Clone, Debug)]
pub struct LiveLatticeRepresentative<P: Scalar> {
    /// Shared monotone authority for the entire selected tuple.
    decision: PublishedLatticeDecision<P>,
    /// Ordered physical coordinate, checked when this dependent descriptor is constructed.
    coordinate: usize,
}

/// A separately capacity-checked executable index still depends on the whole selected tuple.
#[derive(Clone, Debug)]
pub struct LiveLatticeIndex<P: Scalar> {
    /// Shared monotone authority; no independent selected prefix is published.
    decision: PublishedLatticeDecision<P>,
    /// Exact signed value after its distinct executable-capacity check.
    value: i64,
}

impl Heap {
    /// Bind one physical selection to the identical certified original box and live sources.
    /// # Errors
    /// Refuses stale inputs, changed questions or boxes, unsupported enclosure forms and
    /// exhausted resources before returning any prepared attachment.
    pub fn prepare_lattice_decision<P: Scalar + 'static>(
        &self,
        enclosure: FrozenLatticeEnclosure<P>,
        selected: SelectedLattice,
        budget: &mut Budget,
    ) -> Result<PreparedLatticeDecision<P>, LiveDecisionError> {
        self.check_lattice_inputs(enclosure.inputs(), budget)?;
        enclosure
            .inputs()
            .association()
            .case()
            .validate_attachment(selected.attachment(), budget)
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        let authority = selected
            .cas()
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        let (_, evidence) = authority.lattice_selection_evidence(selected.witness(), budget)?;
        let LatticeEnclosure::Original(original) = evidence else {
            return Err(LiveDecisionError::Recipe);
        };
        budget.work(enclosure.original.retained_bytes(), AT)?;
        if &original.original != enclosure.original.contract() {
            return Err(LiveDecisionError::Enclosure);
        }
        drop(authority);
        Ok(PreparedLatticeDecision {
            enclosure,
            selected,
        })
    }

    /// Atomically publish one complete tuple after rechecking every live dependency.
    /// # Errors
    /// Refuses stale or foreign leases and caller limits with no selected prefix or heap output.
    /// All fallible work precedes constructing the shared live authority.
    pub fn publish_lattice_decision<P: Scalar + 'static>(
        &self,
        prepared: PreparedLatticeDecision<P>,
        budget: &mut Budget,
    ) -> Result<PublishedLatticeDecision<P>, LiveDecisionError> {
        let bytes = prepared
            .enclosure
            .retained_payload_bytes()
            .checked_add(prepared.selected.retained_payload_bytes())
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or_else(|| LiveDecisionError::Snapshot(HeapError::Capacity.into()))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.work(128, AT)?;
        self.check_lattice_inputs(prepared.enclosure.inputs(), budget)?;
        Ok(PublishedLatticeDecision(Rc::new(Data {
            prepared,
            retired: Cell::new(false),
        })))
    }
}

impl<P: Scalar + 'static> PublishedLatticeDecision<P> {
    /// Borrow the complete physical tuple under the current original heap dependency lease.
    /// # Errors
    /// Refuses changed dependencies, retired attachments, foreign heaps and resource exhaustion.
    pub fn current<'h>(
        &'h self,
        heap: &'h Heap,
        budget: &mut Budget,
    ) -> Result<&'h SelectedLattice, LiveDecisionError> {
        budget.work(1, AT)?;
        if self.0.retired.get() {
            return Err(LiveDecisionError::Retired);
        }
        if let Err(cause) = heap.check_lattice_inputs(self.0.prepared.enclosure.inputs(), budget) {
            let transient = matches!(
                cause,
                DecisionSnapshotError::Resources(_)
                    | DecisionSnapshotError::Heap(
                        HeapError::ForeignHandle | HeapError::WorkLimit | HeapError::Capacity
                    )
            );
            if !transient {
                self.0.retired.set(true);
            }
            return Err(cause.into());
        }
        Ok(&self.0.prepared.selected)
    }

    /// Authenticate a requested immutable question and every designated source symbol.
    /// # Errors
    /// Refuses equal-valued replacement declarations, changed maps, basis, offsets or conventions.
    pub fn current_for<'h>(
        &'h self,
        heap: &'h Heap,
        association: &LatticeInputs,
        budget: &mut Budget,
    ) -> Result<&'h SelectedLattice, LiveDecisionError> {
        let original = self.0.prepared.enclosure.inputs().association();
        let count =
            u64::try_from(original.operands().len()).map_err(|_| LiveDecisionError::Recipe)?;
        budget.check(BudgetKind::StagedPayloadBytes, count.saturating_mul(8), AT)?;
        budget.work(count, AT)?;
        let symbols = original
            .operands()
            .iter()
            .map(nrz_bindings::DecisionOperand::symbols)
            .collect::<Vec<_>>();
        association
            .authenticate(original.case(), &symbols, budget)
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        self.current(heap, budget)
    }

    /// Retain a physical representative descriptor under the complete parent authority.
    /// # Errors
    /// Refuses stale parents, unavailable coordinates and exhausted resources.
    pub fn representative(
        &self,
        heap: &Heap,
        coordinate: usize,
        budget: &mut Budget,
    ) -> Result<LiveLatticeRepresentative<P>, LiveDecisionError> {
        self.current(heap, budget)?
            .representative(coordinate)
            .ok_or(LiveDecisionError::Recipe)?;
        Ok(LiveLatticeRepresentative {
            decision: self.clone(),
            coordinate,
        })
    }

    /// Lower one exact Integer through its separate signed executable-capacity consumer.
    /// # Errors
    /// Refuses unavailable coordinates, capacity or resource exhaustion without changing the tuple.
    pub fn index(
        &self,
        heap: &Heap,
        coordinate: usize,
        capacity: (i64, i64),
        budget: &mut Budget,
    ) -> Result<LiveLatticeIndex<P>, LiveDecisionError> {
        let selected = self.current(heap, budget)?;
        let value = selected
            .cas()
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?
            .materialize_lattice_index(selected.witness(), coordinate, capacity, budget)?;
        Ok(LiveLatticeIndex {
            decision: self.clone(),
            value,
        })
    }
}

impl<P: Scalar + 'static> LiveLatticeRepresentative<P> {
    /// Borrow the physical representative only while every parent dependency remains current.
    /// # Errors
    /// Refuses parent retirement, changed sources, foreign heaps and exhausted resources.
    pub fn current<'h>(
        &'h self,
        heap: &'h Heap,
        budget: &mut Budget,
    ) -> Result<LatticeCoordinate<'h>, LiveDecisionError> {
        self.decision
            .current(heap, budget)?
            .representative(self.coordinate)
            .ok_or(LiveDecisionError::Recipe)
    }
}

impl<P: Scalar + 'static> LiveLatticeIndex<P> {
    /// Read the lowered signed index only under the complete current parent lease.
    /// # Errors
    /// Refuses parent retirement, changed sources, foreign heaps and exhausted resources.
    pub fn current(&self, heap: &Heap, budget: &mut Budget) -> Result<i64, LiveDecisionError> {
        self.decision.current(heap, budget)?;
        Ok(self.value)
    }
}
