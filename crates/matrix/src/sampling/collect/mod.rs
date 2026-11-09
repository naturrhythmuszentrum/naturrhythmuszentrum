// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared-operation collection with caller-declared cooperative execution bounds.

mod clock;
mod execute;
mod policy;
mod report;
mod runner;
use super::criterion::schedule;

use super::{
    AnalysisCaps, AnalysisError, AnalysisPhase, AnalysisReport, AnalysisSettings, SampleRow,
};
pub use clock::{InstantClock, SamplingClock, SamplingDeadline};
pub use runner::{sample, sample_with_clock};
use std::{fmt::Debug, time::Duration};

/// Bounds under the host's declared allocator, callback and scheduling assumptions.
///
/// Each component covers the entire indicated callback, including destructors.
/// Receipt preservation, disposal and cleanup are sequential, so their bounds add.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterBounds {
    /// Longest uninterrupted reset, snapshot or operation execution interval.
    pub uninterrupted: Duration,
    /// Complete exit snapshot and ordinary receipt preservation.
    pub receipt: Duration,
    /// Complete disposal of an operation output or error.
    pub disposal: Duration,
    /// Complete release of private iteration state, including any required reset.
    pub cleanup: Duration,
}

/// Host platform assumptions for the sampler's own bounded steps.
///
/// Include clock access, allocation/deallocation, analysis checkpoint chunks,
/// fixed-size receipt copying and report construction. These are explicit host
/// declarations, never calibrated defaults or inferred descheduling estimates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplerBounds {
    /// Maximum sampler execution between deadline checks.
    pub uninterrupted: Duration,
    /// Entire remaining sampler finalization from every reachable partial state.
    pub finalization: Duration,
}

/// Explicit collection policy. All fields are validated before reset or execution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingSettings {
    /// Validated complete-row statistical policy, including seed and analysis caps.
    pub analysis: AnalysisSettings,
    /// Positive geometric warm-up target, in timed operation time.
    pub warm_up: Duration,
    /// Positive linear measurement target, in timed operation time.
    pub measurement: Duration,
    /// Positive total admission budget, starting at sampler entry.
    pub wall_budget: Duration,
    /// Maximum iteration attempts across warm-up and measurement, including reset failures.
    pub max_total_iterations: u64,
    /// Maximum iterations in any warm-up group or measurement row.
    pub max_iterations_per_sample: u64,
    /// Aggregate sampler report/scratch payload caps, including the analysis caps.
    pub caps: AnalysisCaps,
    /// Declared execution assumptions for sampler steps and its complete tail.
    pub bounds: SamplerBounds,
}

/// Borrowed identity of one homogeneous representative and its measured scope.
/// Caller-owned strings and shape storage are outside sampler payload accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplingIdentity<'a> {
    /// Representative name; no population weight is inferred.
    pub representative: &'a str,
    /// Retained input shape; an empty slice can describe a scalar.
    pub shape: &'a [usize],
    /// Host operation implementation revision.
    pub operation_revision: &'a str,
    /// Prepared workload identity.
    pub workload: &'a str,
    /// Explicit measured scope, including setup/disposal only if inside the operation.
    pub scope: &'a str,
    /// Host's scoped tracing configuration, retained without installing a subscriber.
    pub trace_configuration: &'a str,
}

/// Immutable representative input and an optional declared element meaning.
#[derive(Debug)]
pub struct Representative<'a, I: ?Sized> {
    /// Input borrowed by every operation; no sampled output is published into it.
    pub item: &'a I,
    /// Positive element count, at most 2^53, or no element-rate claim.
    pub elements_per_item: Option<u64>,
    /// Retained identity and conditions for compatible estimates.
    pub identity: SamplingIdentity<'a>,
}

/// Whether all relevant ledger scopes and transfers are covered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptStatus {
    /// All relevant Work, including private ledgers, is represented.
    Complete,
    /// The adapter cannot attest complete accounting; no estimate is produced.
    Incomplete,
}

/// Operation-owned Work evidence, copied before private owners are released.
///
/// `W` can retain several ledger scopes and transfer reconciliation in a fixed
/// record. The sampler neither subtracts counters nor invents a Work tariff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkReceipt<W> {
    /// Exit snapshot immediately after stopping the timer, if available.
    pub exit: Option<W>,
    /// The operation's ordinary receipt, including private-ledger evidence.
    pub operation: Option<W>,
    /// Explicit coverage claim for all relevant scopes.
    pub status: ReceiptStatus,
}

