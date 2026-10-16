// SPDX-License-Identifier: AGPL-3.0-only
//! Identity normalization and exact copy evaluation perform no floating arithmetic.

use super::{Candidate, Payload};
use crate::binding::buffer;
use crate::heap::identity::identity;
use crate::{
    Bound, CopyEvidence, Failure, FailureRecord, NormalizedInputs, NormalizedView, OperationError,
    OutputReservation,
};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Evaluate this binding's operation-ordered normalized inputs into private scratch.
    ///
    /// The copy recipe preserves represented bits and every supported companion. Only
    /// one successful evaluation issues a candidate token for this binding.
    /// # Errors
    /// Refuses foreign bundles, repeated evaluation, exhausted identities or allocation.
    /// Independently completed copy claims remain in the unpublished failure record.
    pub fn evaluate(&self, inputs: NormalizedInputs<'_, P>) -> Result<Candidate<Q>, Failure<P>> {
        let mut evidence = Vec::new();
        self.copy_inputs(inputs, &mut evidence, identity)
            .map_err(|cause| self.failure(cause, evidence))
    }

    /// Check the execution gate before touching any supplied view or allocating payloads.
    fn copy_inputs(
        &self,
        inputs: NormalizedInputs<'_, P>,
        evidence: &mut Vec<crate::RecipeEvidence>,
        allocate: impl FnOnce() -> Result<u64, crate::HeapError>,
    ) -> Result<Candidate<Q>, OperationError> {
        if inputs.execution != self.execution {
            return Err(OperationError::ForeignInputs);
        }
        if self.issued.get().is_some() {
            return Err(OperationError::AlreadyEvaluated);
        }
        if inputs.views.len() != self.operation.inputs.len() {
            return Err(OperationError::InputCount);
        }
        let token = allocate()?;
        if let crate::binding::NativeRecipe::Kernel(contract) = &self.operation.recipe {
            return self.kernel_candidate(contract, &inputs.views, token, evidence);
        }
        let mut outputs = buffer(inputs.views.len())?;
        evidence
            .try_reserve_exact(inputs.views.len())
            .map_err(|_| crate::HeapError::Capacity)?;
        let context = match self.operation.recipe {
            crate::binding::NativeRecipe::Copy => None,
            crate::binding::NativeRecipe::Kernel(_)
            | crate::binding::NativeRecipe::Affine(_)
            | crate::binding::NativeRecipe::Precision { .. } => {
                Some(nrz_scalar::ArithmeticContext::new().map_err(crate::ConversionError::from)?)
            }
        };
        let mut cache = buffer(inputs.views.len())?;
        for (slot, (view, reservation)) in inputs
            .views
            .into_iter()
            .zip(&self.reservations.outputs)
            .enumerate()
        {
            let (payload, claim) =
                self.evaluate_slot(slot, &view, *reservation, &mut cache, context.as_ref())?;
            evidence.push(claim);
            outputs.push(payload);
        }
        self.issued.set(Some(token));
        Ok(Candidate {
            execution: self.execution,
            token,
            revision: self.operation.revision(),
            outputs,
            evidence: evidence.clone(),
        })
    }

    /// Dispatch only crate-defined numerical schedules after the common execution gate.
    fn evaluate_slot(
        &self,
        slot: usize,
        view: &NormalizedView<'_, P>,
        reservation: OutputReservation,
        cache: &mut Vec<crate::coordinate_numeric::LoweredMap<Q>>,
        context: Option<&nrz_scalar::ArithmeticContext>,
    ) -> Result<(Payload<Q>, crate::RecipeEvidence), OperationError> {
        match &self.operation.recipe {
            crate::binding::NativeRecipe::Kernel(_) => Err(OperationError::CandidateContract),
            crate::binding::NativeRecipe::Copy => {
                let payload = copy_view(view, reservation)?;
                let claim = crate::RecipeEvidence::Copy(CopyEvidence {
                    slot,
                    entries: payload.extent.entries(),
                    coverage: reservation.coverage(),
                });
                Ok((payload, claim))
            }
            crate::binding::NativeRecipe::Affine(contracts) => crate::conversion::evaluate(
                view,
                reservation,
                contracts
                    .get(slot)
                    .ok_or(OperationError::CandidateContract)?,
                cache,
                context.ok_or(OperationError::CandidateContract)?,
                slot,
            ),
            crate::binding::NativeRecipe::Precision { .. } => {
                crate::conversion::precision::evaluate(
                    view,
                    reservation,
                    context.ok_or(OperationError::CandidateContract)?,
                    slot,
                )
            }
        }
    }

    /// Retain attempt facts without borrowing the execution or constructing success handles.
    pub(crate) fn failure(
        &self,
        cause: OperationError,
        evidence: Vec<crate::RecipeEvidence>,
    ) -> Failure<P> {
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
fn copy_view<P: Scalar + 'static, Q: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    reservation: OutputReservation,
) -> Result<Payload<Q>, OperationError> {
    let extent = view.extent();
    if extent != reservation.extent() {
        return Err(OperationError::CandidateContract);
    }
    let mut centers = buffer(extent.entries())?;
    let mut bounds = if reservation.bounded {
        Some(buffer(extent.entries())?)
    } else {
        None
    };
    let mut evaluation = if reservation.evaluation {
        Some(buffer(extent.entries())?)
    } else {
        None
    };
    let mut contributions = if reservation.contributions {
        Some([
            buffer(extent.entries())?,
            buffer(extent.entries())?,
            buffer(extent.entries())?,
        ])
    } else {
        None
    };
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            centers.push(crate::conversion::same_format(view.get(row, column)?)?);
            if let Some(regions) = &mut contributions {
                for (region, term) in regions.iter_mut().zip(crate::ProductTerm::ALL) {
                    region.push(crate::conversion::same_format(
                        view.contribution(term, row, column)?
                            .ok_or(OperationError::CandidateContract)?,
                    )?);
                }
            }
            if let Some(values) = &mut evaluation {
                values.push(crate::conversion::same_format(
                    view.evaluation(row, column)?
                        .ok_or(OperationError::CandidateContract)?,
                )?);
            }
            if let Some(values) = &mut bounds {
                values.push(crate::conversion::same_format(
                    view.bound(row, column)?
                        .ok_or(OperationError::CandidateContract)?,
                )?);
            }
        }
    }
    crate::heap::publication::scan(extent, &centers, bounds.as_deref())?;
    Ok(Payload {
        extent,
        target: reservation.stamp(),
        centers,
        bounds,
        evaluation,
        contributions,
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
