// SPDX-License-Identifier: AGPL-3.0-only
//! Phase-by-phase deterministic cancellation and expiry; no live timing oracle.

use super::*;
use nrz_matrix::sampling::{AnalysisPhase, SamplingPhase};
use std::cell::Cell;

/// A phase-selected stop advances only the injected clock or cancellation token.
struct Clock {
    /// Monotonic synthetic nanosecond counter.
    now: Cell<u64>,
    /// Phase selected for the injected stop.
    target: SamplingPhase,
    /// Whether the selected checkpoint was observed.
    reached: Cell<bool>,
    /// Choose deadline advancement rather than cancellation.
    deadline: bool,
    /// Explicit token shared with the adapter.
    cancellation: crate::CancellationToken,
}
impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        let now = self.now.get();
        self.now.set(now + 1);
        Duration::from_nanos(now)
    }
    fn checkpoint(&self, phase: SamplingPhase) {
        let selected = phase == self.target;
        if selected && !self.reached.get() {
            self.reached.set(true);
            if self.deadline {
                self.now.set(1_000_000_000);
            } else {
                self.cancellation.cancel();
            }
        }
    }
}

/// Deliberately generous synthetic bounds, never a production default.
const fn bounds() -> ExecutionBounds {
    ExecutionBounds {
        uninterrupted_ns: 1000,
        finalization_ns: 1000,
        item_ns: 100,
        cancellation_read_ns: 1,
        sampler_uninterrupted_ns: 100,
        sampler_finalization_ns: 100,
        receipt_ns: 100,
        disposal_ns: 100,
        cleanup_ns: 100,
        report_ns: 100,
        wait_poll_ns: 100,
    }
}

/// Start directly from a synthetic accepted pilot to isolate the sampler phase boundaries.
fn observed(
    target: SamplingPhase,
    deadline: bool,
) -> Result<(CaseOutcome, bool), Box<dyn std::error::Error>> {
    let mut request = crate::request::tests::request();
    request.sampling.warm_up_ns = 1;
    request.sampling.measurement_ns = 1;
    let cancellation = crate::CancellationToken::default();
    let context = RunContext {
        bounds: Some(bounds()),
        cancellation: cancellation.clone(),
        machine_alias: "synthetic".to_owned(),
        command: Vec::new(),
        object_format: crate::report::ObjectFormat::Sha1,
    };
    let clock = Clock {
        now: Cell::new(0),
        target,
        reached: Cell::new(false),
        deadline,
        cancellation,
    };
    let case = &request.cases[0];
    let runner = Runner::prepare(
        case,
        Seed(1),
        &request,
        context.cancellation.clone(),
        bounds(),
    )?;
    let mut pilot = pilot::stopped(PilotStop::Complete);
    pilot.accepted_length = Some(1);
    let mut observations = pilot_observations(pilot);
    observations.worker_layouts = runner.layouts()?;
    let adapter = Adapter {
        runner,
        admission: Admission {
            cancellation: &context.cancellation,
            clock: &clock,
            end: Duration::from_nanos(1_000_000_000),
        },
        bounds: bounds(),
        reset_ns: 0,
    };
    let outcome = sample(
        SamplingCase {
            request: &request,
            case,
            seed: Seed(1),
            context: &context,
            clock: &clock,
            remaining_attempts: request.resources.attempts,
        },
        adapter,
        observations,
        1,
    );
    Ok((outcome, clock.reached.get()))
}

#[test]
fn each_sampler_phase_preserves_partial_evidence_and_measured_finalization()
-> Result<(), Box<dyn std::error::Error>> {
    for target in [
        SamplingPhase::Validation,
        SamplingPhase::Allocation,
        SamplingPhase::Reset,
        SamplingPhase::Snapshot,
        SamplingPhase::Operation,
        SamplingPhase::Receipt,
        SamplingPhase::Disposal,
        SamplingPhase::Cleanup,
        SamplingPhase::Schedule,
        SamplingPhase::Analysis(AnalysisPhase::Validation),
        SamplingPhase::Analysis(AnalysisPhase::Allocation),
        SamplingPhase::Analysis(AnalysisPhase::Statistics),
        SamplingPhase::Analysis(AnalysisPhase::Resampling),
        SamplingPhase::Analysis(AnalysisPhase::Sorting),
        SamplingPhase::Analysis(AnalysisPhase::Report),
        SamplingPhase::Report,
    ] {
        for deadline in [false, true] {
            let (outcome, reached) = observed(target, deadline)?;
            assert!(reached, "{target:?}");
            let CaseOutcome::Incomplete(partial) = outcome else {
                return Err(format!("complete after {target:?}").into());
            };
            assert_eq!(partial.observations.counts.resamples, 0);
            assert!(partial.observations.phases.finalization_ns > 0);
            let counts = partial.observations.counts;
            assert_eq!(counts.attempted, counts.measured + counts.discarded);
        }
    }
    Ok(())
}

#[test]
fn reset_and_finalization_use_distinct_clock_intervals() -> Result<(), Box<dyn std::error::Error>> {
    let (outcome, _) = observed(SamplingPhase::Report, false)?;
    let CaseOutcome::Incomplete(partial) = outcome else {
        return Err("expected cancellation".into());
    };
    assert!(partial.observations.phases.reset_ns > 0);
    assert!(partial.observations.phases.finalization_ns > 0);
    Ok(())
}

/// No live time participates in capability refusal tests.
struct IdleClock;
impl SamplingClock for IdleClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }
}

/// Capability refusal is an owned public case outcome before preparation.
fn assert_refusal(case: &CaseRequest, reason: Reason) -> Result<(), crate::RequestError> {
    let mut request = crate::request::tests::request();
    request.cases[0] = case.clone();
    request.resources.workers = case.workers;
    let context = RunContext {
        bounds: Some(bounds()),
        cancellation: crate::CancellationToken::default(),
        machine_alias: "capability-fixture".to_owned(),
        command: Vec::new(),
        object_format: crate::report::ObjectFormat::Sha1,
    };
    let report = crate::collect::run_with_clock(&request, &context, &IdleClock, || {
        Ok("2000-01-01T00:00:00Z".to_owned())
    })?;
    assert_eq!(
        report.payload.cases[0].outcome,
        CaseOutcome::Unsupported(reason)
    );
    assert_eq!(report.payload.cases[0].arithmetic_inputs, None);
    Ok(())
}

#[test]
fn capability_refusals_precede_preparation() -> Result<(), crate::RequestError> {
    let mut case = crate::request::tests::request().cases.remove(0);
    case.placement = Placement::ExactWorkerBinding;
    assert_refusal(&case, Reason::Affinity)?;
    case.placement = Placement::Unbound;
    case.width = Width::BackendSelectedBinary64;
    assert_refusal(&case, Reason::Width)?;
    case.operation = Operation::Gemm;
    case.dependency = Dependency::BackendKernel;
    case.shape = vec![2, 2, 2];
    case.workers = 2;
    assert_refusal(&case, Reason::ParallelMatrix)?;
    case.workers = 1;
    assert_refusal(&case, Reason::BackendPacking)?;
    case.shape = vec![1, 4096, 4096];
    assert_eq!(unsupported(&case, Some(bounds())), None);
    Ok(())
}
