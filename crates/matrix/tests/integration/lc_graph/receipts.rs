// SPDX-License-Identifier: AGPL-3.0-only
//! Public graph and native phase receipts retain one previously spent ledger incarnation.
use super::fixture::Result;
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

#[derive(Default)]
pub(super) struct Phase {
    family: String,
    operation: String,
    ledger: u64,
    work: Vec<u64>,
}
impl Visit for Phase {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "family" => self.family = value.into(),
            "operation" => self.operation = value.into(),
            _ => {}
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "ledger" => self.ledger = value,
            "work" => self.work.push(value),
            _ => {}
        }
    }
    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}
struct Observer(Arc<Mutex<Vec<Phase>>>);
impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.name() == "nrz.work.phase"
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut phases = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut phase = Phase::default();
        attributes.record(&mut phase);
        phases.push(phase);
        Id::from_u64(u64::try_from(phases.len()).expect("bounded observer"))
    }
    fn record(&self, id: &Id, values: &Record<'_>) {
        let mut phases = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = usize::try_from(id.into_u64() - 1).expect("bounded observer identifier");
        values.record(&mut phases[index]);
    }
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

pub(super) fn recorded<T>(action: impl FnOnce() -> T) -> (T, Vec<Phase>) {
    let phases = Arc::new(Mutex::new(Vec::new()));
    let result = crate::observer_dispatch::with_observer(Observer(Arc::clone(&phases)), action);
    let records = std::mem::take(
        &mut *phases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    (result, records)
}

/// Snapshot endpoints are original cumulative Work, not reconstructed per-phase budgets.
pub(super) fn check(phases: &[Phase], before: u64, after: u64, evaluated: bool) -> Result {
    let selected = phases
        .iter()
        .filter(|phase| matches!(phase.family.as_str(), "matrix.graph" | "matrix.loan"))
        .collect::<Vec<_>>();
    let ledger = selected.first().ok_or("original phase receipt")?.ledger;
    assert_ne!(ledger, 0);
    for phase in &selected {
        assert_eq!(phase.ledger, ledger);
        assert!(phase.work.len() >= 3);
        assert!(phase.work.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(*phase.work.first().ok_or("entry")? >= before);
        assert!(*phase.work.last().ok_or("exit")? <= after);
    }
    for operation in [
        "native phase selection",
        "exact owner reservation",
        "exact owner inventory",
        "native residence partition",
        "loan admission",
    ] {
        assert!(
            selected.iter().any(|phase| phase.operation == operation),
            "{operation}"
        );
    }
    for operation in ["LC pending admission", "LC evaluation", "LC publication"] {
        assert_eq!(
            selected
                .iter()
                .filter(|phase| phase.operation == operation)
                .count(),
            usize::from(evaluated),
            "{operation}"
        );
    }
    Ok(())
}
