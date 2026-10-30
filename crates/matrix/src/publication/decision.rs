// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic, externally owned live attachments for the closed two-input quotient subset.

pub(super) mod arithmetic;
mod kernel;
mod ratio;
pub use kernel::{FrozenRatioKernel, RatioKernelEvidence};

use crate::{DecisionSnapshotError, FrozenDecisionInputs, Heap, HeapError};
use nrz_bindings::readings::algebra::{
    Cas, Fault, Location, limits::BudgetKind, proof::VerifiedWitness,
};
use nrz_bindings::{Budget, CountDescriptor, SelectedDecision};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, LiteralLimits, Scalar};
use std::{cell::Cell, rc::Rc};

/// One location for the separate live admission and publication owner.
const AT: Location = Location::new("matrix live quotient decision");

/// Mathematical, numerical and lifetime refusals remain distinct.
#[derive(Debug, thiserror::Error)]
pub enum LiveDecisionError {
    /// The snapshot no longer names the same complete current inputs.
    #[error("live decision snapshot: {0}")]
    Snapshot(#[from] DecisionSnapshotError),
    /// The physical admission or count construction refused.
    #[error("live physical decision: {0}")]
    Admission(Box<nrz_bindings::DecisionError>),
    /// Original graph inspection, certificate borrowing or caller resources refused.
    #[error("live decision algebra: {0}")]
    Algebra(#[from] Fault),
    /// Outward numerical containment could not be established.
    #[error("live decision arithmetic: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
    /// Pure scalar construction or export refused on the delegated bounded schedule.
    #[error("live decision kernel: {0}")]
    Kernel(#[from] nrz_scalar::KernelError),
    /// The requested original recipe is outside the supported native receiver subset.
    #[error("unsupported original live decision recipe")]
    Recipe,
    /// The entire frozen numerical result does not fit the certified original enclosure.
    #[error("live decision box exceeds the certified original enclosure")]
    Enclosure,
    /// This live authority and every dependent descriptor or lowering have been retired.
    #[error("live decision attachment is retired")]
    Retired,
}

/// Fully checked private attachment; publication consumes it and rechecks input versions.
#[derive(Debug)]
pub struct PreparedDecision<P: Scalar> {
    /// Complete actual numerical input and companion versions.
    frozen: FrozenDecisionInputs<P>,
    /// Qualified immutable mathematical and physical selection.
    selected: SelectedDecision,
    /// Exact floating interpretation used for the snapshot containment check.
    arithmetic_revision: u32,
    /// Explicit constant-decoding policy, charged before either attempt.
    literals: LiteralLimits,
    /// Optional executed trace, retained under these identical complete frozen inputs.
    kernel: Option<Box<RatioKernelEvidence>>,
}

/// One shared live authority; descendants cannot independently survive retirement.
#[derive(Debug)]
struct Data<P: Scalar> {
    /// Complete immutable prepared statement and actual numerical dependencies.
    prepared: PreparedDecision<P>,
    /// Monotone retirement cache, separate from the archived mathematical theorem.
    retired: Cell<bool>,
}

/// Atomically returned decided attachment, scoped to the heap that supplied its inputs.
#[derive(Clone, Debug)]
pub struct PublishedDecision<P: Scalar>(Rc<Data<P>>);

/// A physical count descriptor whose current authority follows its parent attachment.
#[derive(Clone, Debug)]
pub struct LiveDecisionCount<P: Scalar> {
    /// The identical live authority shared by every descriptor from this publication.
    decision: PublishedDecision<P>,
    /// Immutable physical count; access requires the parent to remain current.
    descriptor: CountDescriptor,
}

impl Heap {
    /// Prepare a closed physical quotient selection from a complete frozen input set.
    ///
    /// The outward quotient of the actual total input boxes must fit wholly inside
    /// the independently checked original certificate. Numerical values never enter CAS.
    /// # Errors
    /// Refuses stale inputs, foreign witnesses, unsupported original recipes, a box
    /// crossing the certified region and resources. No live attachment is returned.
    pub fn prepare_ratio_decision<P: Scalar + 'static>(
        &self,
        frozen: FrozenDecisionInputs<P>,
        graph: Rc<Cas>,
        witness: VerifiedWitness,
        arithmetic: &ArithmeticContext,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<PreparedDecision<P>, LiveDecisionError> {
        self.check_decision_inputs(&frozen, budget)?;
        let bytes = frozen.retained_payload_bytes();
        let selected = budget.with_retained_payload(bytes, AT, |budget| {
            ratio::check(&frozen, &graph, &witness, arithmetic, literals, budget)?;
            let case = frozen.association().case();
            case.admit(graph, witness, case.attachment(), budget)
                .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))
        })?;
        Ok(PreparedDecision {
            frozen,
            selected,
            arithmetic_revision: arithmetic.revision(),
            literals,
            kernel: None,
        })
    }

    /// Publish one complete decided attachment after the final current-version check.
    ///
    /// The returned shared owner is the publication unit; the heap allocates no count
    /// region or graph activation. All fallible checks precede its construction.
    /// # Errors
    /// Refuses a foreign heap, changed inputs or resources with no partial attachment.
    pub fn publish_decision<P: Scalar + 'static>(
        &self,
        prepared: PreparedDecision<P>,
        budget: &mut Budget,
    ) -> Result<PublishedDecision<P>, LiveDecisionError> {
        let kernel_bytes = prepared
            .kernel
            .as_ref()
            .map_or(0, |trace| trace.retained_payload_bytes());
        let bytes = prepared
            .frozen
            .retained_payload_bytes()
            .checked_add(prepared.selected.retained_payload_bytes())
            .and_then(|value| value.checked_add(128))
            .and_then(|value| value.checked_add(kernel_bytes))
            .ok_or_else(|| LiveDecisionError::Snapshot(HeapError::Capacity.into()))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.work(128, AT)?;
        self.check_decision_inputs(&prepared.frozen, budget)?;
        Ok(PublishedDecision(Rc::new(Data {
            prepared,
            retired: Cell::new(false),
        })))
    }
}

