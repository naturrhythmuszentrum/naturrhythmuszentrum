// SPDX-License-Identifier: AGPL-3.0-only
//! Host-sampler adapter for raw computation without an engine Work ledger.

use crate::{
    CancellationToken, ExecutionBounds, pilot::PilotOperation, recipe::RecipeError, workers::Runner,
};
use nrz_matrix::sampling::{
    AdapterBounds, HostOperation, ReceiptStatus, SamplingClock, SamplingDeadline, SamplingPhase,
    WorkReceipt,
};
use std::time::Duration;

/// Explicit absence of relevant engine scopes, not a zero-cost Work tariff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) struct NoEngineWork;

/// Borrowed outer admission state shared by pilot and sampling callbacks.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) struct Admission<'a> {
    /// Run cancellation token.
    pub(super) cancellation: &'a CancellationToken,
    /// Whole-run clock epoch.
    pub(super) clock: &'a dyn SamplingClock,
    /// Absolute deadline relative to that epoch.
    pub(super) end: Duration,
}

impl Admission<'_> {
    /// Read cancellation and real clock without falsifying either.
    pub(super) fn stopped(&self) -> bool {
        self.cancellation.is_cancelled() || self.clock.now() >= self.end
    }
}

/// Prepared raw operation, with reset and output consumption outside engine state.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) struct Adapter<'a> {
    /// Prepared runtime coefficient/state inputs.
    pub(super) runner: Runner,
    /// Outer cooperative admission.
    pub(super) admission: Admission<'a>,
    /// Host execution assumptions.
    pub(super) bounds: ExecutionBounds,
    /// Accumulated reset/dry-validation wall time, outside kernel timers.
    pub(super) reset_ns: u64,
}

impl Adapter<'_> {
    /// Preserve reset duration even when restoring or dry validation returns an error.
    fn reset_item(&mut self, length: u64) -> Result<(), RecipeError> {
        let started = self.admission.clock.now();
        let result = self.runner.reset(length);
        let elapsed = self
            .admission
            .clock
            .now()
            .checked_sub(started)
            .ok_or(RecipeError::Domain)?;
        let elapsed = u64::try_from(elapsed.as_nanos()).map_err(|_| RecipeError::Overflow)?;
        self.reset_ns = self
            .reset_ns
            .checked_add(elapsed)
            .filter(|n| *n <= crate::decimal::MAX_INTEGER)
            .ok_or(RecipeError::Overflow)?;
        result
    }
}

impl PilotOperation for Adapter<'_> {
    fn stopped(&self) -> bool {
        self.admission.stopped()
    }
    fn prepare(&mut self, length: u64) -> Result<(), RecipeError> {
        if self.admission.stopped() {
            return Err(RecipeError::Cancelled);
        }
        self.reset_item(length)
    }
    fn execute(&mut self, length: Option<u64>) -> Result<(), RecipeError> {
        self.runner.execute(length)
    }
}

impl HostOperation<(), u64> for Adapter<'_> {
    type Context = ();
    type Output = ();
    type Error = RecipeError;
    type Work = NoEngineWork;

    fn bounds(&self) -> Option<AdapterBounds> {
        Some(AdapterBounds {
            uninterrupted: Duration::from_nanos(self.bounds.item_ns),
            receipt: Duration::from_nanos(self.bounds.receipt_ns),
            disposal: Duration::from_nanos(self.bounds.disposal_ns),
            cleanup: Duration::from_nanos(self.bounds.cleanup_ns),
        })
    }
    fn reset(
        &mut self,
        (): &(),
        item: &u64,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), RecipeError> {
        if self.admission.stopped() || deadline.expired(SamplingPhase::Reset) {
            return Err(RecipeError::Cancelled);
        }
        self.reset_item(*item)
    }
    fn snapshot(&mut self, (): &(), _deadline: &SamplingDeadline<'_>) -> Option<NoEngineWork> {
        Some(NoEngineWork)
    }
    fn run(
        &mut self,
        (): &(),
        item: &u64,
        (): &mut (),
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), RecipeError> {
        if self.admission.stopped() || deadline.expired(SamplingPhase::Operation) {
            return Err(RecipeError::Cancelled);
        }
        self.runner.execute(Some(*item))
    }
    fn receipt(
        &mut self,
        (): &(),
        _output: &Result<(), RecipeError>,
        _deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<NoEngineWork> {
        WorkReceipt {
            exit: Some(NoEngineWork),
            operation: Some(NoEngineWork),
            status: ReceiptStatus::Complete,
        }
    }
    fn dispose(
        &mut self,
        _output: Result<(), RecipeError>,
        _deadline: &SamplingDeadline<'_>,
    ) -> Result<(), RecipeError> {
        Ok(())
    }
    fn cleanup(&mut self, (): (), _deadline: &SamplingDeadline<'_>) -> Result<(), RecipeError> {
        Ok(())
    }
}

impl std::fmt::Debug for Admission<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Admission")
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}
