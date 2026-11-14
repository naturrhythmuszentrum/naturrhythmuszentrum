// SPDX-License-Identifier: AGPL-3.0-only
//! One prepared case, its pilot and unchanged host sampling call.

use super::{RunContext, convert};
use crate::report::{
    CaseOutcome, Counts, Observations, PartialMeasurement, PhaseDurations, Reason,
};
use crate::{
    CaseRequest, Dependency, ExecutionBounds, Operation, Placement, RunRequest, Seed, Width,
};
use crate::{
    adapter::{Adapter, Admission},
    kernels::counts,
    pilot::{self, PilotPolicy, PilotStop},
    workers::Runner,
};
use nrz_matrix::sampling::{
    self, AnalysisCaps, AnalysisSettings, Representative, SamplerBounds, SamplingClock,
    SamplingIdentity, SamplingSettings,
};
use std::time::Duration;

/// Return capability refusals before preparing any kernel state.
fn unsupported(case: &CaseRequest, bounds: Option<ExecutionBounds>) -> Option<Reason> {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return Some(Reason::Platform);
    }
    if bounds.is_none() {
        return Some(Reason::MissingBounds);
    }
    if case.placement == Placement::ExactWorkerBinding {
        return Some(Reason::Affinity);
    }
    if case.dependency == Dependency::BackendKernel {
        if case.workers != 1 {
            return Some(Reason::ParallelMatrix);
        }
        if case.width != Width::BackendSelectedBinary64 {
            return Some(Reason::Width);
        }
        if !crate::kernels::packing_supported(case) {
            return Some(Reason::BackendPacking);
        }
        return None;
    }
    if case.operation == Operation::Read
        && matches!(case.width, Width::Scalar | Width::BackendSelectedBinary64)
    {
        return Some(Reason::Width);
    }
    let plain_stream = matches!(
        case.operation,
        Operation::Write | Operation::Copy | Operation::FusedStream
    );
    if plain_stream && case.width != Width::Scalar {
        return Some(Reason::Width);
    }
    if case.dependency == Dependency::PreparedTraversal {
        if case.width != Width::Scalar {
            return Some(Reason::Width);
        }
        if case.workers != 1 {
            return Some(Reason::Worker);
        }
    }
    if case.width == Width::BackendSelectedBinary64 {
        return Some(Reason::Width);
    }
    match case.operation {
        Operation::Add
        | Operation::Multiply
        | Operation::FmaAddend
        | Operation::FmaMultiplicand
        | Operation::MultiplyAdd
        | Operation::Divide
        | Operation::SquareRoot
        | Operation::Read
        | Operation::Write
        | Operation::Copy
        | Operation::FusedStream
        | Operation::ForwardFused
        | Operation::ForwardSeparate
        | Operation::Inductance
        | Operation::Capacitance
        | Operation::InverseTime => None,
        _ => Some(Reason::Preparation),
    }
}

/// Convert pilot completion/refusal while preserving its complete observation prefix.
fn pilot_observations(pilot: pilot::PilotReport) -> Observations {
    let attempts = pilot.attempted;
    Observations {
        worker_layouts: Vec::new(),
        pilot,
        item_ns: Vec::new(),
        exceptions: Vec::new(),
        rows: Vec::new(),
        linear_step: None,
        sampler_stop: crate::sampling_evidence::SamplingStop::NotStarted,
        counts: Counts {
            attempted: attempts,
            discarded: attempts,
            pilot: attempts,
            ..Counts::default()
        },
        phases: PhaseDurations::default(),
    }
}

/// A pilot stop cannot produce complete estimates.
fn partial(observations: Observations, reason: Reason) -> CaseOutcome {
    CaseOutcome::Incomplete(Box::new(PartialMeasurement {
        reason,
        observations,
    }))
}