impl<P: Scalar + 'static> PublishedDecision<P> {
    /// Borrow the selected physical snapshot only while this heap's inputs remain current.
    /// # Errors
    /// Refuses retired or changed dependencies, foreign heaps and caller limits.
    pub fn current<'h>(
        &'h self,
        heap: &'h Heap,
        budget: &mut Budget,
    ) -> Result<&'h SelectedDecision, LiveDecisionError> {
        budget.work(1, AT)?;
        if self.0.retired.get() {
            return Err(LiveDecisionError::Retired);
        }
        if let Err(cause) = heap.check_decision_inputs(&self.0.prepared.frozen, budget) {
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

    /// Match a requested original source/map/case association before lending live evidence.
    /// # Errors
    /// Refuses changed exact declarations or designated symbols even at equal center bytes.
    pub fn current_for<'h>(
        &'h self,
        heap: &'h Heap,
        association: &nrz_bindings::DecisionInputs,
        budget: &mut Budget,
    ) -> Result<&'h SelectedDecision, LiveDecisionError> {
        let frozen = &self.0.prepared.frozen;
        let [first, second] = frozen.operands() else {
            return Err(LiveDecisionError::Recipe);
        };
        let symbols = [&first.input().symbols, &second.input().symbols];
        association
            .authenticate(frozen.association().case(), &symbols, budget)
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        self.current(heap, budget)
    }

    /// Construct a descriptor whose later reads share this exact live dependency authority.
    /// # Errors
    /// Refuses stale attachments, physical capacity and caller limits atomically.
    pub fn count(
        &self,
        heap: &Heap,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<LiveDecisionCount<P>, LiveDecisionError> {
        let selected = self.current(heap, budget)?;
        let descriptor = selected
            .count(capacity, budget)
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        Ok(LiveDecisionCount {
            decision: self.clone(),
            descriptor,
        })
    }

    /// Historical complete source versions for this attachment; current access checks its lease.
    #[must_use]
    pub fn inputs(&self) -> &FrozenDecisionInputs<P> {
        &self.0.prepared.frozen
    }

    /// Executed pure kernel trace, when this attachment used the optional scalar-origin route.
    /// The historical record alone grants no current heap authority.
    #[must_use]
    pub fn kernel_trace(&self) -> Option<&RatioKernelEvidence> {
        self.0.prepared.kernel.as_deref()
    }

    /// Recorded floating proof revision; this historical fact is not live authority.
    #[must_use]
    pub fn arithmetic_revision(&self) -> u32 {
        self.0.prepared.arithmetic_revision
    }
    /// Recorded explicit literal-decoding policy; all-zero means the pure-dyadic trace used none.
    #[must_use]
    pub fn literal_limits(&self) -> LiteralLimits {
        self.0.prepared.literals
    }
}

impl<P: Scalar + 'static> LiveDecisionCount<P> {
    /// Borrow the count only under the same current heap dependency check as its selection.
    /// # Errors
    /// Every parent retirement, foreign owner or resource refusal also refuses this descriptor.
    pub fn current<'h>(
        &'h self,
        heap: &'h Heap,
        budget: &mut Budget,
    ) -> Result<&'h CountDescriptor, LiveDecisionError> {
        self.decision.current(heap, budget)?;
        Ok(&self.descriptor)
    }
}
