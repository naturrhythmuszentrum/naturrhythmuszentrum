// SPDX-License-Identifier: AGPL-3.0-only
//! Actual phase spans preserve one cumulative ledger across admission and prepaid cancellation.
use super::{LcOmegaOutput, Limits, Scalar, TestResult, demand, request, setup};
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Each dispatcher records a bounded two-phase attempt on its own thread.
static DISPATCHERS: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct Receipt {
    name: &'static str,
    parent: Option<u64>,
    consumer: String,
    application: u64,
    join: u64,
    correspondence: u64,
    seal_application: u64,
    query: u64,
    output_declaration: u64,
    sign_attempts_application: u64,
    source_closure: u64,
    attempt: u64,
    outcome_records: u64,
    terminal_revision: u64,
    delivered: bool,
    demand: String,
    completed_mask: String,
    terminal_ticket: String,
    operation: String,
    outcome: String,
    reason: String,
    ledger: u64,
    work: Vec<u64>,
}
impl Visit for Receipt {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "operation" => self.operation = value.into(),
            "outcome" => self.outcome = value.into(),
            "reason" => self.reason = value.into(),
            "consumer" => self.consumer = value.into(),
            _ => {}
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "ledger" => self.ledger = value,
            "application" => self.application = value,
            "join" => self.join = value,
            "correspondence" => self.correspondence = value,
            "seal_application" => self.seal_application = value,
            "query" => self.query = value,
            "output_declaration" => self.output_declaration = value,
            "sign_attempts_application" => self.sign_attempts_application = value,
            "source_closure" => self.source_closure = value,
            "attempt" => self.attempt = value,
            "outcome_records" => self.outcome_records = value,
            "terminal_revision" => self.terminal_revision = value,
            "work" => self.work.push(value),
            _ => {}
        }
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "delivered" {
            self.delivered = value;
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "demand" => self.demand = format!("{value:?}"),
            "completed_mask" => self.completed_mask = format!("{value:?}"),
            "terminal_ticket" => self.terminal_ticket = format!("{value:?}"),
            _ => {}
        }
    }
}
struct Observer(Arc<Mutex<Vec<Receipt>>>, Mutex<Vec<u64>>);
impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        matches!(
            metadata.name(),
            "nrz.work.phase"
                | "nrz.loan"
                | "nrz.retire"
                | "nrz.walk"
                | "nrz.barrier"
                | "nrz.publish"
        )
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut receipts = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut receipt = Receipt {
            name: attributes.metadata().name(),
            parent: attributes.parent().map(Id::into_u64).or_else(|| {
                self.1
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .last()
                    .copied()
            }),
            ..Receipt::default()
        };
        attributes.record(&mut receipt);
        receipts.push(receipt);
        Id::from_u64(u64::try_from(receipts.len()).expect("bounded receipt count"))
    }
    fn record(&self, id: &Id, values: &Record<'_>) {
        let mut receipts = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = usize::try_from(id.into_u64() - 1).expect("bounded receipt ID");
        values.record(&mut receipts[index]);
    }
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, id: &Id) {
        self.1
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(id.into_u64());
    }
    fn exit(&self, id: &Id) {
        assert_eq!(
            self.1
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop(),
            Some(id.into_u64())
        );
    }
}

