// SPDX-License-Identifier: AGPL-3.0-only
//! Solve evaluation feeds the existing private candidate and atomic publication protocol.

use super::{FactorRef, SolveContract, SolveError, SolveEvidence};
use super::{RightHandSide, restoration::Restored};
use crate::binding::buffer;
use crate::solve_verification::SolveWork;
use crate::{
    Bound, Candidate, Failure, NormalizedInputs, NormalizedView, OperationError, RecipeEvidence,
};
use nrz_accuracy::BudgetDecision;
use nrz_scalar::{ArithmeticContext, Scalar};

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Evaluate with an opaque factor whose exact operator dependencies still match.
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
        let crate::binding::NativeRecipe::Solve(contract) = &self.operation.recipe else {
            return Err(SolveError::Contract.into());
        };
        factor.validate(
            self.dependencies
                .first()
                .ok_or(OperationError::InputCount)?,
            contract,
        )?;
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
        super::recovery::preflight(contract, self.statistics.work)?;
        let operator = views.first().ok_or(OperationError::InputCount)?;
        let input = self
            .dependencies
            .first()
            .ok_or(OperationError::InputCount)?;
        let factor = super::factor::build(operator, self.execution, input, contract)?;
        self.finish_solve(contract, views, &factor, token, evidence)
    }

    /// Preserve a valid over-budget proof in the failure record without issuing a candidate.
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
        if views.len() != contract.inputs.len()
            || P::PRECISION != Q::PRECISION
            || !reservation.bounded
            || !reservation.evaluation
            || reservation.contributions
        {
            return Err(SolveError::Contract.into());
        }
        let evaluated = if contract.policy.recovery().is_some() {
            super::recovery::evaluate(
                contract,
                views,
                factor,
                reservation,
                self.statistics.work,
                evidence,
            )
        } else {
            evaluate(contract, views, factor, reservation, evidence)
        };
        if let Some(RecipeEvidence::Solve(claim)) = evidence.last_mut() {
            claim.factor = Some(factor.used_by(self.execution));
        }
        let (restored, decision) = evaluated?;
        match decision {
            BudgetDecision::Exceeded => return Err(SolveError::BudgetExceeded.into()),
            BudgetDecision::Unresolved => return Err(SolveError::BudgetUnresolved.into()),
            BudgetDecision::Met | BudgetDecision::NotRequested => {}
        }
        let statistics = super::sensitivity::evaluate(self, contract, views, factor, evidence)?;
        let mut outputs = buffer(1)?;
        outputs.push(restored.payload(reservation)?);
        self.issued.set(Some(token));
        Ok(Candidate {
            execution: self.execution,
            token,
            revision: self.operation.revision(),
            outputs,
            evidence: evidence.clone(),
            statistics,
            private: None,
        })
    }
}

/// The same operator perturbation acts on every physical or typed-identity RHS column.
fn evaluate<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    reservation: crate::OutputReservation,
    evidence: &mut Vec<RecipeEvidence>,
) -> Result<(Restored<P>, BudgetDecision), OperationError> {
    let centers = initial(contract, views, factor)?;
    certify(contract, views, factor, centers, reservation, evidence)
}

/// Generate one finite initial candidate, independently of any budget acceptance.
pub(crate) fn initial<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
) -> Result<Vec<P>, OperationError> {
    if let Some(identity) = &contract.identity {
        factor.backend.inverse(identity.extent().rows())
    } else {
        factor
            .backend
            .solve(views.get(1).ok_or(OperationError::InputCount)?)
    }
}

/// Recompute every proof stage after each correction; residual improvement alone never accepts.
pub(crate) fn certify<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    centers: Vec<P>,
    reservation: crate::OutputReservation,
    evidence: &mut Vec<RecipeEvidence>,
) -> Result<(Restored<P>, BudgetDecision), OperationError> {
    let operator = views.first().ok_or(OperationError::InputCount)?;
    let rhs = if let Some(identity) = &contract.identity {
        RightHandSide::Identity(identity)
    } else {
        RightHandSide::Physical(views.get(1).ok_or(OperationError::InputCount)?)
    };
    let context = ArithmeticContext::new().map_err(SolveError::from)?;
    let certificate = crate::solve_verification::center(
        operator,
        &rhs,
        &centers,
        &factor.witness,
        &mut SolveWork::new(contract.work),
        &context,
    )?;
    evidence
        .try_reserve(1)
        .map_err(|_| crate::HeapError::Capacity)?;
    evidence.push(RecipeEvidence::Solve(Box::new(SolveEvidence::represented(
        contract,
        reservation,
        &factor.witness,
        &certificate,
        context.revision(),
    ))));
    let Some(RecipeEvidence::Solve(claim)) = evidence.last_mut() else {
        return Err(OperationError::CandidateContract);
    };
    let family =
        crate::solve_verification::family(operator, &rhs, &factor.witness, &certificate, &context)?;
    claim.include_family(&family, reservation.coverage());
    let restored = super::restoration::restore(
        contract,
        centers,
        family.radius,
        certificate.radius,
        reservation,
        &context,
    )?;
    let budget = contract.policy.assess(restored.total_norm)?;
    claim.complete(&restored, budget);
    Ok((restored, budget.decision()))
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
            || claim.is_inverse() != self.identity.is_some()
            || !claim.is_complete()
            || claim.factor_use().is_none_or(|usage| {
                usage.dependency().operator_revision() != self.operator_revision
                    || usage.dependency().precision() != Q::PRECISION
            })
            || !matches!(
                claim.budget().map(nrz_accuracy::BudgetAssessment::decision),
                Some(BudgetDecision::Met | BudgetDecision::NotRequested)
            )
            || claim.restoration().map(crate::ConversionEvidence::map)
                != self
                    .restoration
                    .as_ref()
                    .map(crate::ConversionContract::map)
        {
            return Err(OperationError::CandidateContract);
        }
        Ok(())
    }
}
