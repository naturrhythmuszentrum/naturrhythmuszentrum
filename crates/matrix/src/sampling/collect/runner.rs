// SPDX-License-Identifier: AGPL-3.0-only
//! Entry validation, whole schedule collection and complete-only analysis.

use super::{
    AdapterBounds, HostOperation, InstantClock, Representative, SamplingClock, SamplingDeadline,
    SamplingError, SamplingPhase, SamplingReport, SamplingSettings, SamplingStop, SchedulePosition,
    execute, policy, schedule,
};
use std::time::Duration;

/// Sample a host's immutable prepared problem using `std::time::Instant`.
///
/// The deadline covers validation, reservations, warm-up, measurement, analysis
/// and mandatory cleanup. Admission is the timestamp immediately before each
/// operation invocation. A late operation still preserves its receipt and releases
/// all private state. Return has the cooperative bound D + U + C + delta; additional
/// scheduling delay delta is not independently measured or bounded by this library.
/// No subscriber, process state, file/console output or background work is created.
///
/// # Errors
/// Returns a pre-execution policy/contract error. After valid admission, all stops
/// are reported together with completed rows and every attempted iteration.
pub fn sample<'a, P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    representative: &Representative<'a, I>,
    adapter: &mut A,
    settings: SamplingSettings,
) -> Result<SamplingReport<'a, A::Work, A::Error>, SamplingError> {
    let clock = InstantClock::new();
    let deadline = SamplingDeadline::at(&clock, settings.wall_budget, Duration::ZERO);
    sample_started(prepared, representative, adapter, settings, &deadline)
}

/// Sample with a bounded monotonic clock seam; statistical semantics are unchanged.
///
/// Aggregate report caps include the collection record, actual capacities of
/// iteration/row vectors, and the entire analysis report allowance (conservatively
/// including its inline record twice). Scratch caps include the analysis scratch
/// allowance. Fixed scalar call frames, allocator metadata and caller-owned
/// clock/input/adapter/context/output storage are excluded. The adapter must bound
/// its own storage. All iteration evidence is reserved before the first reset;
/// finalization cannot need a new sampler allocation.
///
/// # Errors
/// Returns an invalid policy, unsupported contract, capacity or bound-overflow
/// error before any reset, snapshot or operation. Runtime failures are reports.
pub fn sample_with_clock<'a, P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    representative: &Representative<'a, I>,
    adapter: &mut A,
    settings: SamplingSettings,
    clock: &dyn SamplingClock,
) -> Result<SamplingReport<'a, A::Work, A::Error>, SamplingError> {
    let deadline = SamplingDeadline::new(clock, settings.wall_budget);
    sample_started(prepared, representative, adapter, settings, &deadline)
}

/// Both public entries share the same already-started admission and finalization path.
fn sample_started<'a, P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    representative: &Representative<'a, I>,
    adapter: &mut A,
    settings: SamplingSettings,
    deadline: &SamplingDeadline<'_>,
) -> Result<SamplingReport<'a, A::Work, A::Error>, SamplingError> {
    let bounds = adapter.bounds().ok_or(SamplingError::Unsupported)?;
    let admission = policy::validate::<A::Work, A::Error, I>(settings, representative, bounds)?;
    let mut report = empty_report(settings, representative, bounds, admission);
    let _expired = deadline.expired(SamplingPhase::Validation);
    let stop = collect(
        prepared,
        representative,
        adapter,
        deadline,
        admission,
        &mut report,
    );
    report.stop = stop.unwrap_or(SamplingStop::Complete);
    finish(deadline, &mut report);
    Ok(report)
}

