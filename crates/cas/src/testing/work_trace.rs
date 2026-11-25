// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded test subscriber renders exclusive boundary receipts without changing arithmetic.

mod legacy;
mod subscriber;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::testing::common::TestResult;

use subscriber::{State, WorkSubscriber};

/// An exclusive accounting bucket in the existing fixture vocabulary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    /// Primitive key, or the surrounding replay operation.
    pub(crate) rule: String,
    /// Mathematical purpose or reconstruction phase.
    pub(crate) phase: String,
    /// Charged helper location, independent of the enclosing attribution scope.
    pub(crate) location: String,
    /// Successfully consumed exclusive Work.
    pub(crate) work: u64,
}

/// A finite aggregate of completed intervals on exactly one ledger.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Receipt {
    /// Sum of entry-to-exit deltas before subtracting projected children.
    pub(crate) inclusive: u64,
    /// Sum after subtracting only immediate same-ledger children.
    pub(crate) exclusive: u64,
    /// Number of completed entered intervals, including resumed intervals.
    pub(crate) intervals: u64,
    /// Failed intervals retain their successfully consumed prefix.
    pub(crate) refused: u64,
    /// Most recent refusal diagnostic within this finite aggregate.
    pub(crate) last_refusal: Option<String>,
}

/// One ledger's independently reconciled observation window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Ledger {
    /// Earliest observed boundary; prior consumption is excluded.
    pub(crate) first: u64,
    /// Latest observed boundary, including destruction inside the observation.
    pub(crate) last: u64,
    /// Exclusive Work assigned to named operations or purpose-bearing phases.
    pub(crate) classified: u64,
    /// Explicit gaps or unnamed root Work, never silently clamped.
    pub(crate) remainder: u64,
    /// A creation event inside this observation starts the complete private lifetime.
    pub(crate) created: bool,
    /// A closing event preserves discarded private work before destruction.
    pub(crate) closed: bool,
}

/// A link to an already consumed parent debit, not a further charge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Transfer {
    /// Private ledger whose work is being reconciled.
    pub(crate) child: u64,
    /// Ledger containing the existing reconciliation debit.
    pub(crate) parent: u64,
    /// Parent consumption just before that debit.
    pub(crate) before: u64,
    /// Parent consumption just after that debit.
    pub(crate) after: u64,
    /// Amount reconciled once on the parent.
    pub(crate) work: u64,
}

/// Observer costs are separate from the mathematical ledger and timing of the operation.
#[derive(Clone, Debug, Default)]
pub(crate) struct ObserverCost {
    /// Maximum entries in each independently bounded collection.
    pub(crate) capacity: usize,
    /// Maximum simultaneous retained causal spans.
    pub(crate) peak_spans: usize,
    /// Maximum entered interval/wrapper depth.
    pub(crate) peak_depth: usize,
    /// State-mutating callbacks, independent of Work charges.
    pub(crate) callbacks: u64,
    /// Time inside those callbacks, including lock acquisition and the measurement itself.
    pub(crate) callback_time: Duration,
}

/// Bounded observer output; incompleteness belongs to the harness, never the operation.
#[derive(Clone, Debug, Default)]
pub(crate) struct Report {
    /// Storage population and callback time, never charged as Work.
    pub(crate) observer: ObserverCost,
    /// Compatibility rendering summed across the separately reported ledgers.
    pub(crate) rows: Vec<Row>,
    /// Ledger, rule, purpose and helper uniquely identify one aggregate.
    pub(crate) receipts: BTreeMap<(u64, String, String, String), Receipt>,
    /// Each incarnation reconciles before any transfer relation is considered.
    pub(crate) ledgers: BTreeMap<u64, Ledger>,
    /// Explicit existing-debit links.
    pub(crate) transfers: Vec<Transfer>,
    /// False for missing boundaries, invalid identities, overflow or observer capacity loss.
    pub(crate) complete: bool,
}

/// Scoped dispatchers rebuild process-wide callsite interest, so fixtures share one guard.
static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Hold across each subscriber lifetime; concurrency controls coordinate their own child threads.
pub(crate) fn dispatcher_guard() -> MutexGuard<'static, ()> {
    DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Disabled fixture assertions cannot race another fixture's interest rebuild.
pub(crate) fn without_observation<T>(run: impl FnOnce() -> T) -> T {
    let _guard = dispatcher_guard();
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), run)
}

/// Collect ordinary operations under a scoped subscriber with bounded storage.
pub(crate) fn observed<T>(capacity: usize, run: impl FnOnce() -> T) -> (T, Report) {
    filtered(capacity, true, run)
}

/// Keep lifecycle events enabled while testing the actual phase callsite's filter.
pub(crate) fn filtered<T>(capacity: usize, phases: bool, run: impl FnOnce() -> T) -> (T, Report) {
    let _guard = dispatcher_guard();
    uncoordinated(capacity, phases, run)
}

/// The caller holds the fixture guard while deliberately racing dispatchers in child threads.
pub(crate) fn uncoordinated<T>(
    capacity: usize,
    phases: bool,
    run: impl FnOnce() -> T,
) -> (T, Report) {
    let state = Arc::new(Mutex::new(State::new(capacity)));
    let subscriber = WorkSubscriber {
        state: Arc::clone(&state),
        phases,
    };
    let result = tracing::subscriber::with_default(subscriber, run);
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(poisoned) => {
            let mut state = poisoned.into_inner();
            state.lost = true;
            state
        }
    };
    (result, state.report())
}

/// Keep refused results; reject incomplete evidence without replacing the active observer.
pub(crate) fn measured<T>(run: impl FnOnce() -> T) -> TestResult<(T, Vec<Row>)> {
    let (result, report) = measured_report(run)?;
    Ok((result, report.rows))
}

/// Retain the explicit per-ledger remainder for fixtures that reconcile a whole call.
pub(crate) fn measured_report<T>(run: impl FnOnce() -> T) -> TestResult<(T, Report)> {
    let nested = tracing::dispatcher::get_default(|dispatch| {
        dispatch.downcast_ref::<WorkSubscriber>().is_some()
    });
    if nested {
        return Err("nested Work observation".into());
    }
    let (result, report) = observed(4096, run);
    if !report.complete {
        return Err("incomplete Work observation".into());
    }
    legacy::compare(&report)?;
    Ok((result, report))
}
