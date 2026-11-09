// SPDX-License-Identifier: AGPL-3.0-only
//! Projection, interval subtraction and bounded aggregation live wholly in the subscriber.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Instant;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

use super::{Ledger, ObserverCost, Report, Row, Transfer};

/// Fixed vocabulary and bounded fault text keep each retained entry finite.
const FIELD_BYTES: usize = 512;

/// Refusal formatting stops at the observer's field capacity.
#[derive(Default)]
struct BoundedText {
    /// Bounded UTF-8 payload.
    text: String,
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.text.len().saturating_add(value.len()) > FIELD_BYTES {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

/// Structured fields copied only inside the enabled subscriber.
#[derive(Clone, Debug, Default)]
struct Fields {
    /// Overlong fields invalidate evidence without retaining unbounded text.
    lost: bool,
    /// Fixed family identity of the operation.
    family: String,
    /// Outer completion boundary; outside controls remain separately attributable.
    boundary: String,
    /// Event counter amount.
    count: u64,
    /// Causal rule name, inherited through ledgerless wrappers.
    rule: Option<String>,
    /// This subscriber observes the CAS owner namespace.
    owner: Option<String>,
    /// Causal purpose name.
    phase: Option<String>,
    /// An absent operation denotes unclassified root work.
    operation: String,
    /// Owner-qualified nonwrapping ledger incarnation.
    ledger: Option<u64>,
    /// Most recently recorded boundary snapshot.
    work: Option<u64>,
    /// Completed, refused or suspended interval outcome.
    outcome: String,
    /// Bounded structured fault rendering from an emitted refusal.
    reason: String,
    /// Ledger lifecycle action.
    action: String,
    /// Transfer's source incarnation.
    child: Option<u64>,
    /// Transfer's target incarnation.
    parent: Option<u64>,
    /// Parent before its existing debit.
    before: Option<u64>,
    /// Parent after its existing debit.
    after: Option<u64>,
}

impl Visit for Fields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "count" => self.count = value,
            "ledger" => self.ledger = Some(value),
            "work" => self.work = Some(value),
            "child" => self.child = Some(value),
            "parent" => self.parent = Some(value),
            "before" => self.before = Some(value),
            "after" => self.after = Some(value),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if value.len() > FIELD_BYTES {
            self.lost = true;
            return;
        }
        match field.name() {
            "family" => value.clone_into(&mut self.family),
            "rule" => self.rule = Some(value.into()),
            "owner" => self.owner = Some(value.into()),
            "phase" => self.phase = Some(value.into()),
            "operation" => value.clone_into(&mut self.operation),
            "outcome" => value.clone_into(&mut self.outcome),
            "action" => value.clone_into(&mut self.action),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "reason" {
            let mut text = BoundedText::default();
            self.lost |= fmt::write(&mut text, format_args!("{value:?}")).is_err();
            self.reason = text.text;
        }
    }
}

/// One currently entered interval; children are projected by ledger, not causal parentage.
#[derive(Debug)]
struct Interval {
    /// Incarnation cannot change within an entered interval.
    ledger: u64,
    /// Entry snapshot.
    start: u64,
    /// Nearest active interval on this ledger.
    parent: Option<u64>,
    /// Disjoint inclusive deltas of immediate projected children.
    children: u64,
}

/// An active causal span may be suspended and entered again.
#[derive(Debug)]
struct SpanState {
    /// Structured values and inherited classification.
    fields: Fields,
    /// Live span handles; completed history is aggregated rather than retained.
    references: u64,
    /// Fixed metadata name, never supplied by the input.
    name: String,
    /// Independent wall boundary for this entered span.
    started: Option<Instant>,
    /// At most one sequential owner enters a ledger interval at a time.
    interval: Option<Interval>,
    /// An exit requires a new snapshot after its entry.
    fresh: bool,
}

/// Bounded observer storage, independent of every mathematical allowance.
#[derive(Debug)]
pub(super) struct State {
    /// Nonwrapping epoch advanced on each measurement entry and exit.
    boundary_epoch: u64,
    /// First native entry since observation began, retained even after exit.
    first_native_epoch: Option<u64>,
    /// Latest native exit; surviving ledgers require a snapshot at or after this epoch.
    last_native_exit: Option<u64>,
    /// Maximum live spans, ledgers, receipt keys and transfer records, separately.
    capacity: usize,
    /// Nonwrapping subscriber span identities.
    next: u64,
    /// Sequential mutation owner; cross-thread use requires another observer.
    thread: ThreadId,
    /// Active causal span handles.
    spans: BTreeMap<u64, SpanState>,
    /// Entered stack includes ledgerless scopes and other ledgers.
    stack: Vec<u64>,
    /// Finite vocabulary aggregates and reconciliation windows.
    result: Report,
    /// Loss never escapes into arithmetic.
    pub(super) lost: bool,
}

impl State {
    /// Declare the observer's finite capacities before running an operation.
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            boundary_epoch: 0,
            first_native_epoch: None,
            last_native_exit: None,
            capacity,
            next: 1,
            thread: std::thread::current().id(),
            spans: BTreeMap::new(),
            stack: Vec::new(),
            result: Report {
                observer: ObserverCost {
                    capacity,
                    ..ObserverCost::default()
                },
                ..Report::default()
            },
            lost: false,
        }
    }

    /// Require monotone snapshots and reserve each new ledger within observer capacity.
    fn snapshot(&mut self, ledger: u64, work: u64) {
        if ledger == 0 {
            self.lost = true;
            return;
        }
        if !self.result.ledgers.contains_key(&ledger) {
            if work != 0 && self.first_native_epoch.is_some() {
                self.lost = true;
            }
            if self.result.ledgers.len() >= self.capacity {
                self.lost = true;
                return;
            }
            self.result.ledgers.insert(
                ledger,
                Ledger {
                    boundary_epoch: self.boundary_epoch,
                    first: work,
                    last: work,
                    classified: 0,
                    remainder: 0,
                    created: false,
                    closed: false,
                },
            );
        }
        let previous = self
            .result
            .ledgers
            .get(&ledger)
            .map(|value| (value.last, value.boundary_epoch));
        if let Some(value) = self.result.ledgers.get_mut(&ledger) {
            if work < value.last || value.closed {
                self.lost = true;
            }
            value.last = work;
            value.boundary_epoch = self.boundary_epoch;
        }
        if let Some((last, epoch)) = previous
            && let Some(delta) = work.checked_sub(last)
        {
            if delta != 0 && epoch != self.boundary_epoch {
                self.lost = true;
                return;
            }
            let boundary = self
                .stack
                .last()
                .and_then(|id| self.spans.get(id))
                .map_or_default(|span| span.fields.boundary.clone());
            if !boundary.is_empty() && delta != 0 {
                let key = (boundary, ledger);
                if !self.result.boundary_work.contains_key(&key)
                    && self.result.boundary_work.len() >= self.capacity
                {
                    self.lost = true;
                } else {
                    Self::add(
                        self.result.boundary_work.entry(key).or_default(),
                        delta,
                        &mut self.lost,
                    );
                }
            }
        }
    }

    /// Creation and destruction distinguish an incarnation from a reused identifier.
    fn lifecycle(&mut self, fields: &Fields) {
        if fields.owner.as_deref() != Some("nrz-cas") {
            self.lost = true;
            return;
        }
        let (Some(id), Some(work)) = (fields.ledger, fields.work) else {
            self.lost = true;
            return;
        };
        if fields.action == "created" && self.result.ledgers.contains_key(&id) {
            self.lost = true;
            return;
        }
        self.snapshot(id, work);
        if let Some(ledger) = self.result.ledgers.get_mut(&id) {
            match fields.action.as_str() {
                "created" => ledger.created = true,
                "closed" => ledger.closed = true,
                _ => self.lost = true,
            }
        }
    }

    /// Sum with checked arithmetic; loss is evidence failure, never a replacement receipt.
    const fn add(target: &mut u64, amount: u64, lost: &mut bool) {
        if let Some(total) = target.checked_add(amount) {
            *target = total;
        } else {
            *lost = true;
        }
    }

    /// Open one interval against the currently active same-ledger projection.
    fn enter(&mut self, id: u64) {
        let Some(span) = self.spans.get(&id) else {
            self.lost = true;
            return;
        };
        if self.stack.len() >= self.capacity || self.stack.contains(&id) {
            self.lost = true;
            return;
        }
        let measurement = span.name == "nrz.measure";
        let native = measurement && span.fields.family.starts_with("native-");
        let nested_in_native = measurement
            && self.stack.iter().any(|active| {
                self.spans.get(active).is_some_and(|parent| {
                    parent.name == "nrz.measure" && parent.fields.family.starts_with("native-")
                })
            });
        if nested_in_native {
            self.lost = true;
        }
        let interval = match (span.fields.ledger, span.fields.work) {
            (Some(ledger), Some(work)) if span.fresh => {
                let parent = self.stack.iter().rev().copied().find(|active| {
                    self.spans
                        .get(active)
                        .and_then(|span| span.interval.as_ref())
                        .is_some_and(|interval| interval.ledger == ledger)
                });
                self.snapshot(ledger, work);
                Some(Interval {
                    ledger,
                    start: work,
                    parent,
                    children: 0,
                })
            }
            (None, None) => None,
            _ => {
                self.lost = true;
                return;
            }
        };
        if let Some(span) = self.spans.get_mut(&id) {
            span.started = Some(Instant::now());
            span.interval = interval;
            span.fresh = false;
            span.fields.outcome.clear();
            span.fields.reason.clear();
        }
        if measurement {
            Self::add(&mut self.boundary_epoch, 1, &mut self.lost);
        }
        if native && self.first_native_epoch.is_none() {
            self.first_native_epoch = Some(self.boundary_epoch);
        }
        self.stack.push(id);
        self.result.observer.peak_depth = self.result.observer.peak_depth.max(self.stack.len());
    }

    /// Close against the actual interval forest and aggregate only exclusive named work.
    fn exit(&mut self, id: u64) {
        if self.stack.pop() != Some(id) {
            self.lost = true;
            return;
        }
        let Some(span) = self.spans.get_mut(&id) else {
            self.lost = true;
            return;
        };
        if span.name == "nrz.measure" {
            Self::add(&mut self.boundary_epoch, 1, &mut self.lost);
        }
        if span.name == "nrz.measure" && span.fields.family.starts_with("native-") {
            self.last_native_exit = Some(self.boundary_epoch);
        }
        let name = if span.fields.operation.is_empty() {
            &span.name
        } else {
            &span.fields.operation
        };
        let key = (
            span.fields.boundary.clone(),
            name.clone(),
            span.fields.family.clone(),
        );
        if let Some(started) = span.started.take() {
            if !self.result.clocks.contains_key(&key) && self.result.clocks.len() >= self.capacity {
                self.lost = true;
            } else {
                let clock = self.result.clocks.entry(key).or_default();
                if let Some(total) = clock.0.checked_add(started.elapsed()) {
                    clock.0 = total;
                } else {
                    self.lost = true;
                }
                Self::add(&mut clock.1, 1, &mut self.lost);
            }
        }
        let Some(interval) = span.interval.take() else {
            return;
        };
        let fields = span.fields.clone();
        let valid_outcome = matches!(
            fields.outcome.as_str(),
            "completed" | "refused" | "suspended"
        );
        if !span.fresh || fields.ledger != Some(interval.ledger) || !valid_outcome {
            self.lost = true;
            return;
        }
        let Some(end) = fields.work else {
            self.lost = true;
            return;
        };
        let Some(inclusive) = end.checked_sub(interval.start) else {
            self.lost = true;
            return;
        };
        let Some(exclusive) = inclusive.checked_sub(interval.children) else {
            self.lost = true;
            return;
        };
        self.snapshot(interval.ledger, end);
        if let Some(parent) = interval.parent {
            if let Some(parent) = self
                .spans
                .get_mut(&parent)
                .and_then(|span| span.interval.as_mut())
            {
                Self::add(&mut parent.children, inclusive, &mut self.lost);
            } else {
                self.lost = true;
            }
        }
        self.receipt(interval.ledger, fields, inclusive, exclusive);
    }

    /// Bound receipt keys independently of the number of historical operations.
    fn receipt(&mut self, ledger: u64, fields: Fields, inclusive: u64, exclusive: u64) {
        let classified = !fields.operation.is_empty();
        let key = (
            ledger,
            fields.rule.unwrap_or_else(|| "replay".into()),
            fields.phase.unwrap_or_else(|| "overhead".into()),
            fields.operation,
        );
        if !self.result.receipts.contains_key(&key) && self.result.receipts.len() >= self.capacity {
            self.lost = true;
            return;
        }
        let receipt = self.result.receipts.entry(key).or_default();
        Self::add(&mut receipt.inclusive, inclusive, &mut self.lost);
        Self::add(&mut receipt.exclusive, exclusive, &mut self.lost);
        Self::add(&mut receipt.intervals, 1, &mut self.lost);
        if fields.outcome == "refused" {
            Self::add(&mut receipt.refused, 1, &mut self.lost);
            receipt.last_refusal = Some(fields.reason);
        }
        if classified && let Some(value) = self.result.ledgers.get_mut(&ledger) {
            Self::add(&mut value.classified, exclusive, &mut self.lost);
        }
    }

    /// Transfers validate the existing debit after each ledger has its own receipts.
    fn transfer(&mut self, fields: &Fields) {
        if fields.owner.as_deref() != Some("nrz-cas") {
            self.lost = true;
            return;
        }
        let (Some(child), Some(parent), Some(before), Some(after), Some(work)) = (
            fields.child,
            fields.parent,
            fields.before,
            fields.after,
            fields.work,
        ) else {
            self.lost = true;
            return;
        };
        if child == parent
            || after.checked_sub(before) != Some(work)
            || self.result.transfers.len() >= self.capacity
        {
            self.lost = true;
            return;
        }
        let duplicate = self.result.transfers.iter().any(|previous| {
            previous.parent == parent
                && (previous.before == before && previous.after == after
                    || previous.before < after && before < previous.after)
        });
        if duplicate {
            self.lost = true;
            return;
        }
        self.result.transfers.push(Transfer {
            child,
            parent,
            before,
            after,
            work,
        });
    }

    /// Reconcile all ledgers and produce the old sorted rendering without zero buckets.
    pub(super) fn report(&mut self) -> Report {
        if !self.stack.is_empty() || !self.spans.is_empty() {
            self.lost = true;
        }
        for ledger in self.result.ledgers.values_mut() {
            let missing_terminal = !ledger.closed
                && self
                    .last_native_exit
                    .map_or(ledger.created, |exit| ledger.boundary_epoch < exit);
            if missing_terminal {
                self.lost = true;
            }
            if let Some(remainder) = ledger
                .last
                .checked_sub(ledger.first)
                .and_then(|delta| delta.checked_sub(ledger.classified))
            {
                ledger.remainder = remainder;
            } else {
                self.lost = true;
            }
        }
        let mut rows = BTreeMap::new();
        for ((_, rule, phase, location), receipt) in &self.result.receipts {
            if receipt.exclusive != 0 && !location.is_empty() {
                let value = rows
                    .entry((rule.clone(), phase.clone(), location.clone()))
                    .or_insert(0);
                Self::add(value, receipt.exclusive, &mut self.lost);
            }
        }
        self.result.rows = rows
            .into_iter()
            .map(|((rule, phase, location), work)| Row {
                rule,
                phase,
                location,
                work,
            })
            .collect();
        for transfer in &self.result.transfers {
            let child = self.result.ledgers.get(&transfer.child);
            let parent = self.result.ledgers.get(&transfer.parent);
            let valid = child.is_some_and(|ledger| {
                ledger
                    .last
                    .checked_sub(ledger.first)
                    .is_some_and(|delta| delta >= transfer.work)
            }) && parent.is_some_and(|ledger| {
                ledger.first <= transfer.before && ledger.last >= transfer.after
            });
            if !valid {
                self.lost = true;
            }
        }
        self.result.complete = !self.lost;
        self.result.clone()
    }
}

