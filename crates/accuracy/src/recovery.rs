// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded recovery transitions and inspectable stopping evidence.
//!
//! # Recovery, retry and verification contracts
//!
//! `RecoveryPolicy` sets a maximum number of refinements after the initial execution at each precision, a positive stagnation window and `Promotion::Never` or `Promotion::F32ToF64`. Zero refinements is valid. Starting a `Recovery` fixes its budget and cumulative limits. `begin_attempt` reserves resources before a kernel; `charge` covers further work; `complete` accepts a caller-certified radius and recomputes the full budget comparison. `fail` records an arithmetic refusal. Invalid phase transitions return errors without advancing counters. Recovery owns only numerical policy state: it executes no solver, retains no numerical buffers and grants no physical assurance.
//!
//! Budget success stops immediately; absence of a request stops after the first available bound as `NotRequested`. Otherwise, consecutive attempts without a strictly smaller best certified radius trigger stagnation, followed by the per-precision refinement limit. A decrease back to an old best does not reset that window. Resource exhaustion and arithmetic failure stop explicitly. `RecoveryEvidence` records precision, total attempts/work, refinements, stagnation count, best/last available radii, the last available comparison and the stopping reason. A failed threshold evaluation retains its newly supplied bound with no comparison. A failed attempt without a new bound retains earlier independently valid evidence. The heap owner retains snapshots per attempt with target/version and algorithm records; a reduced residual alone is never a success condition.
//!
//! A stopped binary32 run may consume itself through `retry_f64` only when promotion was authorized and its stop was a refinement limit, stagnation, nonfinite arithmetic or overflow. This returns a fresh binary64 policy run together with the earlier evidence. It transfers cumulative limits and charged work, widens the fixed budget exactly, and resets per-precision progress without carrying forward a result bound. The caller explicitly converts inputs with their existing error contributions and computes fresh certificates. Binary64 runs expose no further promotion. Successful, unrequested, resource-stopped and unsupported-environment outcomes cannot authorize retry.
//!
//! Budget tests include exact absolute/relative equality, both sides of the threshold, interval overlap, absent requests, invalid reference scales, overflow and independent rational examples where a rounded-up tolerance would falsely accept. Recovery tests cover state transitions, exact resource limits, integer overflow, zero refinement, strict progress and stagnation, nonfinite attempts, retained failure evidence and explicit precision retry. Public vector-to-policy lifecycles compare every norm to an exact rational oracle before a decision; retry tests preserve coefficient widths and cumulative charges. An inline private test covers the otherwise inaccessible exhausted attempt counter; all externally expressible assertions live in the public test target.

use crate::{
    AccuracyFailure, Budget, BudgetAssessment, BudgetDecision, Limits, Nonnegative, Promotion,
    RecoveryPolicy, ResourceLimit, WorkBudget, assess_budget,
};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Precision, Scalar};

/// Why the policy ended an execution at its current precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The certified upper radius meets the requested tolerance.
    BudgetMet,
    /// No budget was requested; the first available bound ends recovery.
    NotRequested,
    /// The configured number of refinement attempts has been used.
    RefinementLimit,
    /// No new best certified radius appeared within the configured window.
    Stagnation,
    /// A cumulative work or attempt reservation failed.
    ResourceLimit(ResourceLimit),
    /// The caller's arithmetic or tolerance evaluation failed.
    Arithmetic(ArithmeticFailure),
}

/// Current phase of a recovery run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStatus {
    /// The caller may reserve an initial or refinement attempt.
    Ready,
    /// One reserved attempt awaits a bound or an arithmetic failure.
    Running,
    /// No further attempt at this precision is permitted.
    Stopped(StopReason),
}

/// Per-precision progress, without any physical evidence or numerical buffers.
#[derive(Debug, Clone, Copy)]
struct Progress<P: Scalar> {
    /// Current legal transition phase.
    status: RecoveryStatus,
    /// Reserved refinements after the initial execution at this precision.
    refinements: u32,
    /// Attempts since the last strictly improved best certified radius.
    stagnant: u32,
    /// Smallest certified upper radius supplied at this precision.
    best: Option<Nonnegative<P>>,
    /// Most recently supplied certified upper radius, even if comparison failed.
    last_upper: Option<Nonnegative<P>>,
    /// Comparison of the most recently supplied bound, if evaluation succeeded.
    assessment: Option<BudgetAssessment<P>>,
}