pub(super) fn check<P: Scalar + 'static>() -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for short in [false, true] {
        abandonment::<P>(short)?;
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            0,
            0,
        )?;
        let tiles = members.references();
        let prepared = output
            .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let selected = demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
        let consume = s.account.limits().work - s.account.usage().work - 1_039 + u64::from(short);
        s.account.ledger.work(consume, crate::loan::AT)?;
        let before = s.account.usage().work;
        let receipts = Arc::new(Mutex::new(Vec::new()));
        crate::observer_dispatch::with_observer(
            Observer(Arc::clone(&receipts), Mutex::new(Vec::new())),
            || -> TestResult {
                let result = prepared.admit(&mut s.heap, selected, &mut s.account);
                if short {
                    assert!(result.is_err());
                } else {
                    let mut pending = result.map_err(|f| f.into_parts().1)?;
                    pending.cancel(&mut s.heap, &mut s.account)?;
                }
                Ok(())
            },
        )?;
        let receipts = receipts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let phase = |receipt: &&Receipt| receipt.name == "nrz.work.phase";
        assert_eq!(
            receipts.iter().filter(phase).count(),
            if short { 1 } else { 2 }
        );
        let loan = receipts
            .iter()
            .find(|receipt| receipt.name == "nrz.loan")
            .expect("admission span");
        let mut phases = receipts.iter().filter(phase);
        let first = phases.next().expect("admission Work receipt");
        assert_eq!(first.operation, "LC pending admission");
        assert_eq!(first.outcome, if short { "refused" } else { "completed" });
        assert_ne!(first.ledger, 0);
        assert_eq!(first.work.first(), Some(&before));
        assert_eq!(
            first.work.last(),
            Some(&(before + if short { 371 } else { 1_039 }))
        );
        if !short {
            let retirement = receipts
                .iter()
                .find(|receipt| receipt.name == "nrz.retire")
                .expect("cancellation span");
            assert_ne!(loan.application, 0);
            assert_eq!(retirement.application, loan.application);
            let last = phases.next().expect("cancellation Work receipt");
            assert_eq!(last.operation, "LC pending cancellation");
            assert_eq!(last.outcome, "completed");
            assert_eq!(last.ledger, first.ledger);
            assert!(last.work.iter().all(|work| *work == before + 1_039));
        }
        drop(receipts);
    }
    Ok(())
}

/// The original ledger distinguishes the six-visit abandonment tariff from the outer scan slot.
fn abandonment<P: Scalar + 'static>(short: bool) -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        0,
        0,
    )?;
    let prepared = output
        .prepare(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &mut s.account,
        )
        .map_err(|f| f.into_parts().1)?;
    let selected = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
    let pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    let ticket = pending.ticket();
    let cursor = super::scan_prefix(&mut s.heap, &mut s.account, ticket.identity.root.slot, None)?;
    let remaining = 7 - u64::from(short);
    let consume = s.account.limits().work - s.account.usage().work - remaining;
    s.account.ledger.work(consume, crate::loan::AT)?;
    let before = s.account.usage().work;
    drop(pending);
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let outcome = crate::observer_dispatch::with_observer(
        Observer(Arc::clone(&receipts), Mutex::new(Vec::new())),
        || s.heap.reconcile_detached(&mut s.account, 1, Some(cursor)),
    );
    if short {
        assert!(matches!(outcome, crate::ScanOutcome::WorkExhausted(_)));
        assert_eq!(s.account.usage().work - before, 1);
        assert!(matches!(
            s.heap.loans.group(ticket.identity)?.state,
            crate::loan::registry::State::LcOut(_)
        ));
    } else {
        assert!(matches!(outcome, crate::ScanOutcome::Pending(_)));
        assert_eq!(s.account.usage().work - before, 7);
        assert!(matches!(
            s.heap.loans.group(ticket.identity)?.state,
            crate::loan::registry::State::Retiring
        ));
    }
    let receipts = receipts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let retire = receipts
        .iter()
        .find(|receipt| receipt.name == "nrz.retire")
        .expect("abandonment span");
    assert_eq!(retire.reason, "abandoned");
    assert_eq!(retire.application, ticket.identity.request);
    let phase = receipts
        .iter()
        .find(|receipt| receipt.name == "nrz.work.phase")
        .expect("reconciliation receipt");
    assert_eq!(phase.operation, "detached reconciliation");
    assert_eq!(phase.outcome, if short { "refused" } else { "completed" });
    assert_ne!(phase.ledger, 0);
    assert_eq!(phase.work.first(), Some(&before));
    assert_eq!(phase.work.last(), Some(&s.account.usage().work));
    drop(receipts);
    Ok(())
}

