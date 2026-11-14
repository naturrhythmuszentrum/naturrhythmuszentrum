// SPDX-License-Identifier: AGPL-3.0-only
//! Synchronous explicit collection with ordered outcomes and outer admission.

mod convert;
mod evidence;
mod one;

use crate::report::{
    CaseOutcome, CaseRecord, Execution, ObjectFormat, PhaseDurations, Reason, RunPayload, RunReport,
};
use crate::{CancellationToken, ExecutionBounds, RequestCause, RequestError, RunRequest};
use nrz_matrix::sampling::{InstantClock, SamplingClock};
use std::time::Duration;

/// Caller declarations and cancellation; captured build facts are never caller replacements.
#[derive(Clone, Debug)]
pub struct RunContext {
    /// Explicit host execution assumptions, absent when unsupported.
    pub bounds: Option<ExecutionBounds>,
    /// Caller-owned cancellation token with declared bounded read cost.
    pub cancellation: CancellationToken,
    /// Contributor-assigned opaque public machine alias.
    pub machine_alias: String,
    /// Explicit invocation arguments; no process arguments are read by the library.
    pub command: Vec<String>,
    /// Chosen raw content object format.
    pub object_format: ObjectFormat,
}

/// Execute only the explicit ordered selections, without writing or printing.
/// # Errors
/// Rejects invalid requests and inability to retain all failure outcomes before work starts.
pub fn run(request: &RunRequest, context: &RunContext) -> Result<RunReport, RequestError> {
    let clock = InstantClock::new();
    run_with_clock(request, context, &clock, crate::capture::timestamp)
}

/// Internal injected-clock entry keeps deterministic tests separate from live observations.
fn run_with_clock(
    request: &RunRequest,
    context: &RunContext,
    clock: &dyn SamplingClock,
    timestamp: fn() -> Result<String, &'static str>,
) -> Result<RunReport, RequestError> {
    let started = clock.now();
    request.validate(context.bounds)?;
    let mut ownership = crate::memory::Budget::new(request.resources.report_bytes);
    let owned_request =
        crate::memory::clone_request(request, &mut ownership).map_err(|_| capacity())?;
    let mut cases = reserve_outcomes(request, &mut ownership)?;
    let validated = clock.now();
    let evidence::Captured {
        envelope,
        build,
        environment,
        publication,
        timestamp,
        command,
    } = evidence::capture(
        request,
        context,
        &owned_request,
        &mut ownership,
        cases.capacity(),
        nanos(clock.now().saturating_sub(started))?,
        timestamp,
    )?;
    let end = started
        .checked_add(Duration::from_nanos(request.resources.budget_ns))
        .ok_or_else(capacity)?;
    let prepared = clock.now();
    let mut stop_reason = collect_cases(request, context, clock, started, end, &mut cases);
    let finalization_started = clock.now();
    let phases = nanos(validated.saturating_sub(started))
        .and_then(|validation| {
            phase_totals(
                &cases,
                validation,
                nanos(prepared.saturating_sub(validated))?,
            )
        })
        .ok();
    let elapsed_ns = nanos(clock.now().saturating_sub(started)).ok();
    if phases.is_none() || elapsed_ns.is_none() {
        stop_reason.get_or_insert(Reason::Clock);
    }
    let complete = stop_reason.is_none()
        && cases
            .iter()
            .all(|case| matches!(case.outcome, CaseOutcome::Complete(_)));
    let report = RunReport {
        namespace: envelope.namespace,
        version: [1, 1, 0],
        owner: envelope.owner,
        license: envelope.license,
        sources: envelope.sources,
        labels: envelope.labels,
        payload: RunPayload {
            identity: envelope.identity,
            build,
            environment,
            publication,
            request: owned_request,
            execution: Execution {
                timestamp,
                command,
                elapsed_ns,
                overshoot_ns: elapsed_ns
                    .map(|elapsed| elapsed.saturating_sub(request.resources.budget_ns)),
                bounds: context.bounds,
                phases,
                complete,
                stop_reason,
            },
            cases,
        },
    };
    Ok(finish_report(
        report,
        clock,
        started,
        finalization_started,
        phases,
    ))
}

