// SPDX-License-Identifier: AGPL-3.0-only
//! Solve evaluation feeds the existing private candidate and atomic publication protocol.

use super::{FactorRef, SolveContract, SolveError, SolveEvidence};
use crate::binding::buffer;
use crate::publication::Payload;
use crate::solve_verification::SolveWork;
use crate::{
    Bound, Candidate, Failure, NormalizedInputs, NormalizedView, OperationError, RecipeEvidence,
};
use nrz_scalar::{ArithmeticContext, Scalar};

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Evaluate with an opaque factor belonging to this same validated execution.
    /// # Errors
    /// Refuses foreign factors or inputs, repeated evaluation, failed residual proof
    /// or any incomplete result. No candidate or output handle escapes on failure.
    pub fn evaluate_factored(
        &self,
        inputs: &NormalizedInputs<'_, P>,
        factor: &FactorRef<P>,
    ) -> Result<Candidate<Q>, Failure<P>> {
        let mut evidence = Vec::new();
        self.factored(inputs, factor, &mut evidence)
            .map_err(|cause| self.failure(cause, evidence))
    }

    /// Apply the same execution and single-issuance gate as ordinary evaluation.
    fn factored(
        &self,
        inputs: &NormalizedInputs<'_, P>,
        factor: &FactorRef<P>,
        evidence: &mut Vec<RecipeEvidence>,
    ) -> Result<Candidate<Q>, OperationError> {
        if inputs.execution() != self.execution {
            return Err(OperationError::ForeignInputs);
        }
        if self.issued.get().is_some() {
            return Err(OperationError::AlreadyEvaluated);
        }
        if factor.execution != self.execution {
            return Err(SolveError::ForeignFactor.into());
        }
        let crate::binding::NativeRecipe::Solve(contract) = &self.operation.recipe else {
            return Err(SolveError::Contract.into());
        };
        let token = crate::heap::identity::identity()?;
        self.finish_solve(contract, inputs.views(), factor, token, evidence)
    }

    /// Enter from the ordinary transaction gate after checking input execution and token state.
    pub(crate) fn solve_candidate(
        &self,
        contract: &SolveContract,
        views: &[NormalizedView<'_, P>],
        token: u64,
        evidence: &mut Vec<RecipeEvidence>,
    ) -> Result<Candidate<Q>, OperationError> {
        let operator = views.first().ok_or(OperationError::InputCount)?;
        let stamp = self
            .dependencies
            .first()
            .ok_or(OperationError::InputCount)?
            .stamp();
        let factor = super::factor::build(operator, self.execution, stamp, contract.work)?;
        self.finish_solve(contract, views, &factor, token, evidence)
    }

    /// Finish the independent center proof before issuing the one private candidate.
    fn finish_solve(
        &self,
        contract: &SolveContract,
        views: &[NormalizedView<'_, P>],
        factor: &FactorRef<P>,
        token: u64,
        evidence: &mut Vec<RecipeEvidence>,
    ) -> Result<Candidate<Q>, OperationError> {
        let reservation = *self
            .reservations
            .outputs
            .first()
            .ok_or(OperationError::CandidateContract)?;
        if views.len() != 2
            || P::PRECISION != Q::PRECISION
            || !reservation.bounded
            || !reservation.evaluation
            || reservation.contributions
        {
            return Err(SolveError::Contract.into());
        }
        let operator = views.first().ok_or(OperationError::InputCount)?;
        let rhs = views.get(1).ok_or(OperationError::InputCount)?;
        let context = ArithmeticContext::new().map_err(SolveError::from)?;
        let centers = factor.backend.solve(rhs)?;
        let certificate = crate::solve_verification::center(
            operator,
            rhs,
            &centers,
            &factor.witness,
            &mut SolveWork::new(contract.work),
            &context,
        )?;
        let claim = SolveEvidence::new(
            reservation.extent(),
            reservation.coverage(),
            contract.work,
            &factor.witness,
            &certificate,
            context.revision(),
        );
        let payload = payload(&centers, certificate.radius, reservation)?;
        evidence
            .try_reserve(1)
            .map_err(|_| crate::HeapError::Capacity)?;
        evidence.push(RecipeEvidence::Solve(claim));
        let mut outputs = buffer(1)?;
        outputs.push(payload);
        self.issued.set(Some(token));
        Ok(Candidate {
            execution: self.execution,
            token,
            revision: self.operation.revision(),
            outputs,
            evidence: evidence.clone(),
        })
    }
}

/// Restore coherent unit-reference coordinates exactly and expand a norm radius per entry.
fn payload<P: Scalar + 'static, Q: Scalar + 'static>(
    centers: &[P],
    radius: P,
    reservation: crate::OutputReservation,
) -> Result<Payload<Q>, OperationError> {
    if centers.len() != reservation.extent().entries() {
        return Err(SolveError::Contract.into());
    }
    let mut values = buffer(centers.len())?;
    let mut bounds = buffer(centers.len())?;
    let mut evaluation = buffer(centers.len())?;
    for &center in centers {
        values.push(crate::conversion::same_format(center)?);
        bounds.push(crate::conversion::same_format(radius)?);
        evaluation.push(crate::conversion::same_format(radius)?);
    }
    Ok(Payload {
        extent: reservation.extent(),
        target: reservation.stamp(),
        centers: values,
        bounds: Some(bounds),
        evaluation: Some(evaluation),
        contributions: None,
    })
}

impl SolveContract {
    /// Check the private proof's exact recipe association before publication.
    pub(crate) fn verify<Q: Scalar>(
        &self,
        evidence: Option<&RecipeEvidence>,
    ) -> Result<(), OperationError> {
        let Some(RecipeEvidence::Solve(claim)) = evidence else {
            return Err(OperationError::CandidateContract);
        };
        if claim.extent != self.output.shape().extent()
            || claim.precision() != Q::PRECISION
            || claim.arithmetic_revision() != 1
            || claim.work() != self.work
        {
            return Err(OperationError::CandidateContract);
        }
        Ok(())
    }
}
