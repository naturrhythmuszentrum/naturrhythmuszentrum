// SPDX-License-Identifier: AGPL-3.0-only
//! Monotonic clock seam and a latched cooperative deadline.

use super::SamplingPhase;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// Injected monotonic clock. Checkpoint hooks support deterministic phase-cost tests.
/// Implementations must have bounded calls; the production clock uses `Instant`.
pub trait SamplingClock {
    /// A monotonic timestamp relative to a clock-owned epoch.
    fn now(&self) -> Duration;
    /// Optional bounded instrumentation, never called between timer stop and receipt.
    fn checkpoint(&self, _phase: SamplingPhase) {}
}

/// Production `std::time::Instant` clock, with no process-global state.
#[derive(Debug)]
pub struct InstantClock(Instant);

impl InstantClock {
    /// Start a private monotonic epoch.
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for InstantClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SamplingClock for InstantClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Cooperative admission deadline shared by the sampler and host callbacks.
/// Expiry is latched; checking inside finalization never cancels its obligations.
pub struct SamplingDeadline<'a> {
    /// Clock borrowed for this invocation only.
    clock: &'a dyn SamplingClock,
    /// Entry timestamp before validation.
    started: Duration,
    /// Explicit host wall allowance.
    budget: Duration,
    /// Last observed timestamp, retaining monotonicity evidence.
    last: Cell<Duration>,
    /// First checkpoint observing expiry.
    expired: Cell<Option<SamplingPhase>>,
    /// An invalid injected clock is an explicit outcome.
    reversed: Cell<bool>,
}

impl std::fmt::Debug for SamplingDeadline<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplingDeadline")
            .field("started", &self.started)
            .field("budget", &self.budget)
            .field("expired", &self.expired)
            .field("reversed", &self.reversed)
            .finish_non_exhaustive()
    }
}

impl<'a> SamplingDeadline<'a> {
    /// Capture entry before all validation, allocation and callbacks.
    pub(super) fn new(clock: &'a dyn SamplingClock, budget: Duration) -> Self {
        let started = clock.now();
        Self::at(clock, budget, started)
    }

    /// Use an already captured entry epoch, including the production wrapper cost.
    pub(super) const fn at(
        clock: &'a dyn SamplingClock,
        budget: Duration,
        started: Duration,
    ) -> Self {
        Self {
            clock,
            started,
            budget,
            last: Cell::new(started),
            expired: Cell::new(None),
            reversed: Cell::new(false),
        }
    }

    /// Observe a bounded checkpoint and latch expiry. `true` forbids ordinary work.
    #[must_use]
    pub fn expired(&self, phase: SamplingPhase) -> bool {
        self.clock.checkpoint(phase);
        self.stamp(phase);
        self.is_stopped()
    }

    /// Total observed elapsed time; does not infer scheduling delay.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.stamp(SamplingPhase::Report)
            .saturating_sub(self.started)
    }

    /// Requested wall allowance.
    #[must_use]
    pub const fn budget(&self) -> Duration {
        self.budget
    }

    /// Read the timer directly, without inserting instrumentation around Work snapshots.
    pub(super) fn stamp(&self, phase: SamplingPhase) -> Duration {
        let now = self.clock.now();
        if now < self.last.get() {
            self.reversed.set(true);
        }
        self.last.set(now);
        if now.saturating_sub(self.started) >= self.budget && self.expired.get().is_none() {
            self.expired.set(Some(phase));
        }
        now
    }

    /// Whether any previous observation forbids ordinary admission.
    pub(super) const fn is_stopped(&self) -> bool {
        self.reversed.get() || self.expired.get().is_some()
    }

    /// First observed expiry, independently of other stop reasons.
    pub(super) const fn expiry(&self) -> Option<SamplingPhase> {
        self.expired.get()
    }

    /// Whether the clock violated its monotonic contract.
    pub(super) const fn reversed(&self) -> bool {
        self.reversed.get()
    }
}
