// SPDX-License-Identifier: AGPL-3.0-only
//! Identity normalization and exact copy evaluation perform no floating arithmetic.

use super::{Candidate, Payload};
use crate::binding::buffer;
use crate::heap::identity::identity;
use crate::{
    Bound, CopyEvidence, Coverage, Failure, FailureRecord, NormalizedInputs, NormalizedView,
    OperationError, OutputReservation,
};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static> Bound<'_, P> {
    /// Evaluate this binding's operation-ordered normalized inputs into private scratch.
    ///
    /// The copy recipe preserves represented bits and every supported companion. Only
    /// one successful evaluation issues a candidate token for this binding.
    /// # Errors
    /// Refuses foreign bundles, repeated evaluation, exhausted identities or allocation.
    /// Independently completed copy claims remain in the unpublished failure record.
    pub fn evaluate(&self, inputs: NormalizedInputs<'_, P>) -> Result<Candidate<P>, Failure<P>> {
        let mut evidence = Vec::new();
        self.copy_inputs(inputs, &mut evidence, identity)
            .map_err(|cause| self.failure(cause, evidence))
    }

    /// Check the execution gate before touching any supplied view or allocating payloads.
    fn copy_inputs(
        &self,
        inputs: NormalizedInputs<'_, P>,
        evidence: &mut Vec<CopyEvidence>,
        allocate: impl FnOnce() -> Result<u64, crate::HeapError>,
    ) -> Result<Candidate<P>, OperationError> {
        if inputs.execution != self.execution {
            return Err(OperationError::ForeignInputs);
        }
        if self.issued.get().is_some() {
            return Err(OperationError::AlreadyEvaluated);
        }
        if inputs.views.len() != self.reservations.outputs.len() {
            return Err(OperationError::InputCount);
        }
        let token = allocate()?;
        let mut outputs = buffer(inputs.views.len())?;
        evidence
            .try_reserve_exact(inputs.views.len())
            .map_err(|_| crate::HeapError::Capacity)?;
        for (slot, (view, reservation)) in inputs
            .views
            .into_iter()
            .zip(&self.reservations.outputs)
            .enumerate()
        {
            let payload = copy_view(&view, *reservation)?;
            evidence.push(CopyEvidence {
                slot,
                entries: payload.extent.entries(),
                coverage: reservation.coverage(),
            });
            outputs.push(payload);
        }
        self.issued.set(Some(token));
        Ok(Candidate {
            execution: self.execution,
            token,
            revision: self.operation.revision(),
            outputs,
        })
    }

    /// Retain attempt facts without borrowing the execution or constructing success handles.
    pub(crate) fn failure(&self, cause: OperationError, evidence: Vec<CopyEvidence>) -> Failure<P> {
        Failure {
            record: Box::new(FailureRecord {
                cause,
                execution: self.execution,
                operation: self.operation.clone(),
                inputs: self.dependencies.clone(),
                evidence,
            }),
        }
    }
}

/// Copy one coherent-reference view and its companion in exactly logical column order.
fn copy_view<P: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    reservation: OutputReservation,
) -> Result<Payload<P>, OperationError> {
    let extent = view.extent();
    if extent != reservation.extent() {
        return Err(OperationError::CandidateContract);
    }
    let mut centers = buffer(extent.entries())?;
    let mut bounds = if reservation.coverage() == Coverage::DeclaredBounds {
        Some(buffer(extent.entries())?)
    } else {
        None
    };
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            centers.push(view.get(row, column)?);
            if let Some(values) = &mut bounds {
                values.push(
                    view.bound(row, column)?
                        .ok_or(OperationError::CandidateContract)?,
                );
            }
        }
    }
    crate::heap::publication::scan(extent, &centers, bounds.as_deref())?;
    Ok(Payload {
        extent,
        target: reservation.stamp(),
        centers,
        bounds,
    })
}

/// Isolated allocator exhaustion exercises the real candidate issuance gate.
#[cfg(test)]
mod tests {
    use crate::binding_testing::{TestResult, operation};
    use crate::heap::identity::allocate_identity;
    use crate::{Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationError};
    use nrz_units::Descriptor;
    use std::sync::atomic::AtomicU64;

    /// Exhausted candidate counters refuse without issuing a token or changing the heap.
    /// ´test:unit:candidate-counter-exhaustion-leaves-binding-retryable´
    #[test]
    fn candidate_counter_exhaustion_leaves_binding_retryable() -> TestResult {
        let counter = AtomicU64::new(u64::MAX - 1);
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
        let first = Binding::new(
            operation(1, 1)?,
            vec![source.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let mut evidence = Vec::new();
        let candidate = first.copy_inputs(first.normalized_inputs()?, &mut evidence, || {
            allocate_identity(&counter)
        })?;
        assert_eq!(candidate.token, u64::MAX - 1);
        drop(first);
        let second = Binding::new(
            operation(1, 1)?,
            vec![source.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        evidence.clear();
        assert!(matches!(
            second.copy_inputs(second.normalized_inputs()?, &mut evidence, || {
                allocate_identity(&counter)
            }),
            Err(OperationError::Heap(HeapError::CounterExhausted))
        ));
        assert_eq!(second.issued.get(), None);
        assert_eq!(evidence, []);
        let retry = second.evaluate(second.normalized_inputs()?)?;
        assert_eq!(second.publish(retry)?.output().len(), 1);
        Ok(())
    }
}