/// Final clock/ledger failures annotate the owned report rather than discarding it.
fn finish_report(
    mut report: RunReport,
    clock: &dyn SamplingClock,
    started: Duration,
    finalization_started: Duration,
    phases: Option<PhaseDurations>,
) -> RunReport {
    let budget = report.payload.request.resources.budget_ns;
    let finished = clock.now();
    let elapsed = nanos(finished.saturating_sub(started)).ok();
    let ledger = elapsed.zip(phases).and_then(|(elapsed, phases)| {
        let tail = nanos(finished.saturating_sub(finalization_started)).ok()?;
        finalize_phases(phases, elapsed, tail).ok()
    });
    let execution = &mut report.payload.execution;
    let elapsed = ledger.and(elapsed);
    execution.elapsed_ns = elapsed;
    execution.overshoot_ns = elapsed.map(|value| value.saturating_sub(budget));
    execution.phases = ledger;
    if elapsed.is_none() || ledger.is_none() {
        execution.stop_reason = Some(Reason::Clock);
        execution.complete = false;
    }
    report
}

/// Add explicitly observed report finalization and retain the unclassified wall residual.
fn finalize_phases(
    mut phases: PhaseDurations,
    elapsed: u64,
    report_tail: u64,
) -> Result<PhaseDurations, RequestError> {
    phases.finalization_ns = phases
        .finalization_ns
        .checked_add(report_tail)
        .ok_or_else(capacity)?;
    let known = [
        phases.validation_ns,
        phases.preparation_ns,
        phases.pilot_ns,
        phases.warm_up_ns,
        phases.measurement_ns,
        phases.reset_ns,
        phases.finalization_ns,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .ok_or_else(capacity)?;
    phases.harness_ns = elapsed.checked_sub(known).ok_or_else(capacity)?;
    Ok(phases)
}

/// Reserve and initialize every failure outcome before capture or workload preparation.
fn reserve_outcomes(
    request: &RunRequest,
    budget: &mut crate::memory::Budget,
) -> Result<Vec<CaseRecord>, RequestError> {
    let count = request
        .cases
        .len()
        .checked_mul(request.seeds.len())
        .ok_or_else(capacity)?;
    let mut cases = budget.reserve(count).map_err(|_| capacity())?;
    for (repetition, seed) in request.seeds.iter().enumerate() {
        for (index, case) in request.cases.iter().enumerate() {
            let record = CaseRecord {
                case_index: u64::try_from(index).map_err(|_| capacity())?,
                repetition: u64::try_from(repetition).map_err(|_| capacity())?,
                seed: *seed,
                effective_offset: u64::try_from(seed.offset()).map_err(|_| capacity())?,
                accepted_length: None,
                arithmetic_inputs: None,
                packing: (case.dependency == crate::Dependency::BackendKernel)
                    .then(|| crate::recipe::PackingEvidence::from_case_owned(case, budget))
                    .transpose()
                    .map_err(|_| capacity())?,
                lc_ledger: (case.dependency == crate::Dependency::PreparedTraversal)
                    .then(|| crate::recipe::LcLedger::from_case_owned(case, budget))
                    .transpose()
                    .map_err(|_| capacity())?,
                load_before: None,
                load_after: None,
                outcome: CaseOutcome::NotStarted(Reason::Capacity),
            };
            cases.push(record);
        }
    }
    Ok(cases)
}

/// Aggregate fixed case phases and the run's admission/capture preparation.
fn phase_totals(
    cases: &[CaseRecord],
    validation_ns: u64,
    preparation_ns: u64,
) -> Result<PhaseDurations, RequestError> {
    let mut total = PhaseDurations {
        validation_ns,
        preparation_ns,
        ..PhaseDurations::default()
    };
    for record in cases {
        let phases = match &record.outcome {
            CaseOutcome::Complete(measurement) => measurement.observations.phases,
            CaseOutcome::Incomplete(partial) => partial.observations.phases,
            CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => continue,
        };
        total.validation_ns = total
            .validation_ns
            .checked_add(phases.validation_ns)
            .ok_or_else(capacity)?;
        total.preparation_ns = total
            .preparation_ns
            .checked_add(phases.preparation_ns)
            .ok_or_else(capacity)?;
        total.pilot_ns = total
            .pilot_ns
            .checked_add(phases.pilot_ns)
            .ok_or_else(capacity)?;
        total.warm_up_ns = total
            .warm_up_ns
            .checked_add(phases.warm_up_ns)
            .ok_or_else(capacity)?;
        total.measurement_ns = total
            .measurement_ns
            .checked_add(phases.measurement_ns)
            .ok_or_else(capacity)?;
        total.reset_ns = total
            .reset_ns
            .checked_add(phases.reset_ns)
            .ok_or_else(capacity)?;
        total.finalization_ns = total
            .finalization_ns
            .checked_add(phases.finalization_ns)
            .ok_or_else(capacity)?;
    }
    Ok(total)
}

/// Retain the explicit request order while carrying the aggregate attempt budget.
fn collect_cases(
    request: &RunRequest,
    context: &RunContext,
    clock: &dyn SamplingClock,
    started: Duration,
    end: Duration,
    cases: &mut [CaseRecord],
) -> Option<Reason> {
    let mut stopped = None;
    let mut slots = cases.iter_mut();
    let mut remaining_attempts = request.resources.attempts;
    for seed in &request.seeds {
        for case in &request.cases {
            let Some(slot) = slots.next() else {
                return Some(Reason::Capacity);
            };
            if context.cancellation.is_cancelled() {
                stopped.get_or_insert(Reason::Cancelled);
            }
            if clock.now() >= end {
                stopped.get_or_insert(Reason::Deadline);
            }
            if remaining_attempts == 0 {
                stopped.get_or_insert(Reason::Capacity);
            }
            let before = if stopped.is_none() {
                snapshot(request, case, clock, started, &mut stopped)
            } else {
                None
            };
            let (outcome, arithmetic_inputs) = stopped.map_or_else(
                || {
                    one::collect(
                        request,
                        case,
                        *seed,
                        context,
                        clock,
                        end,
                        remaining_attempts,
                    )
                },
                |reason| (CaseOutcome::NotStarted(reason), None),
            );
            let after = if before.is_some() {
                snapshot(request, case, clock, started, &mut stopped)
            } else {
                None
            };
            let outcome = stopped_outcome(
                outcome,
                if context.cancellation.is_cancelled() {
                    Some(Reason::Cancelled)
                } else if clock.now() >= end {
                    Some(Reason::Deadline)
                } else {
                    None
                },
            );
            if let CaseOutcome::Incomplete(partial) = &outcome
                && matches!(
                    partial.reason,
                    Reason::Worker | Reason::Clock | Reason::Capacity
                )
            {
                stopped.get_or_insert(partial.reason);
            }
            let observations = match &outcome {
                CaseOutcome::Complete(measurement) => Some(&measurement.observations),
                CaseOutcome::Incomplete(partial) => Some(&partial.observations),
                CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => None,
            };
            let attempts = observations.map_or(0, |value| value.counts.attempted);
            let accepted_length = observations.and_then(|value| value.pilot.accepted_length);
            let has_observations = observations.is_some();
            remaining_attempts = debit_attempts(remaining_attempts, attempts, &mut stopped);
            slot.accepted_length = accepted_length;
            slot.arithmetic_inputs = if has_observations {
                arithmetic_inputs
            } else {
                None
            };
            slot.load_before = before;
            slot.load_after = after;
            slot.outcome = outcome;
        }
    }
    stopped
}

/// An inconsistent attempt ledger stops further dispatch without discarding outcomes.
fn debit_attempts(remaining: u64, attempts: u64, stopped: &mut Option<Reason>) -> u64 {
    remaining.checked_sub(attempts).unwrap_or_else(|| {
        stopped.get_or_insert(Reason::Capacity);
        0
    })
}

/// Snapshot overflow stops the run while retaining every case already collected.
fn snapshot(
    request: &RunRequest,
    case: &crate::CaseRequest,
    clock: &dyn SamplingClock,
    started: Duration,
    stopped: &mut Option<Reason>,
) -> Option<crate::telemetry::LoadSnapshot> {
    nanos(clock.now().saturating_sub(started)).map_or_else(
        |_| {
            stopped.get_or_insert(Reason::Clock);
            None
        },
        |offset| {
            Some(crate::telemetry::capture(
                offset,
                request.resources.scratch_bytes,
                case,
            ))
        },
    )
}

/// A cancellation observed during finalization invalidates the complete classification.
fn stopped_outcome(outcome: CaseOutcome, stopped: Option<Reason>) -> CaseOutcome {
    let Some(reason) = stopped else {
        return outcome;
    };
    match outcome {
        CaseOutcome::Complete(measurement) => {
            let mut observations = measurement.observations;
            observations.counts.resamples = 0;
            CaseOutcome::Incomplete(Box::new(crate::PartialMeasurement {
                reason,
                observations,
            }))
        }
        other => other,
    }
}

/// Request-level failure before admission.
const fn capacity() -> RequestError {
    RequestError {
        field: "report",
        cause: RequestCause::Capacity,
    }
}

/// Preserve integer durations without clipping or rounding.
fn nanos(duration: Duration) -> Result<u64, RequestError> {
    u64::try_from(duration.as_nanos())
        .ok()
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or_else(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalization_does_not_absorb_unclassified_harness_time() -> Result<(), RequestError> {
        let phases = PhaseDurations {
            validation_ns: 5,
            preparation_ns: 10,
            pilot_ns: 20,
            warm_up_ns: 10,
            measurement_ns: 20,
            reset_ns: 5,
            finalization_ns: 3,
            harness_ns: 0,
        };
        let observed = finalize_phases(phases, 100, 7)?;
        assert_eq!(observed.finalization_ns, 10);
        assert_eq!(observed.harness_ns, 20);
        assert!(finalize_phases(phases, 79, 7).is_err());
        Ok(())
    }

    #[test]
    fn request_and_failure_records_cannot_reuse_the_same_allowance()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = crate::request::tests::request();
        let mut budget = crate::memory::Budget::new(request.resources.report_bytes);
        let owned = crate::memory::clone_request(&request, &mut budget)?;
        let used = request.resources.report_bytes - budget.remaining();
        let records = u64::try_from(size_of::<CaseRecord>())?;
        drop(owned);
        let mut shared = crate::memory::Budget::new(used + records - 1);
        let _owned = crate::memory::clone_request(&request, &mut shared)?;
        assert_eq!(reserve_outcomes(&request, &mut shared), Err(capacity()));
        Ok(())
    }

    #[test]
    fn failure_records_exist_in_requested_order_before_work() -> Result<(), RequestError> {
        let mut request = crate::request::tests::request();
        request.cases.push(request.cases[0].clone());
        request.seeds.push(crate::Seed(2));
        let records = reserve_outcomes(
            &request,
            &mut crate::memory::Budget::new(request.resources.report_bytes),
        )?;
        assert_eq!(records.len(), 4);
        for (record, (case, repetition, seed)) in records.iter().zip([
            (0, 0, crate::Seed(1)),
            (1, 0, crate::Seed(1)),
            (0, 1, crate::Seed(2)),
            (1, 1, crate::Seed(2)),
        ]) {
            assert_eq!(record.case_index, case);
            assert_eq!(record.repetition, repetition);
            assert_eq!(record.seed, seed);
            assert_eq!(record.outcome, CaseOutcome::NotStarted(Reason::Capacity));
            assert_eq!(record.load_before, None);
            assert_eq!(record.load_after, None);
        }
        request.resources.report_bytes = 1;
        assert_eq!(
            reserve_outcomes(
                &request,
                &mut crate::memory::Budget::new(request.resources.report_bytes)
            ),
            Err(capacity())
        );
        Ok(())
    }
    /// Clock fault after the first case's pre-snapshot, without live timing assertions.
    struct OverflowClock(std::cell::Cell<u64>);
    impl SamplingClock for OverflowClock {
        fn now(&self) -> Duration {
            let call = self.0.get() + 1;
            self.0.set(call);
            Duration::from_nanos(if call >= 7 {
                crate::decimal::MAX_INTEGER + 1000
            } else {
                call
            })
        }
    }

    #[test]
    fn post_admission_clock_failure_retains_ordered_outcomes() -> Result<(), RequestError> {
        let mut request = crate::request::tests::request();
        request.cases.push(request.cases[0].clone());
        request.resources.cases = 2;
        let context = RunContext {
            bounds: None,
            cancellation: CancellationToken::default(),
            machine_alias: "clock-fixture".to_owned(),
            command: Vec::new(),
            object_format: ObjectFormat::Sha1,
        };
        let report = run_with_clock(
            &request,
            &context,
            &OverflowClock(std::cell::Cell::new(0)),
            || Ok("2000-01-01T00:00:00Z".to_owned()),
        )?;
        assert_eq!(report.payload.cases.len(), 2);
        assert_eq!(
            report.payload.cases[0].outcome,
            CaseOutcome::Unsupported(Reason::MissingBounds)
        );
        assert_eq!(
            report.payload.cases[1].outcome,
            CaseOutcome::NotStarted(Reason::Clock)
        );
        assert_eq!(report.payload.execution.stop_reason, Some(Reason::Clock));
        assert_eq!(report.payload.execution.elapsed_ns, None);
        assert_eq!(report.payload.execution.phases, None);
        assert!(!report.payload.execution.complete);
        Ok(())
    }

    #[test]
    fn attempt_underflow_records_a_stop_instead_of_returning_request_error() {
        let mut stopped = None;
        assert_eq!(debit_attempts(4, 3, &mut stopped), 1);
        assert_eq!(stopped, None);
        assert_eq!(debit_attempts(1, 2, &mut stopped), 0);
        assert_eq!(stopped, Some(Reason::Capacity));
        stopped = Some(Reason::Worker);
        assert_eq!(debit_attempts(0, 1, &mut stopped), 0);
        assert_eq!(stopped, Some(Reason::Worker));
    }
}