/// Hand-written subscriber avoids a production observer dependency and propagates no faults.
#[derive(Clone, Debug)]
pub(super) struct WorkSubscriber {
    /// Bounded shared observation state.
    pub(super) state: Arc<Mutex<State>>,
    /// The filtering control can reject phase callsites while still enabling lifecycle events.
    pub(super) phases: bool,
}

impl WorkSubscriber {
    /// Time observer callbacks and isolate poisoned/cross-thread state from arithmetic.
    fn with_state<T>(&self, run: impl FnOnce(&mut State) -> T) -> T {
        let start = Instant::now();
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.lost = true;
                state
            }
        };
        if state.thread != std::thread::current().id() {
            state.lost = true;
        }
        let result = run(&mut state);
        if let Some(total) = state
            .result
            .observer
            .callback_time
            .checked_add(start.elapsed())
        {
            state.result.observer.callback_time = total;
        } else {
            state.lost = true;
        }
        if let Some(total) = state.result.observer.callbacks.checked_add(1) {
            state.result.observer.callbacks = total;
        } else {
            state.lost = true;
        }
        drop(state);
        result
    }
}

impl Subscriber for WorkSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        if matches!(
            metadata.name(),
            "nrz.kernel" | "nrz.validation" | "nrz.import" | "nrz.measure"
        ) {
            return self.phases;
        }
        metadata.target() == "nrz::work"
            && match metadata.name() {
                "nrz.work.phase" => self.phases,
                "nrz.work.ledger"
                | "nrz.work.transfer"
                | "nrz.observer.loss"
                | "nrz.construct"
                | "nrz.measure.counter" => true,
                _ => false,
            }
    }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        self.with_state(|state| {
            let id = state.next;
            let Some(next) = id.checked_add(1) else {
                state.lost = true;
                return Id::from_u64(id);
            };
            state.next = next;
            if state.spans.len() >= state.capacity {
                state.lost = true;
                return Id::from_u64(id);
            }
            let parent = attributes.parent().map(Id::into_u64).or_else(|| {
                attributes
                    .is_contextual()
                    .then(|| state.stack.last().copied())
                    .flatten()
            });
            let mut fields = Fields::default();
            if let Some(parent) = parent.and_then(|parent| state.spans.get(&parent)) {
                fields.boundary.clone_from(&parent.fields.boundary);
                fields.rule.clone_from(&parent.fields.rule);
                fields.phase.clone_from(&parent.fields.phase);
            }
            attributes.record(&mut fields);
            if attributes.metadata().name() == "nrz.measure" {
                fields.boundary.clone_from(&fields.family);
            }
            state.lost |= fields.lost;
            if fields.ledger.is_some() && fields.owner.as_deref() != Some("nrz-cas") {
                state.lost = true;
            }
            state.spans.insert(
                id,
                SpanState {
                    fields,
                    references: 1,
                    name: attributes.metadata().name().into(),
                    started: None,
                    interval: None,
                    fresh: true,
                },
            );
            state.result.observer.peak_spans =
                state.result.observer.peak_spans.max(state.spans.len());
            Id::from_u64(id)
        })
    }

    fn record(&self, id: &Id, values: &Record<'_>) {
        self.with_state(|state| {
            if let Some(span) = state.spans.get_mut(&id.into_u64()) {
                let mut update = Fields::default();
                values.record(&mut update);
                if update.work.is_some() {
                    span.fresh = true;
                }
                values.record(&mut span.fields);
                state.lost |= span.fields.lost;
            } else {
                state.lost = true;
            }
        });
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        self.with_state(|state| {
            let mut fields = Fields::default();
            event.record(&mut fields);
            state.lost |= fields.lost;
            match event.metadata().name() {
                "nrz.construct" | "nrz.measure.counter" => {
                    let boundary = state
                        .stack
                        .last()
                        .and_then(|id| state.spans.get(id))
                        .map_or_else(String::new, |span| span.fields.boundary.clone());
                    let boundary = if boundary.is_empty() {
                        fields.family
                    } else {
                        boundary
                    };
                    let key = (boundary, fields.operation);
                    if !state.result.counters.contains_key(&key)
                        && state.result.counters.len() >= state.capacity
                    {
                        state.lost = true;
                    } else {
                        let count = state.result.counters.entry(key).or_default();
                        State::add(count, fields.count, &mut state.lost);
                    }
                }
                "nrz.observer.loss" => state.lost = true,
                "nrz.work.ledger" => {
                    state.lifecycle(&fields);
                }
                "nrz.work.transfer" => state.transfer(&fields),
                _ => {}
            }
        });
    }

    fn enter(&self, id: &Id) {
        self.with_state(|state| state.enter(id.into_u64()));
    }

    fn exit(&self, id: &Id) {
        self.with_state(|state| state.exit(id.into_u64()));
    }

    fn clone_span(&self, id: &Id) -> Id {
        self.with_state(|state| {
            if let Some(span) = state.spans.get_mut(&id.into_u64()) {
                if let Some(references) = span.references.checked_add(1) {
                    span.references = references;
                } else {
                    state.lost = true;
                }
            } else {
                state.lost = true;
            }
            id.clone()
        })
    }

    fn try_close(&self, id: Id) -> bool {
        self.with_state(|state| {
            let key = id.into_u64();
            let Some(span) = state.spans.get_mut(&key) else {
                state.lost = true;
                return false;
            };
            if span.references > 1 {
                span.references -= 1;
                return false;
            }
            if span.interval.is_some() || state.stack.contains(&key) {
                state.lost = true;
            }
            state.spans.remove(&key);
            true
        })
    }
}
