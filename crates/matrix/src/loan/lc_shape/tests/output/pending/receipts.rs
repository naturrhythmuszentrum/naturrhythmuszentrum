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
    application: u64,
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
            _ => {}
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "ledger" => self.ledger = value,
            "application" => self.application = value,
            "work" => self.work.push(value),
            _ => {}
        }
    }
    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}
struct Observer(Arc<Mutex<Vec<Receipt>>>);
impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        matches!(
            metadata.name(),
            "nrz.work.phase" | "nrz.loan" | "nrz.retire"
        )
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut receipts = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut receipt = Receipt {
            name: attributes.metadata().name(),
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
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
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
        tracing::subscriber::with_default(Observer(Arc::clone(&receipts)), || -> TestResult {
            let result = prepared.admit(&mut s.heap, selected, &mut s.account);
            if short {
                assert!(result.is_err());
            } else {
                let mut pending = result.map_err(|f| f.into_parts().1)?;
                pending.cancel(&mut s.heap, &mut s.account)?;
            }
            Ok(())
        })?;
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
    let outcome = tracing::subscriber::with_default(Observer(Arc::clone(&receipts)), || {
        s.heap.reconcile_detached(&mut s.account, 1, Some(cursor))
    });
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
