// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical evaluation produces only private candidates and complete companion payloads.

use super::arithmetic::{Entry, operand};
use super::contract::Recipe;
use super::{KernelContract, KernelError, KernelEvidence};
use crate::binding::buffer;
use crate::publication::Payload;
use crate::{NormalizedView, OperationError, OutputReservation, RecipeEvidence};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Dispatch a checked kernel with every operand borrowed under the same execution lease.
pub(crate) fn evaluate<P: Scalar + 'static, Q: Scalar + 'static>(
    contract: &KernelContract,
    views: &[NormalizedView<'_, P>],
    reservation: OutputReservation,
) -> Result<(Payload<Q>, RecipeEvidence), OperationError> {
    if views.len() != contract.inputs.len()
        || reservation.extent() != contract.output.shape().extent()
        || !reservation.bounded
        || !reservation.evaluation
        || P::PRECISION != Q::PRECISION
    {
        return Err(KernelError::Contract.into());
    }
    let context = ArithmeticContext::new().map_err(KernelError::from)?;
    let entries = match &contract.recipe {
        Recipe::Sum { subtract, right } => sum(views, *subtract, right, &context)?,
        Recipe::Transform(map) => transform(views, map)?,
        Recipe::Linear(map) => super::linear_evaluate::evaluate(map, first(views)?, &context)?,
    };
    let mut centers = buffer(entries.len())?;
    let mut totals = buffer(entries.len())?;
    let mut errors = buffer(entries.len())?;
    for entry in entries {
        centers.push(crate::conversion::same_format(entry.center)?);
        totals.push(crate::conversion::same_format(entry.total)?);
        errors.push(crate::conversion::same_format(entry.error)?);
    }
    let evidence = KernelEvidence {
        kind: contract.kind(),
        entries: reservation.extent().entries(),
        coverage: reservation.coverage(),
        precision: Q::PRECISION,
        revision: context.revision(),
        work: contract.work,
    };
    Ok((
        Payload {
            extent: reservation.extent(),
            target: reservation.stamp(),
            centers,
            bounds: Some(totals),
            evaluation: Some(errors),
        },
        RecipeEvidence::Kernel(evidence),
    ))
}

/// Evaluate the selected right operand in the checked common output order.
fn sum<P: Scalar + 'static>(
    views: &[NormalizedView<'_, P>],
    subtract: bool,
    map: &crate::EntryMap,
    context: &ArithmeticContext,
) -> Result<Vec<Entry<P>>, OperationError> {
    let left = first(views)?;
    let right = views.get(1).ok_or(OperationError::InputCount)?;
    let mut result = buffer(map.target().entries())?;
    for (index, source) in map.indices().iter().enumerate() {
        result.push(super::arithmetic::sum(
            operand(left, index)?,
            operand(right, *source)?,
            subtract,
            context,
        )?);
    }
    Ok(result)
}

/// Copy or repeat each selected center and total, with zero newly introduced arithmetic error.
fn transform<P: Scalar + 'static>(
    views: &[NormalizedView<'_, P>],
    map: &crate::EntryMap,
) -> Result<Vec<Entry<P>>, OperationError> {
    let input = first(views)?;
    let mut result = buffer(map.target().entries())?;
    for source in map.indices() {
        let (center, radius) = operand(input, *source)?;
        result.push(Entry {
            center,
            total: radius.get(),
            error: P::ZERO,
        });
    }
    Ok(result)
}

/// Locate the one operand required by unary kernels after checking the closed input count.
fn first<'a, 'b, P: Scalar>(
    views: &'a [NormalizedView<'b, P>],
) -> Result<&'a NormalizedView<'b, P>, OperationError> {
    views.first().ok_or(OperationError::InputCount)
}

impl<P: Scalar + 'static, Q: Scalar + 'static> crate::Bound<'_, P, Q> {
    /// Issue one candidate only after the shared execution gate has checked the borrowed views.
    pub(crate) fn kernel_candidate(
        &self,
        contract: &KernelContract,
        views: &[NormalizedView<'_, P>],
        token: u64,
        evidence: &mut Vec<RecipeEvidence>,
    ) -> Result<crate::Candidate<Q>, OperationError> {
        let reservation = *self
            .reservations
            .outputs
            .first()
            .ok_or(OperationError::CandidateContract)?;
        let (payload, claim) = evaluate(contract, views, reservation)?;
        let mut outputs = buffer(1)?;
        evidence
            .try_reserve(1)
            .map_err(|_| crate::HeapError::Capacity)?;
        outputs.push(payload);
        evidence.push(claim);
        self.issued.set(Some(token));
        Ok(crate::Candidate {
            execution: self.execution,
            token,
            revision: self.operation.revision(),
            outputs,
            evidence: evidence.clone(),
        })
    }
}