/// Prepare exact coefficient evidence before any kernel or worker work is admitted.
pub(super) fn collect(
    request: &RunRequest,
    case: &CaseRequest,
    seed: Seed,
    context: &RunContext,
    clock: &dyn SamplingClock,
    outer_end: Duration,
    remaining_attempts: u64,
) -> (CaseOutcome, Option<crate::recipe::ArithmeticInputs>) {
    if let Some(reason) = unsupported(case, context.bounds) {
        return (CaseOutcome::Unsupported(reason), None);
    }
    let started = clock.now();
    let Ok(inputs) = crate::kernels::arithmetic_inputs(case, seed) else {
        return (
            partial(
                pilot_observations(pilot::stopped(PilotStop::Capacity)),
                Reason::Capacity,
            ),
            None,
        );
    };
    let elapsed = clock
        .now()
        .checked_sub(started)
        .and_then(|time| super::nanos(time).ok());
    let mut outcome = collect_outcome(
        request,
        case,
        seed,
        context,
        clock,
        outer_end,
        remaining_attempts,
    );
    let phases = match &mut outcome {
        CaseOutcome::Complete(value) => Some(&mut value.observations.phases),
        CaseOutcome::Incomplete(value) => Some(&mut value.observations.phases),
        CaseOutcome::NotStarted(_) | CaseOutcome::Unsupported(_) => None,
    };
    if let Some(phases) = phases {
        if let Some(preparation) = elapsed.and_then(|time| phases.preparation_ns.checked_add(time))
        {
            phases.preparation_ns = preparation;
        } else {
            outcome = super::stopped_outcome(outcome, Some(Reason::Clock));
        }
    }
    (outcome, inputs)
}

/// Retain the case-local residual independently of observed preparation/reset/finalization.
fn collect_outcome(
    request: &RunRequest,
    case: &CaseRequest,
    seed: Seed,
    context: &RunContext,
    clock: &dyn SamplingClock,
    outer_end: Duration,
    remaining_attempts: u64,
) -> CaseOutcome {
    let started = clock.now();
    let mut outcome = collect_inner(
        request,
        case,
        seed,
        context,
        clock,
        outer_end,
        remaining_attempts,
    );
    let phases = match &mut outcome {
        CaseOutcome::Complete(value) => &mut value.observations.phases,
        CaseOutcome::Incomplete(value) => &mut value.observations.phases,
        CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => return outcome,
    };
    let elapsed = super::nanos(clock.now().saturating_sub(started));
    if let Ok(Ok(ledger)) = elapsed.map(|elapsed| super::finalize_phases(*phases, elapsed, 0)) {
        *phases = ledger;
    } else {
        return super::stopped_outcome(outcome, Some(Reason::Clock));
    }
    outcome
}

/// Execute one case under the minimum outer/per-case admission deadline.
fn collect_inner(
    request: &RunRequest,
    case: &CaseRequest,
    seed: Seed,
    context: &RunContext,
    clock: &dyn SamplingClock,
    outer_end: Duration,
    remaining_attempts: u64,
) -> CaseOutcome {
    if let Some(reason) = unsupported(case, context.bounds) {
        return CaseOutcome::Unsupported(reason);
    }
    let Some(bounds) = context.bounds else {
        return CaseOutcome::Unsupported(Reason::MissingBounds);
    };
    let started = clock.now();
    let end = started
        .saturating_add(Duration::from_nanos(case.budget_ns))
        .min(outer_end);
    if context.cancellation.is_cancelled() || started >= end {
        let reason = if context.cancellation.is_cancelled() {
            Reason::Cancelled
        } else {
            Reason::Deadline
        };
        return partial(
            pilot_observations(pilot::stopped(PilotStop::Stopped)),
            reason,
        );
    }
    let runner = match Runner::prepare(case, seed, request, context.cancellation.clone(), bounds) {
        Ok(runner) => runner,
        Err(error) => {
            let (stop, reason) = match error {
                crate::recipe::RecipeError::Worker => (PilotStop::Worker, Reason::Worker),
                crate::recipe::RecipeError::Cancelled => (PilotStop::Stopped, Reason::Cancelled),
                crate::recipe::RecipeError::Allocation | crate::recipe::RecipeError::Overflow => {
                    (PilotStop::Capacity, Reason::Capacity)
                }
                crate::recipe::RecipeError::Domain => (PilotStop::Operation, Reason::Preparation),
            };
            return partial(pilot_observations(pilot::stopped(stop)), reason);
        }
    };
    let Ok(worker_layouts) = runner.layouts() else {
        return partial(
            pilot_observations(pilot::stopped(PilotStop::Capacity)),
            Reason::Capacity,
        );
    };
    let mut adapter = Adapter {
        runner,
        reset_ns: 0,
        admission: Admission {
            cancellation: &context.cancellation,
            clock,
            end,
        },
        bounds,
    };
    let maximum_length = counts(case, 1)
        .ok()
        .map(|counts| counts.floating_operations.unwrap_or(counts.elements))
        .map_or(0, |operations| {
            request.resources.operations_per_item / operations
        });
    let policy = PilotPolicy {
        length: case.item_length,
        maximum_length,
        attempts: remaining_attempts,
        item_bound_ns: bounds.item_ns,
        resolution_ns: None,
        grow: case.operation != Operation::Factor
            || case.scope == crate::Scope::RestorationInclusiveRepeated,
    };
    let prepared = clock.now();
    let pilot = pilot::size(clock, &mut adapter, policy);
    let mut observations = pilot_observations(pilot);
    observations.worker_layouts = worker_layouts;
    let Ok(preparation_ns) = super::nanos(prepared.saturating_sub(started)) else {
        return finalize(partial(observations, Reason::Clock), adapter, clock);
    };
    let Ok(pilot_ns) = super::nanos(clock.now().saturating_sub(prepared)) else {
        return finalize(partial(observations, Reason::Clock), adapter, clock);
    };
    observations.phases.preparation_ns = preparation_ns;
    observations.phases.pilot_ns = pilot_ns.saturating_sub(adapter.reset_ns);
    observations.phases.reset_ns = adapter.reset_ns;
    let length = match accepted(&observations, context) {
        Ok(length) => length,
        Err(reason) => return finalize(partial(observations, reason), adapter, clock),
    };
    sample(
        SamplingCase {
            request,
            case,
            seed,
            context,
            clock,
            remaining_attempts,
        },
        adapter,
        observations,
        length,
    )
}