/// A snapshot of numerical stopping evidence; it grants no physical assurance.
#[derive(Debug, Clone, Copy)]
pub struct RecoveryEvidence<P: Scalar> {
    /// Progress at the snapshot's precision.
    progress: Progress<P>,
    /// Total attempts reserved across both precisions, including failed attempts.
    attempts: u32,
    /// Total work reserved across both precisions.
    work: u64,
    /// Original finite cumulative limits.
    limits: Limits,
}

impl<P: Scalar> RecoveryEvidence<P> {
    /// Precision of the recorded radii and refinements.
    #[must_use]
    pub const fn precision(self) -> Precision {
        P::PRECISION
    }

    /// Current phase or explicit stopping reason.
    #[must_use]
    pub const fn status(self) -> RecoveryStatus {
        self.progress.status
    }

    /// Total reserved attempts across executions, including failures and retry.
    #[must_use]
    pub const fn attempts(self) -> u32 {
        self.attempts
    }

    /// Total charged work across executions.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }

    /// Finite cumulative allowances.
    #[must_use]
    pub const fn limits(self) -> Limits {
        self.limits
    }

    /// Reserved refinements at this precision, excluding its initial execution.
    #[must_use]
    pub const fn refinements(self) -> u32 {
        self.progress.refinements
    }

    /// Consecutive supplied bounds that did not improve the best radius.
    #[must_use]
    pub const fn stagnant_attempts(self) -> u32 {
        self.progress.stagnant
    }

    /// Best available certified upper radius at this precision.
    #[must_use]
    pub const fn best_upper(self) -> Option<Nonnegative<P>> {
        self.progress.best
    }

    /// Most recent supplied radius, retained even when tolerance arithmetic fails.
    #[must_use]
    pub const fn last_upper(self) -> Option<Nonnegative<P>> {
        self.progress.last_upper
    }

    /// Last bound's budget comparison, absent if no bound or tolerance was available.
    #[must_use]
    pub const fn assessment(self) -> Option<BudgetAssessment<P>> {
        self.progress.assessment
    }
}

/// An allocation-free policy run with a fixed budget and checked cumulative work.
///
/// The caller performs every numerical solve, independently certifies each radius
/// and retains attempt snapshots with target/version evidence in its own records.
/// Residual improvement alone cannot complete an attempt successfully.
#[derive(Debug)]
pub struct Recovery<P: Scalar> {
    /// Fixed bounded refinement and retry policy.
    policy: RecoveryPolicy,
    /// Fixed mathematical budget for this execution.
    budget: Option<Budget<P>>,
    /// Counters carried unchanged into a permitted precision retry.
    work: WorkBudget,
    /// Current per-precision progress.
    progress: Progress<P>,
}

impl<P: Scalar> Recovery<P> {
    /// Start a precision execution, retaining supplied cumulative counters.
    pub(crate) const fn new(
        policy: RecoveryPolicy,
        budget: Option<Budget<P>>,
        work: WorkBudget,
    ) -> Self {
        Self {
            policy,
            budget,
            work,
            progress: Progress {
                status: RecoveryStatus::Ready,
                refinements: 0,
                stagnant: 0,
                best: None,
                last_upper: None,
                assessment: None,
            },
        }
    }

    /// Snapshot counters, available bounds and the current stopping evidence.
    #[must_use]
    pub const fn evidence(&self) -> RecoveryEvidence<P> {
        RecoveryEvidence {
            progress: self.progress,
            attempts: self.work.attempts(),
            work: self.work.work(),
            limits: self.work.limits(),
        }
    }

    /// The fixed budget for this precision execution.
    #[must_use]
    pub const fn budget(&self) -> Option<&Budget<P>> {
        self.budget.as_ref()
    }

    /// Reserve one attempt and its known work before entering a numerical kernel.
    ///
    /// # Errors
    /// Refuses a non-ready state or exhausted resources. Resource refusal stops
    /// recovery without changing counters; an invalid transition changes nothing.
    pub fn begin_attempt(&mut self, work: u64) -> Result<u32, AccuracyFailure> {
        if self.progress.status != RecoveryStatus::Ready {
            return Err(AccuracyFailure::InvalidRecoveryState);
        }
        let ordinal = self.work.reserve_attempt(work).map_err(|limit| {
            self.progress.status = RecoveryStatus::Stopped(StopReason::ResourceLimit(limit));
            AccuracyFailure::ResourceLimit(limit)
        })?;
        if self.progress.last_upper.is_some() {
            // A ready run is strictly below its finite refinement limit.
            self.progress.refinements = self.progress.refinements.saturating_add(1);
        }
        self.progress.status = RecoveryStatus::Running;
        Ok(ordinal)
    }

