// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded same-factor corrections and explicit fresh-precision retry evidence.
//!
//! # Finite refinement and stopping evidence
//!
//! `SolvePolicy::with_recovery` authorizes a finite accuracy `RecoveryPolicy` and cumulative `Limits`. Each correction reuses the LU factors, forms an independently enclosed f64 residual, selects a finite correction RHS, and updates in the declared candidate format. The full inverse, residual, input-family, restoration and budget interpretation remains mandatory. `SolveEvidence::recovery` exposes every completed attempt, both precisions, certified residual and total radius, charged work and the explicit `SolveStop`. A refinement limit, stagnation or exhausted work returns a refusal with available evidence; the last iterate is never published as a success. Zero corrections and disabled promotion are supported.

use super::scratch::{Attempt, Stage};
use super::{FactorRef, SolveContract, SolveError, restoration::Restored};
use crate::private::Buffer;
use crate::{NormalizedView, OperationError, OutputReservation, RecipeEvidence};
use nrz_accuracy::{BudgetDecision, RecoveryPolicy, WorkBudget};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Precision, Scalar};

/// Why a bounded solve stopped; only met or absent budgets authorize publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveStop {
    /// The complete certified output radius meets the requested budget.
    BudgetMet,
    /// No numerical budget was requested.
    NotRequested,
    /// No further correction was authorized.
    RefinementLimit,
    /// No strict improvement within the configured window.
    Stagnation,
    /// Cumulative attempt or work allowance was exhausted.
    ResourceLimit,
    /// A correction or certificate could not be represented finitely.
    NonFinite,
    /// A mathematical or storage premise refused; its typed cause remains in the failure.
    CertificationFailure,
}
/// One completed, independently certified initial or correction attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveAttempt {
    /// Zero denotes the initial candidate; later indices count corrections.
    iteration: u32,
    /// Candidate and final theorem format.
    precision: Precision,
    /// Precision of the independent correction-residual calculation.
    residual_precision: Precision,
    /// Certified represented-center residual in normalized coordinates.
    residual: f64,
    /// Complete certified output radius, including declared inputs and restoration.
    radius: f64,
    /// Actual conservative budget comparison.
    decision: BudgetDecision,
}
impl SolveAttempt {
    /// Correction ordinal, with zero for the initial candidate.
    #[must_use]
    pub const fn iteration(&self) -> u32 {
        self.iteration
    }
    /// Candidate and final certificate format.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Independent residual format used to choose this correction.
    #[must_use]
    pub const fn residual_precision(&self) -> Precision {
        self.residual_precision
    }
    /// Full represented-center residual upper bound.
    #[must_use]
    pub const fn residual_norm(&self) -> f64 {
        self.residual
    }
    /// Certified total radius in actual output coordinates.
    #[must_use]
    pub const fn radius(&self) -> f64 {
        self.radius
    }
    /// Complete bound comparison; a smaller residual cannot override it.
    #[must_use]
    pub const fn budget(&self) -> BudgetDecision {
        self.decision
    }
}
/// Historical attempt sequence and finite resource use for one precision execution.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveRecoveryEvidence {
    /// Every completed certificate, in execution order.
    attempts: Vec<SolveAttempt>,
    /// Explicit reason no further correction is performed.
    stop: SolveStop,
    /// Reserved numerical solve work, including unsuccessful attempts.
    work: u64,
    /// Reserved attempts, including one that may fail before producing a certificate.
    reserved: u32,
}
impl SolveRecoveryEvidence {
    /// Ordered independent certificates; a failed final attempt may have no radius.
    #[must_use]
    pub fn attempts(&self) -> &[SolveAttempt] {
        &self.attempts
    }
    /// Final stopping reason.
    #[must_use]
    pub const fn stop(&self) -> SolveStop {
        self.stop
    }
    /// Cumulative charged work for this precision execution.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
    /// Reserved initial and correction attempts, including uncertified failures.
    #[must_use]
    pub const fn reserved_attempts(&self) -> u32 {
        self.reserved
    }
}
/// Mutable state stays private until the final certificate or explicit refusal is recorded.
struct Run {
    /// Full per-attempt work admitted on the cumulative recovery ledger.
    cost: u64,
    /// Finite configured correction limits.
    policy: RecoveryPolicy,
    /// Cumulative checked resource counter.
    budget: WorkBudget,
    /// Ordered completed certificates and termination facts.
    evidence: SolveRecoveryEvidence,
    /// Best total radius and consecutive nonimprovements.
    best: Option<f64>,
    /// Count since the last strict improvement.
    stagnant: u32,
}