/// Shared immutable case admission for post-pilot sampling and report conversion.
#[derive(Clone, Copy)]
struct SamplingCase<'a> {
    /// Explicit run policy.
    request: &'a RunRequest,
    /// Selected case identity.
    case: &'a CaseRequest,
    /// Independent repetition seed.
    seed: Seed,
    /// Host declarations and cancellation.
    context: &'a RunContext,
    /// Monotonic run clock.
    clock: &'a dyn SamplingClock,
    /// Aggregate remaining attempts, including this case's pilot.
    remaining_attempts: u64,
}

/// Call the unchanged host sampler with the accepted fixed workload identity.
fn sample(
    input: SamplingCase<'_>,
    mut adapter: Adapter<'_>,
    mut observations: Observations,
    length: u64,
) -> CaseOutcome {
    let SamplingCase {
        request,
        case,
        seed,
        context,
        clock,
        remaining_attempts,
    } = input;
    let end = adapter.admission.end;
    if adapter.admission.stopped() {
        let reason = if context.cancellation.is_cancelled() {
            Reason::Cancelled
        } else {
            Reason::Deadline
        };
        return finalize(partial(observations, reason), adapter, clock);
    }
    let bounds = adapter.bounds;
    let Ok(numerators) = counts(case, length) else {
        return finalize(partial(observations, Reason::Capacity), adapter, clock);
    };
    let remaining = end.saturating_sub(clock.now());
    let Ok(settings) = settings(
        request,
        seed,
        bounds,
        remaining,
        observations.counts.attempted,
        remaining_attempts,
    ) else {
        return finalize(partial(observations, Reason::Capacity), adapter, clock);
    };
    let Ok(shape) = case
        .shape
        .iter()
        .map(|value| usize::try_from(*value))
        .collect::<Result<Vec<_>, _>>()
    else {
        return finalize(partial(observations, Reason::Capacity), adapter, clock);
    };
    let workload = format!(
        "{:?}/{:?}/{:?}",
        case.operation, case.width, case.dependency
    );
    let scope = format!("{:?}", case.scope);
    let representative = Representative {
        item: &length,
        elements_per_item: Some(numerators.elements),
        identity: SamplingIdentity {
            representative: "compute-case",
            shape: &shape,
            operation_revision: "compute-1",
            workload: &workload,
            scope: &scope,
            trace_configuration: "caller-scoped-unknown",
        },
    };
    if let Err(reason) = convert::reserve(
        &mut observations,
        remaining_attempts,
        request.sampling.samples,
    ) {
        return finalize(partial(observations, reason), adapter, clock);
    }
    let sampled = sampling::sample_with_clock(&(), &representative, &mut adapter, settings, clock);
    let finalization_started = clock.now();
    let reset_ns = adapter.reset_ns;
    let stopped = if context.cancellation.is_cancelled() {
        Some(Reason::Cancelled)
    } else if clock.now() >= end {
        Some(Reason::Deadline)
    } else {
        None
    };
    drop(adapter);
    let outcome = match sampled {
        Ok(report) => convert::outcome(&report, observations, numerators, stopped),
        Err(error) => {
            observations.sampler_stop =
                crate::sampling_evidence::SamplingStop::Rejected(error.into());
            partial(observations, Reason::Sampling)
        }
    };
    let outcome = finalization_duration(outcome, reset_ns, finalization_started, clock);
    let stopped = if context.cancellation.is_cancelled() {
        Some(Reason::Cancelled)
    } else if clock.now() >= end {
        Some(Reason::Deadline)
    } else {
        None
    };
    super::stopped_outcome(outcome, stopped)
}