    /// Charge further verification or kernel work before performing it.
    ///
    /// # Errors
    /// Refuses a non-running state or excessive work; resource refusal stops the run.
    pub fn charge(&mut self, work: u64) -> Result<(), AccuracyFailure> {
        if self.progress.status != RecoveryStatus::Running {
            return Err(AccuracyFailure::InvalidRecoveryState);
        }
        self.work.charge(work).map_err(|limit| {
            self.progress.status = RecoveryStatus::Stopped(StopReason::ResourceLimit(limit));
            AccuracyFailure::ResourceLimit(limit)
        })
    }

    /// Supply a newly certified radius and recompute its comparison with the fixed budget.
    ///
    /// # Errors
    /// Refuses a non-running state. Tolerance arithmetic failure stops recovery,
    /// retains the supplied radius and returns the scalar cause.
    pub fn complete(
        &mut self,
        upper: Nonnegative<P>,
        context: &ArithmeticContext,
    ) -> Result<RecoveryEvidence<P>, AccuracyFailure> {
        if self.progress.status != RecoveryStatus::Running {
            return Err(AccuracyFailure::InvalidRecoveryState);
        }
        self.record_bound(upper);
        let assessment = assess_budget(self.budget.as_ref(), upper, context).map_err(|cause| {
            self.progress.status = RecoveryStatus::Stopped(StopReason::Arithmetic(cause));
            AccuracyFailure::Arithmetic(cause)
        })?;
        self.progress.assessment = Some(assessment);
        self.progress.status = self.after_comparison(assessment.decision());
        Ok(self.evidence())
    }

    /// End a running attempt after an arithmetic failure, retaining earlier evidence.
    ///
    /// # Errors
    /// Refuses a non-running state without changing it.
    pub fn fail(
        &mut self,
        cause: ArithmeticFailure,
    ) -> Result<RecoveryEvidence<P>, AccuracyFailure> {
        if self.progress.status != RecoveryStatus::Running {
            return Err(AccuracyFailure::InvalidRecoveryState);
        }
        self.progress.status = RecoveryStatus::Stopped(StopReason::Arithmetic(cause));
        Ok(self.evidence())
    }

    /// Retain the new radius and count attempts without a strict best improvement.
    fn record_bound(&mut self, upper: Nonnegative<P>) {
        self.progress.last_upper = Some(upper);
        self.progress.assessment = None;
        if self.progress.best.is_none_or(|best| upper < best) {
            self.progress.best = Some(upper);
            self.progress.stagnant = 0;
        } else {
            self.progress.stagnant = self.progress.stagnant.saturating_add(1);
        }
    }

    /// Budget success takes priority, then stagnation, then the refinement limit.
    const fn after_comparison(&self, decision: BudgetDecision) -> RecoveryStatus {
        let reason = match decision {
            BudgetDecision::Met => StopReason::BudgetMet,
            BudgetDecision::NotRequested => StopReason::NotRequested,
            BudgetDecision::Exceeded | BudgetDecision::Unresolved => {
                if self.progress.stagnant >= self.policy.stagnation_window() {
                    StopReason::Stagnation
                } else if self.progress.refinements >= self.policy.max_refinements() {
                    StopReason::RefinementLimit
                } else {
                    return RecoveryStatus::Ready;
                }
            }
        };
        RecoveryStatus::Stopped(reason)
    }
}

impl Recovery<f32> {
    /// Whether the current stop permits a fresh binary64 execution.
    #[must_use]
    pub fn can_retry_f64(&self) -> bool {
        self.policy.promotion() == Promotion::F32ToF64
            && matches!(
                self.progress.status,
                RecoveryStatus::Stopped(
                    StopReason::RefinementLimit
                        | StopReason::Stagnation
                        | StopReason::Arithmetic(
                            ArithmeticFailure::NonFinite | ArithmeticFailure::Overflow
                        )
                )
            )
    }

    /// Start an explicitly permitted fresh binary64 execution, returning the old evidence.
    ///
    /// Cumulative counters and all budget coefficient widths are preserved.
    /// New radii and stagnation history start empty. The caller must convert its
    /// inputs explicitly, retain their prior errors and recompute every certificate.
    /// No binary64-to-wider retry exists in this crate.
    ///
    /// # Errors
    /// Refuses an active, successful, unrequested, resource-stopped or unauthorized run.
    pub fn retry_f64(self) -> Result<(Recovery<f64>, RecoveryEvidence<f32>), AccuracyFailure> {
        if !self.can_retry_f64() {
            return Err(AccuracyFailure::InvalidRecoveryState);
        }
        let evidence = self.evidence();
        let promoted = Recovery::new(self.policy, self.budget.map(Budget::widen), self.work);
        Ok((promoted, evidence))
    }
}