/// Attempt admission precedes candidate and correction work; no last iterate escapes on refusal.
pub(crate) fn evaluate<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    reservation: OutputReservation,
    statistical_work: usize,
    evidence: &mut Vec<RecipeEvidence>,
    scratch: &super::scratch::Run,
) -> Result<(Restored<P>, BudgetDecision), OperationError> {
    let (policy, limits) = contract.policy.recovery().ok_or(SolveError::Contract)?;
    let cost = contract
        .work
        .checked_add(statistical_work)
        .ok_or(SolveError::ResourceLimit)?;
    let cost = u64::try_from(cost).map_err(|_| SolveError::ResourceLimit)?;
    let mut run = Run {
        cost,
        policy,
        budget: limits.start(),
        best: None,
        stagnant: 0,
        evidence: SolveRecoveryEvidence {
            attempts: Vec::new(),
            stop: SolveStop::CertificationFailure,
            work: 0,
            reserved: 0,
        },
    };
    let result = iterations(
        contract,
        views,
        factor,
        reservation,
        &mut run,
        evidence,
        scratch,
    );
    if let Err(cause) = &result {
        run.evidence.stop = stop_for(cause);
    }
    run.evidence.work = run.budget.work();
    run.evidence.reserved = run.budget.attempts();
    if let Some(RecipeEvidence::Solve(claim)) = evidence.last_mut() {
        claim.recovery = Some(run.evidence);
    }
    result
}
/// Each iteration replaces the final proof only after retaining the previous attempt's facts.
fn iterations<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    reservation: OutputReservation,
    run: &mut Run,
    evidence: &mut Vec<RecipeEvidence>,
    scratch: &super::scratch::Run,
) -> Result<(Restored<P>, BudgetDecision), OperationError> {
    run.budget
        .reserve_attempt(run.cost)
        .map_err(|_| SolveError::ResourceLimit)?;
    let mut centers = super::evaluate::initial(contract, views, factor, scratch)?;
    let mut iteration = 0;
    loop {
        let attempt = Attempt {
            run: scratch,
            index: usize::try_from(iteration).map_err(|_| SolveError::ResourceLimit)?,
        };
        let mut retained = attempt.buffer(Stage::RetainedCenters)?;
        for &value in &centers {
            retained.push(value)?;
        }
        let mut current = Vec::new();
        let result = super::evaluate::certify(
            contract,
            views,
            factor,
            retained,
            reservation,
            &mut current,
            attempt,
        );
        if !current.is_empty() {
            *evidence = current;
        }
        let (restored, decision) = result?;
        let Some(RecipeEvidence::Solve(claim)) = evidence.last() else {
            return Err(SolveError::Contract.into());
        };
        run.record::<P>(iteration, claim, decision)?;
        if let Some(stop) = run.stopping(iteration, decision) {
            run.evidence.stop = stop;
            return match stop {
                SolveStop::BudgetMet | SolveStop::NotRequested => Ok((restored, decision)),
                SolveStop::Stagnation => Err(SolveError::Stagnation.into()),
                _ => Err(SolveError::RefinementLimit.into()),
            };
        }
        run.budget
            .reserve_attempt(run.cost)
            .map_err(|_| SolveError::ResourceLimit)?;
        drop(restored);
        iteration = iteration.checked_add(1).ok_or(SolveError::ResourceLimit)?;
        centers = correction(
            contract,
            views,
            factor,
            &centers,
            Attempt {
                run: scratch,
                index: usize::try_from(iteration).map_err(|_| SolveError::ResourceLimit)?,
            },
        )?;
    }
}
impl Run {
    /// Stagnation is measured by complete output certificates, including physical input floors.
    fn record<P: Scalar>(
        &mut self,
        iteration: u32,
        claim: &super::SolveEvidence,
        decision: BudgetDecision,
    ) -> Result<(), OperationError> {
        let radius = claim.output_radius().ok_or(SolveError::Contract)?;
        if self.best.is_none_or(|best| radius < best) {
            self.best = Some(radius);
            self.stagnant = 0;
        } else {
            self.stagnant = self
                .stagnant
                .checked_add(1)
                .ok_or(SolveError::ResourceLimit)?;
        }
        self.evidence
            .attempts
            .try_reserve(1)
            .map_err(|_| SolveError::ResourceLimit)?;
        self.evidence.attempts.push(SolveAttempt {
            iteration,
            precision: P::PRECISION,
            residual_precision: if iteration == 0 {
                P::PRECISION
            } else {
                Precision::F64
            },
            residual: claim.residual_norm(),
            radius,
            decision,
        });
        Ok(())
    }
    /// Success precedes stagnation and the configured maximum correction count.
    const fn stopping(&self, iteration: u32, decision: BudgetDecision) -> Option<SolveStop> {
        match decision {
            BudgetDecision::Met => Some(SolveStop::BudgetMet),
            BudgetDecision::NotRequested => Some(SolveStop::NotRequested),
            BudgetDecision::Exceeded | BudgetDecision::Unresolved => {
                if self.stagnant >= self.policy.stagnation_window() {
                    Some(SolveStop::Stagnation)
                } else if iteration >= self.policy.max_refinements() {
                    Some(SolveStop::RefinementLimit)
                } else {
                    None
                }
            }
        }
    }
}
/// Preserve typed numerical causes while classifying the explicit recovery termination.
const fn stop_for(cause: &OperationError) -> SolveStop {
    match cause {
        OperationError::Solve(SolveError::RefinementLimit) => SolveStop::RefinementLimit,
        OperationError::Solve(SolveError::Stagnation) => SolveStop::Stagnation,
        OperationError::Solve(SolveError::ResourceLimit) | OperationError::ResourceLimit => {
            SolveStop::ResourceLimit
        }
        OperationError::Solve(SolveError::CandidateFailure | SolveError::Arithmetic(_))
        | OperationError::Statistics(crate::StatisticsError::Arithmetic(_)) => SolveStop::NonFinite,
        _ => SolveStop::CertificationFailure,
    }
}

