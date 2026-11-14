// SPDX-License-Identifier: AGPL-3.0-only
//! Scripted dispatch evidence for sizing; no live timing oracle.

use super::{PilotOperation, PilotPolicy, PilotStop, size};
use crate::recipe::RecipeError;
use nrz_matrix::sampling::SamplingClock;
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

/// Deterministic one-nanosecond clock read cost plus scripted operation costs.
#[derive(Debug, Default)]
struct Clock(Cell<u64>);

impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        let time = self.0.get();
        self.0.set(time + 1);
        Duration::from_nanos(time)
    }
}

/// One explicit operation cost per current length; empty calls share the timer pair.
struct Script<'a> {
    /// Injected clock.
    clock: &'a Clock,
    /// Requested lengths actually dispatched.
    lengths: RefCell<Vec<u64>>,
    /// Per-unit execution cost.
    cost: u64,
    /// Explicit cancellation seam.
    stopped: bool,
}

impl PilotOperation for Script<'_> {
    fn stopped(&self) -> bool {
        self.stopped
    }
    fn prepare(&mut self, _length: u64) -> Result<(), RecipeError> {
        Ok(())
    }
    fn execute(&mut self, length: Option<u64>) -> Result<(), RecipeError> {
        let cost = length.map_or(10, |length| {
            self.lengths.borrow_mut().push(length);
            length * self.cost
        });
        self.clock.0.set(self.clock.0.get() + cost);
        Ok(())
    }
}

/// Explicit sizing fixture, not a runtime default.
const fn policy(bound: u64) -> PilotPolicy {
    PilotPolicy {
        length: 1,
        maximum_length: 1024,
        attempts: 1000,
        item_bound_ns: bound,
        resolution_ns: None,
        grow: true,
    }
}

#[test]
fn target_above_bound_dispatches_no_workload() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 80_000,
        stopped: false,
    };
    let report = size(&clock, &mut script, policy(99_999));
    assert_eq!(report.stop, PilotStop::ItemTooShort);
    assert_eq!(report.target_ns, Some(100_000));
    assert_eq!(*script.lengths.borrow(), [] as [u64; 0]);
    assert_eq!(report.empty_ns.len(), 256);
}

#[test]
fn failed_prediction_precedes_doubled_dispatch() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 80_000,
        stopped: false,
    };
    let report = size(&clock, &mut script, policy(150_000));
    assert_eq!(report.stop, PilotStop::ItemTooShort);
    assert_eq!(*script.lengths.borrow(), [1; 8]);
    assert_eq!(report.accepted_length, None);
}

#[test]
fn observed_overrun_is_not_predictive_refusal() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 160_000,
        stopped: false,
    };
    let report = size(&clock, &mut script, policy(150_000));
    assert_eq!(report.stop, PilotStop::ItemExceedsBound);
    assert_eq!(*script.lengths.borrow(), [1]);
    assert_eq!(report.batches[0].elapsed_ns, [160_001]);
}

#[test]
fn checked_doubling_accepts_a_fixed_length() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 80_000,
        stopped: false,
    };
    let report = size(&clock, &mut script, policy(1_000_000));
    assert_eq!(report.stop, PilotStop::Complete);
    assert_eq!(report.accepted_length, Some(2));
    assert_eq!(
        *script.lengths.borrow(),
        [1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2]
    );
    assert_eq!(report.observed_tick_ns, Some(1));
}

#[test]
fn cancellation_admits_no_timer_diagnostics() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 1,
        stopped: true,
    };
    let report = size(&clock, &mut script, policy(1_000_000));
    assert_eq!(report.stop, PilotStop::Stopped);
    assert_eq!(report.empty_ns, [] as [u64; 0]);
}

#[test]
fn controls_below_attempt_capacity_do_not_dispatch() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 80_000,
        stopped: false,
    };
    let mut caps = policy(1_000_000);
    caps.attempts = 255;
    let report = size(&clock, &mut script, caps);
    assert_eq!(report.stop, PilotStop::Capacity);
    assert_eq!(report.attempted, 0);
    assert_eq!(report.empty_ns, [] as [u64; 0]);
    assert_eq!(*script.lengths.borrow(), [] as [u64; 0]);
}

#[test]
fn sizing_attempt_capacity_retains_dispatched_items() {
    let clock = Clock::default();
    let mut script = Script {
        clock: &clock,
        lengths: RefCell::new(Vec::new()),
        cost: 80_000,
        stopped: false,
    };
    let mut caps = policy(1_000_000);
    caps.attempts = 259;
    let report = size(&clock, &mut script, caps);
    assert_eq!(report.stop, PilotStop::Capacity);
    assert_eq!(report.attempted, 259);
    assert_eq!(report.empty_ns.len(), 256);
    assert_eq!(*script.lengths.borrow(), [1; 3]);
    assert_eq!(report.batches[0].elapsed_ns, [80_001; 3]);
}
