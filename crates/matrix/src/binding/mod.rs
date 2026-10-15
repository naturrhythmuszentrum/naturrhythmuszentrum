// SPDX-License-Identifier: AGPL-3.0-only
//! Exclusive checked bindings, frozen dependencies and private output reservations.

mod error;
mod limits;
mod operation;
mod reservation;
mod snapshot;
mod views;

pub use error::OperationError;
pub use limits::ExecutionLimits;
pub use operation::{CheckedOperation, CopyContract, OperationDraft};
pub use reservation::OutputReservation;
pub use snapshot::{CompanionSnapshot, InputSnapshot};
pub use views::{NormalizedInputs, NormalizedView, PhysicalView};

use crate::heap::identity::identity;
use crate::{Handle, Heap, HeapError, Value};
use nrz_scalar::Scalar;
use reservation::Reservations;

/// Unique process-local identity of one validated execution, never a buffer address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionId(pub(crate) u64);

/// Proposed concrete operands; only validation can lend numerical inputs.
#[derive(Debug)]
pub struct Binding<P: Scalar> {
    /// Private checked recipe, never an unchecked draft.
    operation: CheckedOperation,
    /// Operation-ordered input locators.
    inputs: Vec<Handle<Value, P>>,
    /// Explicit work and private scratch limits.
    limits: ExecutionLimits,
}

impl<P: Scalar + 'static> Binding<P> {
    /// Bind a checked recipe to concrete operands, with no access granted yet.
    #[must_use]
    pub const fn new(
        operation: CheckedOperation,
        inputs: Vec<Handle<Value, P>>,
        limits: ExecutionLimits,
    ) -> Self {
        Self {
            operation,
            inputs,
            limits,
        }
    }

    /// Freeze actual dependencies and reserve complete fresh disjoint output bundles.
    ///
    /// Dropping the result discards all reservations without publishing any handle.
    /// # Errors
    /// Refuses stale/foreign/category/precision mismatches, schemas, structure or limits.
    pub fn validate(self, heap: &mut Heap) -> Result<Bound<'_, P>, OperationError> {
        if self.inputs.len() != self.operation.inputs.len() {
            return Err(OperationError::InputCount);
        }
        self.limits.check(self.inputs.len(), 0)?;
        let dependencies = capture(heap, &self.inputs)?;
        let entries = payload_entries(&dependencies)?;
        self.limits.check(self.inputs.len(), entries)?;
        for (slot, (dependency, contract)) in dependencies
            .iter()
            .zip(self.operation.inputs.iter())
            .enumerate()
        {
            dependency.validate(heap, contract, slot)?;
        }
        let execution = ExecutionId(identity()?);
        let reservations = Reservations::prepare(heap, &dependencies)?;
        reservations.validate(heap)?;
        Ok(Bound {
            heap,
            operation: self.operation,
            dependencies,
            execution,
            reservations,
        })
    }
}

/// Evidence-owning execution lease; every view borrows for less than this exclusive borrow.
#[derive(Debug)]
pub struct Bound<'h, P: Scalar> {
    /// The only mutable access to the owner throughout this execution.
    pub(crate) heap: &'h mut Heap,
    /// Checked original recipe and exact normalization references.
    pub(crate) operation: CheckedOperation,
    /// Actual ordered input versions, schemas, selections and companion targets.
    pub(crate) dependencies: Vec<InputSnapshot<P>>,
    /// Unforgeable association between a binding and its borrowed numerical inputs.
    pub(crate) execution: ExecutionId,
    /// Fully private output storage and descriptor occupations.
    pub(crate) reservations: Reservations<P>,
}

impl<P: Scalar> Bound<'_, P> {
    /// Execution identity shared by this binding's views and later candidates.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Checked native recipe and normalization definitions.
    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
    /// Actual operation-ordered dependencies, including companion-only evidence.
    #[must_use]
    pub fn dependencies(&self) -> &[InputSnapshot<P>] {
        &self.dependencies
    }
    /// Future output versions and interpretations; no value handles escape here.
    #[must_use]
    pub fn output_reservations(&self) -> &[OutputReservation] {
        &self.reservations.outputs
    }
}

/// Capture metadata before scanning numerical entries under the total work limit.
fn capture<P: Scalar + 'static>(
    heap: &Heap,
    inputs: &[Handle<Value, P>],
) -> Result<Vec<InputSnapshot<P>>, OperationError> {
    let mut snapshots = buffer(inputs.len())?;
    for &handle in inputs {
        snapshots.push(InputSnapshot::capture(heap, handle)?);
    }
    Ok(snapshots)
}

/// Count centers and all carried companions without confusing base and selected extents.
fn payload_entries<P: Scalar>(inputs: &[InputSnapshot<P>]) -> Result<usize, OperationError> {
    inputs.iter().try_fold(0_usize, |total, input| {
        let count = input
            .layout()
            .extent()
            .entries()
            .checked_mul(1 + usize::from(input.companion().is_some()))
            .ok_or(OperationError::ResourceLimit)?;
        total
            .checked_add(count)
            .ok_or(OperationError::ResourceLimit)
    })
}

/// Fallibly reserve a metadata or scalar vector before entering an append-only phase.
pub(crate) fn buffer<T>(count: usize) -> Result<Vec<T>, HeapError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| HeapError::Capacity)?;
    Ok(result)
}