/// An isolated, cooperative adapter; usable as a trait object with associated types fixed.
///
/// Prepared input and retained graph state must remain unchanged, including on
/// failure and repeated calls. `reset` creates fresh execution/ledger owners or
/// lawfully resets private scratch; it must never reset a live cumulative budget,
/// refund Work, rewind identities or publish into the host's graph. An adapter
/// with externally visible effects or persistent caches needs lawful isolation.
/// Return `None` from `bounds` when these contracts or cooperative bounds cannot
/// be supplied. No panic recovery or hard termination is promised.
///
/// Work/error records are fixed-size `Copy` values; referenced storage belongs to
/// the caller, must outlive the report and must remain immutable while it lives.
/// Context/output storage is adapter-owned
/// and bounded by its own resource contract. Every callback, allocator and drop
/// must obey the declared bounds. Mandatory finalization must finish even after
/// expiry; its callbacks may check the deadline but cannot abandon obligations.
pub trait HostOperation<P: ?Sized, I: ?Sized> {
    /// Fresh private execution and ledger owners.
    type Context;
    /// Ordinary output, retained only until receipt preservation and disposal.
    type Output;
    /// Fixed-size typed error, retained without formatting or allocation.
    type Error: Copy + Debug;
    /// Fixed-size scoped Work evidence; never derived from time or re-execution.
    type Work: Copy + Debug;