/// Join every worker on early exits and retain the measured mandatory tail.
fn finalize(outcome: CaseOutcome, adapter: Adapter<'_>, clock: &dyn SamplingClock) -> CaseOutcome {
    let started = clock.now();
    let reset_ns = adapter.reset_ns;
    drop(adapter);
    finalization_duration(outcome, reset_ns, started, clock)
}

/// Explicit finalization is distinct from the unclassified harness residual.
fn finalization_duration(
    mut outcome: CaseOutcome,
    reset_ns: u64,
    started: Duration,
    clock: &dyn SamplingClock,
) -> CaseOutcome {
    let phases = match &mut outcome {
        CaseOutcome::Complete(value) => &mut value.observations.phases,
        CaseOutcome::Incomplete(value) => &mut value.observations.phases,
        CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => return outcome,
    };
    phases.reset_ns = reset_ns;
    if let Ok(duration) = super::nanos(clock.now().saturating_sub(started)) {
        phases.finalization_ns = duration;
        outcome
    } else {
        super::stopped_outcome(outcome, Some(Reason::Clock))
    }
}

/// Preserve the observed pilot stop independently of accepted-length presence.
fn accepted(observations: &Observations, context: &RunContext) -> Result<u64, Reason> {
    if observations.pilot.stop == PilotStop::Complete {
        return observations.pilot.accepted_length.ok_or(Reason::Capacity);
    }
    Err(match observations.pilot.stop {
        PilotStop::ItemTooShort => Reason::ItemTooShort,
        PilotStop::ItemExceedsBound => Reason::ItemExceedsBound,
        PilotStop::Stopped => {
            if context.cancellation.is_cancelled() {
                Reason::Cancelled
            } else {
                Reason::Deadline
            }
        }
        PilotStop::NoPositiveTick | PilotStop::Clock => Reason::Clock,
        PilotStop::Operation => Reason::Preparation,
        PilotStop::Worker => Reason::Worker,
        PilotStop::Capacity | PilotStop::Complete => Reason::Capacity,
    })
}

/// Pass explicit statistical policy unchanged, capped by remaining outer admission.
fn settings(
    request: &RunRequest,
    seed: Seed,
    bounds: ExecutionBounds,
    remaining: Duration,
    pilot_attempts: u64,
    remaining_attempts: u64,
) -> Result<SamplingSettings, sampling::AnalysisError> {
    let policy = request.sampling;
    let caps = |report, scratch| -> Result<AnalysisCaps, sampling::AnalysisError> {
        Ok(AnalysisCaps {
            report_bytes: usize::try_from(report)
                .map_err(|_| sampling::AnalysisError::CountRepresentation)?,
            scratch_bytes: usize::try_from(scratch)
                .map_err(|_| sampling::AnalysisError::CountRepresentation)?,
        })
    };
    let analysis = AnalysisSettings::new(
        policy.samples as usize,
        policy.resamples as usize,
        f64::from_bits(policy.confidence.0),
        seed.0,
        caps(policy.analysis_report_bytes, policy.analysis_scratch_bytes)?,
    )?;
    Ok(SamplingSettings {
        analysis,
        warm_up: Duration::from_nanos(policy.warm_up_ns),
        measurement: Duration::from_nanos(policy.measurement_ns),
        wall_budget: remaining,
        max_total_iterations: remaining_attempts.saturating_sub(pilot_attempts),
        max_iterations_per_sample: request.resources.iterations_per_row,
        caps: caps(
            crate::memory::sampler_bytes(
                request,
                remaining_attempts.saturating_sub(pilot_attempts),
            )
            .map_err(|_| sampling::AnalysisError::CountRepresentation)?,
            policy.analysis_scratch_bytes,
        )?,
        bounds: SamplerBounds {
            uninterrupted: Duration::from_nanos(bounds.sampler_uninterrupted_ns),
            finalization: Duration::from_nanos(bounds.sampler_finalization_ns),
        },
    })
}

#[cfg(test)]
mod tests;