/// Actual span parentage joins every selected attempt to its application, without cell Work phases.
pub(in crate::loan::lc_shape::tests::output) fn traversal<T>(
    expected: [u64; 5],
    run: impl FnOnce() -> T,
) -> T {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let [application, before, readiness, evaluation, demanded] = expected;
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let result = crate::observer_dispatch::with_observer(
        Observer(Arc::clone(&receipts), Mutex::new(Vec::new())),
        run,
    );
    let receipts = receipts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let phases: Vec<_> = receipts
        .iter()
        .filter(|r| r.name == "nrz.work.phase")
        .collect();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].operation, "LC readiness");
    assert_eq!(phases[1].operation, "LC evaluation");
    assert_eq!(phases[0].ledger, phases[1].ledger);
    assert_ne!(phases[0].ledger, 0);
    assert_eq!(phases[0].work.first(), Some(&before));
    assert_eq!(phases[0].work.last(), Some(&(before + readiness)));
    assert_eq!(phases[1].work.first(), Some(&(before + readiness)));
    assert_eq!(
        phases[1].work.last(),
        Some(&(before + readiness + evaluation))
    );
    assert!(phases.iter().all(|phase| phase.outcome == "completed"));
    let (walk, span) = receipts
        .iter()
        .enumerate()
        .find(|(_, r)| r.name == "nrz.walk")
        .expect("application evaluation span");
    assert_eq!(span.application, application);
    let barriers: Vec<_> = receipts
        .iter()
        .filter(|r| r.name == "nrz.barrier")
        .collect();
    assert_eq!(
        barriers.len(),
        usize::try_from(demanded).expect("bounded demand")
    );
    for barrier in barriers {
        assert_eq!(
            barrier.parent,
            Some(u64::try_from(walk + 1).expect("span index"))
        );
        assert_eq!(barrier.consumer, "origin-traversal");
        assert!(
            barrier
                .work
                .iter()
                .all(|work| *work == before + readiness + evaluation)
        );
        assert_eq!(barrier.outcome, "completed");
    }
    drop(receipts);
    result
}

/// The publication receipt joins attempts through their application, without enumerating sign keys.
pub(in crate::loan::lc_shape::tests::output) fn publication<T>(
    expected: [u64; 6],
    masks: [crate::MaskVersion; 2],
    ticket: crate::LoanTicket,
    before: u64,
    run: impl FnOnce() -> T,
) -> T {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let result = crate::observer_dispatch::with_observer(
        Observer(Arc::clone(&receipts), Mutex::new(Vec::new())),
        run,
    );
    let receipts = receipts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let phases: Vec<_> = receipts
        .iter()
        .filter(|r| r.name == "nrz.work.phase")
        .collect();
    assert_eq!(phases.len(), 3);
    let mut work = before;
    for (phase, (operation, tariff)) in phases.iter().zip([
        ("LC readiness", 249),
        ("LC evaluation", 187),
        ("LC publication", 639),
    ]) {
        assert_eq!(phase.operation, operation);
        assert_eq!(phase.ledger, phases[0].ledger);
        assert_ne!(phase.ledger, 0);
        assert_eq!(phase.work.first(), Some(&work));
        work += tariff;
        assert_eq!(phase.work.last(), Some(&work));
        assert_eq!(phase.outcome, "completed");
    }
    let (walk_index, walk) = receipts
        .iter()
        .enumerate()
        .find(|(_, r)| r.name == "nrz.walk")
        .expect("evaluation walk");
    assert_eq!(walk.application, expected[0]);
    let publish: Vec<_> = receipts
        .iter()
        .filter(|r| r.name == "nrz.publish")
        .collect();
    assert_eq!(publish.len(), 1);
    let publish = publish[0];
    assert_eq!(
        [
            publish.application,
            publish.join,
            publish.correspondence,
            publish.query,
            publish.source_closure,
            publish.output_declaration
        ],
        expected
    );
    assert_ne!(publish.output_declaration, 0);
    assert_eq!(publish.sign_attempts_application, walk.application);
    assert_eq!(publish.seal_application, publish.application);
    assert_ne!(publish.outcome_records, 0);
    assert_ne!(publish.terminal_revision, 0);
    assert!(publish.delivered);
    assert_eq!(publish.demand, format!("{:?}", masks[0]));
    assert_eq!(publish.completed_mask, format!("{:?}", masks[1]));
    assert_eq!(publish.terminal_ticket, format!("{ticket:?}"));
    let barriers: Vec<_> = receipts
        .iter()
        .filter(|r| r.name == "nrz.barrier")
        .collect();
    assert_eq!(barriers.len(), 3);
    for (index, barrier) in barriers.iter().enumerate() {
        assert_ne!(barrier.attempt, 0);
        assert!(
            barriers[..index]
                .iter()
                .all(|other| other.attempt != barrier.attempt)
        );
        assert_eq!(
            barrier.parent,
            Some(u64::try_from(walk_index + 1).expect("bounded span"))
        );
        assert_eq!(barrier.consumer, "origin-traversal");
    }
    drop(receipts);
    result
}