    /// Pure contract declaration; no preparation, allocation or execution here.
    fn bounds(&self) -> Option<AdapterBounds>;
    /// Create/reset private iteration state outside the timer.
    /// On error, release all partially created state within this callback's bound.
    /// # Errors
    /// Returns the host's ordinary reset/isolation error.
    fn reset(
        &mut self,
        prepared: &P,
        item: &I,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Self::Context, Self::Error>;
    /// Read entry Work immediately before the timer; `None` declares missing evidence.
    fn snapshot(
        &mut self,
        context: &Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Option<Self::Work>;
    /// Execute the chosen operation by borrow, including its ordinary charging/evidence.
    /// # Errors
    /// Returns the operation's ordinary typed error, without refunding its Work.
    fn run(
        &mut self,
        prepared: &P,
        item: &I,
        context: &mut Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Self::Output, Self::Error>;
    /// First read exit Work, then preserve the ordinary receipt before owners die.
    /// This mandatory callback runs immediately after the timer, including on error.
    fn receipt(
        &mut self,
        context: &Self::Context,
        output: &Result<Self::Output, Self::Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Self::Work>;
    /// Consume and completely release the output/error outside the timer.
    /// # Errors
    /// Returns a disposal error after releasing all owned output state.
    fn dispose(
        &mut self,
        output: Result<Self::Output, Self::Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Self::Error>;
    /// Consume and completely release private iteration state outside the timer.
    /// # Errors
    /// Returns a cleanup error after releasing all owned context state.
    fn cleanup(
        &mut self,
        context: Self::Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Self::Error>;
}

/// A checkpoint or mandatory finalization phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingPhase {
    /// Fixed policy admission before callbacks.
    Validation,
    /// Fallible reservation, before any operation is admitted.
    Allocation,
    /// Fresh context or lawful reset.
    Reset,
    /// Entry Work snapshot.
    Snapshot,
    /// Timer start and invocation admission.
    Operation,
    /// Mandatory exit snapshot and receipt preservation.
    Receipt,
    /// Mandatory output disposal.
    Disposal,
    /// Mandatory private-state cleanup.
    Cleanup,
    /// Checked warm-up/schedule bookkeeping.
    Schedule,
    /// An existing analysis checkpoint.
    Analysis(AnalysisPhase),
    /// Complete bounded return-report construction.
    Report,
}

/// Position of an iteration attempt, retained even for unfinished rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulePosition {
    /// Geometric warm-up group, identified by its planned size.
    WarmUp(u64),
    /// Zero-based linear measurement row.
    Measurement(u32),
}

/// An iteration's ordinary result, independently of receipt/disposal status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IterationOutcome<E> {
    /// Reset failed before any operation invocation.
    ResetFailed(E),
    /// The operation was not invoked, including expiry before reset.
    /// A cleanup result indicates that an owned context was created.
    NotInvoked,
    /// The operation returned its ordinary success or typed error.
    Returned(Result<(), E>),
}

/// All evidence for one attempted iteration; failed attempts are never refunded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IterationRecord<W, E> {
    /// Warm-up group or measurement row.
    pub position: SchedulePosition,
    /// Ordinary outcome, including reset/admission failures.
    pub outcome: IterationOutcome<E>,
    /// Timed operation only, absent if not invoked.
    pub elapsed: Option<Duration>,
    /// Entry Work snapshot immediately before the timer.
    pub entry: Option<W>,
    /// Mandatory exit evidence, absent if not invoked.
    pub receipt: Option<WorkReceipt<W>>,
    /// Disposal result, absent if no output/error was produced.
    pub disposal: Option<Result<(), E>>,
    /// Cleanup result; presence indicates reset returned an owned context.
    pub cleanup: Option<Result<(), E>>,
}

/// Pre-invocation admission failures have no engine refusal meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SamplingError {
    /// Durations, count caps or identity are invalid.
    #[error("invalid explicit sampling policy or representative")]
    InvalidSettings,
    /// A complete linear schedule cannot fit the checked iteration caps.
    #[error("sampling schedule exceeds iteration caps")]
    IterationCap,
    /// The host cannot attest isolation, receipts and cooperative execution bounds.
    #[error("unsupported host sampling contract")]
    Unsupported,
    /// Aggregate duration/count arithmetic overflowed.
    #[error("sampling arithmetic overflow")]
    Arithmetic,
    /// Aggregate report/scratch payload cannot fit its cap.
    #[error("sampling payload capacity exhausted")]
    Capacity,
    /// A fallible reservation was refused.
    #[error("sampling allocation failed")]
    Allocation,
}

/// Why collection stopped. Finalization failures also remain in iteration records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingStop<E> {
    /// Entire schedule and every requested bootstrap completed before expiry.
    Complete,
    /// Admission ended at this checkpoint; mandatory work still finished.
    Deadline(SamplingPhase),
    /// Collection admission or schedule validation failed.
    Rejected(SamplingError),
    /// A host callback failed; the typed error and phase are retained.
    HostError(SamplingPhase, E),
    /// An operation receipt lacked complete Work coverage.
    IncompleteReceipt,
    /// An invoked operation had zero measured duration.
    ClockResolution,
    /// The injected clock violated monotonicity.
    ClockReversed,
    /// Complete rows could not yield a complete analysis.
    Analysis(AnalysisError),
}

/// Immutable collection evidence; partial runs never expose an estimate.
#[derive(Debug)]
pub struct SamplingReport<'a, W, E> {
    /// All explicit requested settings.
    settings: SamplingSettings,
    /// Retained host identity and trace configuration.
    identity: SamplingIdentity<'a>,
    /// Optional declared element meaning.
    elements: Option<u64>,
    /// Declared adapter bounds, kept for independent composition checks.
    adapter_bounds: AdapterBounds,
    /// Sum of sampler and adapter uninterrupted allowances.
    uninterrupted: Duration,
    /// Sum of every sequential mandatory tail component.
    finalization: Duration,
    /// Every attempted iteration, including warm-up and failed/incomplete rows.
    iterations: Vec<IterationRecord<W, E>>,
    /// Complete measurement rows only, in schedule order.
    rows: Vec<SampleRow>,
    /// Chosen linear step, absent if warm-up did not yield a valid schedule.
    step: Option<u64>,
    /// Completed warm-up operation count.
    warm_up_iterations: u64,
    /// Sum of timed warm-up durations.
    warm_up_elapsed: Duration,
    /// Sum of all timed operations, including failed and partial rows.
    measured: Duration,
    /// Total elapsed through mandatory finalization/report construction.
    total: Duration,
    /// Complete analysis only when the whole collection succeeded.
    analysis: Option<AnalysisReport>,
    /// Stop reason after finalization, with no numerical refusal meaning.
    stop: SamplingStop<E>,
}

#[cfg(test)]
mod tests;