/// Construct the admitted fixed header without allocating or invoking host code.
const fn empty_report<'a, W, E, I: ?Sized>(
    settings: SamplingSettings,
    representative: &Representative<'a, I>,
    bounds: AdapterBounds,
    admission: policy::Admission,
) -> SamplingReport<'a, W, E> {
    SamplingReport {
        settings,
        identity: representative.identity,
        elements: representative.elements_per_item,
        adapter_bounds: bounds,
        uninterrupted: admission.uninterrupted,
        finalization: admission.finalization,
        iterations: Vec::new(),
        rows: Vec::new(),
        step: None,
        warm_up_iterations: 0,
        warm_up_elapsed: Duration::ZERO,
        measured: Duration::ZERO,
        total: Duration::ZERO,
        analysis: None,
        stop: SamplingStop::Complete,
    }
}

/// Ordinary collection ends on the first stop, retaining all admitted evidence.
fn collect<P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    representative: &Representative<'_, I>,
    adapter: &mut A,
    deadline: &SamplingDeadline<'_>,
    admission: policy::Admission,
    report: &mut SamplingReport<'_, A::Work, A::Error>,
) -> Option<SamplingStop<A::Error>> {
    if let Some(stop) = execute::deadline_stop(deadline) {
        return Some(stop);
    }
    if let Err(error) = reserve(report, admission, deadline) {
        return Some(SamplingStop::Rejected(error));
    }
    if let Some(stop) = execute::deadline_stop(deadline) {
        return Some(stop);
    }
    let mut group = 1;
    loop {
        for _ in 0..group {
            if let Some(stop) = record_iteration(
                prepared,
                representative.item,
                adapter,
                deadline,
                SchedulePosition::WarmUp(group),
                report,
            ) {
                return Some(stop);
            }
        }
        if report.warm_up_elapsed >= report.settings.warm_up {
            break;
        }
        group = match schedule::next_group(group, report.warm_up_iterations, report.settings) {
            Ok(group) => group,
            Err(error) => return Some(SamplingStop::Rejected(error)),
        };
    }
    let step = match schedule::linear_step(
        report.settings,
        report.warm_up_iterations,
        report.warm_up_elapsed,
    ) {
        Ok(step) => step,
        Err(error) => return Some(SamplingStop::Rejected(error)),
    };
    report.step = Some(step);
    for row in 0..report.settings.analysis.samples() {
        let iterations = u64::from(row + 1).checked_mul(step);
        let Some(iterations) = iterations else {
            return Some(SamplingStop::Rejected(SamplingError::Arithmetic));
        };
        if let Some(stop) = measurement_row(
            prepared,
            representative.item,
            adapter,
            deadline,
            (row, iterations),
            report,
        ) {
            return Some(stop);
        }
    }
    match super::super::analyze(
        &report.rows,
        report.settings.analysis,
        report.elements,
        |phase| deadline.expired(SamplingPhase::Analysis(phase)),
    ) {
        Ok(analysis) => {
            report.analysis = Some(analysis);
            None
        }
        Err(error) => execute::deadline_stop(deadline).or(Some(SamplingStop::Analysis(error))),
    }
}

/// Preserve a completed row even when its last invocation/finalization crosses D.
fn measurement_row<P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    item: &I,
    adapter: &mut A,
    deadline: &SamplingDeadline<'_>,
    (row, iterations): (u32, u64),
    report: &mut SamplingReport<'_, A::Work, A::Error>,
) -> Option<SamplingStop<A::Error>> {
    let before = report.measured;
    let mut completed = 0;
    let mut stop = None;
    for _ in 0..iterations {
        let previous = report.iterations.len();
        stop = record_iteration(
            prepared,
            item,
            adapter,
            deadline,
            SchedulePosition::Measurement(row),
            report,
        );
        if report.iterations.len() > previous
            && report.iterations.last().is_some_and(|record| {
                let success = matches!(record.outcome, super::IterationOutcome::Returned(Ok(())));
                let timed = record.elapsed.is_some_and(|elapsed| !elapsed.is_zero());
                success && timed
            })
        {
            completed += 1;
        }
        if stop.is_some() {
            break;
        }
    }
    if completed == iterations {
        report.rows.push(super::SampleRow {
            iterations,
            elapsed_seconds: report.measured.saturating_sub(before).as_secs_f64(),
        });
    }
    stop
}

