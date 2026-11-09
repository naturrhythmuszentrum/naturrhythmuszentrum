// SPDX-License-Identifier: AGPL-3.0-only
//! One iteration: private reset, adjacent snapshots/timer, mandatory finalization.

use super::{
    HostOperation, IterationOutcome, IterationRecord, ReceiptStatus, SamplingDeadline,
    SamplingPhase, SamplingStop, SchedulePosition,
};

/// Fixed evidence and optional stop from a single attempt.
type IterationResult<W, E> = (IterationRecord<W, E>, Option<SamplingStop<E>>);

/// Retain the first stop while always completing mandatory callbacks.
pub(super) fn deadline_stop<E>(deadline: &SamplingDeadline<'_>) -> Option<SamplingStop<E>> {
    if deadline.reversed() {
        Some(SamplingStop::ClockReversed)
    } else {
        deadline.expiry().map(SamplingStop::Deadline)
    }
}

/// Execute at most one operation; no fallible sampler allocation occurs here.
pub(super) fn iteration<P: ?Sized, I: ?Sized, A: HostOperation<P, I> + ?Sized>(
    prepared: &P,
    item: &I,
    adapter: &mut A,
    deadline: &SamplingDeadline<'_>,
    position: SchedulePosition,
) -> IterationResult<A::Work, A::Error> {
    let mut record = IterationRecord {
        position,
        outcome: IterationOutcome::NotInvoked,
        elapsed: None,
        entry: None,
        receipt: None,
        disposal: None,
        cleanup: None,
    };
    if deadline.expired(SamplingPhase::Reset) {
        return (record, deadline_stop(deadline));
    }
    let mut context = match adapter.reset(prepared, item, deadline) {
        Ok(context) => context,
        Err(error) => {
            record.outcome = IterationOutcome::ResetFailed(error);
            return (
                record,
                Some(SamplingStop::HostError(SamplingPhase::Reset, error)),
            );
        }
    };
    let mut stop = None;
    if !deadline.expired(SamplingPhase::Snapshot) {
        record.entry = adapter.snapshot(&context, deadline);
        // This timestamp IS operation admission. There is no callback between
        // entry Work and timer start, nor between timer stop and receipt reading.
        let start = deadline.stamp(SamplingPhase::Operation);
        if !deadline.is_stopped() {
            let output = std::hint::black_box(adapter.run(
                std::hint::black_box(prepared),
                std::hint::black_box(item),
                &mut context,
                deadline,
            ));
            let end = deadline.stamp(SamplingPhase::Operation);
            let mut receipt = adapter.receipt(&context, &output, deadline);
            let _expired = deadline.expired(SamplingPhase::Receipt);
            if record.entry.is_none() || receipt.exit.is_none() || receipt.operation.is_none() {
                receipt.status = ReceiptStatus::Incomplete;
            }
            let elapsed = end.saturating_sub(start);
            record.elapsed = Some(elapsed);
            record.receipt = Some(receipt);
            record.outcome =
                IterationOutcome::Returned(output.as_ref().map(|_| ()).map_err(|e| *e));
            stop = operation_stop(&record, deadline);
            let _expired = deadline.expired(SamplingPhase::Disposal);
            let disposal = adapter.dispose(output, deadline);
            if let Err(error) = disposal {
                stop = stop.or(Some(SamplingStop::HostError(
                    SamplingPhase::Disposal,
                    error,
                )));
            }
            record.disposal = Some(disposal);
        }
    }
    stop = stop.or_else(|| deadline_stop(deadline));
    let _expired = deadline.expired(SamplingPhase::Cleanup);
    let cleanup = adapter.cleanup(context, deadline);
    if let Err(error) = cleanup {
        stop = stop.or(Some(SamplingStop::HostError(SamplingPhase::Cleanup, error)));
    }
    record.cleanup = Some(cleanup);
    let _expired = deadline.expired(SamplingPhase::Schedule);
    (record, stop.or_else(|| deadline_stop(deadline)))
}

/// Preserve operation errors and incomplete Work even when the wall budget also expires.
fn operation_stop<W, E: Copy>(
    record: &IterationRecord<W, E>,
    deadline: &SamplingDeadline<'_>,
) -> Option<SamplingStop<E>> {
    if let IterationOutcome::Returned(Err(error)) = record.outcome {
        return Some(SamplingStop::HostError(SamplingPhase::Operation, error));
    }
    if let Some(receipt) = &record.receipt
        && receipt.status == ReceiptStatus::Incomplete
    {
        return Some(SamplingStop::IncompleteReceipt);
    }
    if deadline.reversed() {
        return Some(SamplingStop::ClockReversed);
    }
    if record.elapsed.is_some_and(|time| time.is_zero()) {
        return Some(SamplingStop::ClockResolution);
    }
    deadline_stop(deadline)
}