/// Binary64 residuals improve correction candidates; the final theorem still checks the declared format.
pub(crate) fn correction<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    centers: &[P],
    attempt: Attempt<'_>,
) -> Result<Buffer<P>, OperationError> {
    let extent = contract.solution.shape().extent();
    let context = ArithmeticContext::new().map_err(SolveError::from)?;
    let operator = views.first().ok_or(SolveError::Contract)?;
    let mut rhs = attempt.buffer(Stage::CorrectionRightHandSide)?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let initial = if contract.identity.is_some() {
                if row == column { 1.0 } else { 0.0 }
            } else {
                views
                    .get(1)
                    .ok_or(SolveError::Contract)?
                    .get(row, column)?
                    .to_f64()
            };
            let mut residual = point(initial)?;
            for index in 0..extent.rows() {
                let center = centers
                    .get(extent.index(index, column)?)
                    .ok_or(SolveError::Contract)?;
                residual = residual
                    .sub(
                        point(operator.get(row, index)?.to_f64())?
                            .mul(point(center.to_f64())?, &context)?,
                        &context,
                    )
                    .map_err(SolveError::from)?;
            }
            rhs.push(residual_center(residual, &context)?)?;
        }
    }
    let delta =
        factor
            .resource
            .backend
            .correction_planned(extent, &rhs, attempt.run, attempt.index)?;
    let mut updated = attempt.buffer(Stage::UpdatedCenters)?;
    for (&center, &update) in centers.iter().zip(&delta) {
        updated.push(
            Finite::new(center + update)
                .map_err(SolveError::from)?
                .get(),
        )?;
    }
    Ok(updated)
}
/// A finite endpoint of the enclosed residual is an eligible private correction candidate.
fn residual_center<P: Scalar + 'static>(
    residual: Enclosure<f64>,
    context: &ArithmeticContext,
) -> Result<P, OperationError> {
    match P::PRECISION {
        Precision::F64 => crate::conversion::same_format(residual.lower().get()),
        Precision::F32 => {
            let narrowed = nrz_scalar::narrow(
                residual.lower(),
                Enclosure::singleton(residual.lower()),
                context,
            )
            .map_err(SolveError::from)?;
            crate::conversion::same_format(narrowed.center().get())
        }
    }
}
/// Exact widening of a represented input is separate from every residual arithmetic operation.
fn point(value: f64) -> Result<Enclosure<f64>, OperationError> {
    Ok(Enclosure::singleton(
        Finite::new(value).map_err(SolveError::from)?,
    ))
}

/// Refuse impossible cumulative allowances before creating any backend factor.
pub(crate) fn preflight(
    contract: &SolveContract,
    statistical_work: usize,
) -> Result<(), OperationError> {
    if let Some((_, limits)) = contract.policy.recovery() {
        let cost = contract
            .work
            .checked_add(statistical_work)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(SolveError::ResourceLimit)?;
        if limits.max_attempts() == 0 || limits.max_work() < cost {
            return Err(SolveError::ResourceLimit.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Run, SolveRecoveryEvidence, SolveStop};
    use crate::solve_testing::TestResult;
    use nrz_accuracy::{BudgetDecision, Limits, Promotion, RecoveryPolicy};

    /// The private transition gives budget success priority over simultaneous finite stop limits.
    /// ´test:unit:solve-recovery-success-precedes-stagnation-and-limit´
    #[test]
    fn solve_recovery_success_precedes_stagnation_and_limit() -> TestResult {
        let run = Run {
            cost: 1,
            policy: RecoveryPolicy::new(0, 1, Promotion::Never)?,
            budget: Limits::new(1, 1).start(),
            evidence: SolveRecoveryEvidence {
                attempts: Vec::new(),
                stop: SolveStop::CertificationFailure,
                work: 0,
                reserved: 0,
            },
            best: Some(1.0),
            stagnant: 1,
        };
        assert_eq!(
            run.stopping(0, BudgetDecision::Met),
            Some(SolveStop::BudgetMet)
        );
        assert_eq!(
            run.stopping(0, BudgetDecision::NotRequested),
            Some(SolveStop::NotRequested)
        );
        assert_eq!(
            run.stopping(0, BudgetDecision::Exceeded),
            Some(SolveStop::Stagnation)
        );
        Ok(())
    }
}