/// Check actual aggregate capacities and stop between every fallible reservation.
fn reserve<W, E>(
    report: &mut SamplingReport<'_, W, E>,
    admission: policy::Admission,
    deadline: &SamplingDeadline<'_>,
) -> Result<(), SamplingError> {
    if deadline.expired(SamplingPhase::Allocation) {
        return Ok(());
    }
    let iterations = policy::reserve::<super::IterationRecord<W, E>>(admission.iterations)?;
    let iter_bytes = policy::bytes::<super::IterationRecord<W, E>>(iterations.capacity())?;
    let used = iter_bytes
        .checked_add(admission.fixed_report)
        .ok_or(SamplingError::Capacity)?;
    let needed = used
        .checked_add(policy::bytes::<super::SampleRow>(admission.rows)?)
        .ok_or(SamplingError::Capacity)?;
    if needed > report.settings.caps.report_bytes {
        return Err(SamplingError::Capacity);
    }
    report.iterations = iterations;
    if deadline.expired(SamplingPhase::Allocation) {
        return Ok(());
    }
    let rows = policy::reserve::<super::SampleRow>(admission.rows)?;
    let used = used
        .checked_add(policy::bytes::<super::SampleRow>(rows.capacity())?)
        .ok_or(SamplingError::Capacity)?;
    if used > report.settings.caps.report_bytes {
        return Err(SamplingError::Capacity);
    }
    report.rows = rows;
    let _expired = deadline.expired(SamplingPhase::Allocation);
    Ok(())
}

/// Preserve the complete fixed record even when an operation crosses the deadline.
fn record_iteration<P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    item: &I,
    adapter: &mut A,
    deadline: &SamplingDeadline<'_>,
    position: SchedulePosition,
    report: &mut SamplingReport<'_, A::Work, A::Error>,
) -> Option<SamplingStop<A::Error>> {
    if deadline.expired(SamplingPhase::Schedule) {
        return execute::deadline_stop(deadline);
    }
    if report.iterations.len() == report.iterations.capacity() {
        return Some(SamplingStop::Rejected(SamplingError::IterationCap));
    }
    let (record, mut stop) = execute::iteration(prepared, item, adapter, deadline, position);
    if let Some(elapsed) = record.elapsed {
        if let Some(total) = report.measured.checked_add(elapsed) {
            report.measured = total;
        } else {
            stop = stop.or(Some(SamplingStop::Rejected(SamplingError::Arithmetic)));
        }
        if matches!(position, SchedulePosition::WarmUp(_)) {
            report.warm_up_iterations += 1;
            if let Some(total) = report.warm_up_elapsed.checked_add(elapsed) {
                report.warm_up_elapsed = total;
            } else {
                stop = stop.or(Some(SamplingStop::Rejected(SamplingError::Arithmetic)));
            }
        }
    }
    report.iterations.push(record);
    stop
}

/// Report construction is mandatory; expiry removes any otherwise complete estimate.
fn finish<W, E>(deadline: &SamplingDeadline<'_>, report: &mut SamplingReport<'_, W, E>) {
    let _expired = deadline.expired(SamplingPhase::Report);
    if let Some(stop) = execute::deadline_stop(deadline) {
        if matches!(report.stop, SamplingStop::Complete) {
            report.stop = stop;
        }
        report.analysis = None;
    }
    report.total = deadline.elapsed();
    // The final timestamp can itself observe expiry under the declared clock bound.
    if let Some(stop) = execute::deadline_stop(deadline) {
        if matches!(report.stop, SamplingStop::Complete) {
            report.stop = stop;
        }
        let discarded = report.analysis.take();
        let had_analysis = discarded.is_some();
        drop(discarded);
        if had_analysis {
            // Include deallocation if the final timestamp was the first expiry.
            report.total = deadline.elapsed();
        }
    }
}
