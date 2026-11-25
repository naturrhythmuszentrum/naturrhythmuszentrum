// SPDX-License-Identifier: AGPL-3.0-only
//! Endpoint conversion phases expose their prepaid native Work.

use super::{TestResult, publish};
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Each observation uses one synchronous dispatcher.
static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Record the explicit Work operation names.
#[derive(Default)]
struct Operations(Vec<String>);
impl Visit for Operations {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "operation" {
            self.0.push(value.to_owned());
        }
    }
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

/// Native evaluation creates two Work spans; IDs retain their creation order.
struct Observer(Arc<Mutex<Operations>>);
impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "nrz::work" || metadata.name() == "nrz.kernel"
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut operations = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        attributes.record(&mut *operations);
        Id::from_u64(u64::try_from(operations.0.len()).unwrap_or(u64::MAX).max(1))
    }
    fn record(&self, _id: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn event(&self, _event: &Event<'_>) {}
    fn enter(&self, _id: &Id) {}
    fn exit(&self, _id: &Id) {}
}

/// Binding and conversion record distinct phases without changing their literal Work.
/// ´test:unit:loan-endpoint-view-work-phases´
#[test]
fn loan_endpoint_view_work_phases() -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (heap, mut account, tile, mask) = publish(&[[1.0_f64; 3]])?;
    let source = tile.reader(&heap, &mut account)?;
    let selected = source.masked_view(heap.mask(mask, mask.layout())?, &mut account)?;
    let operations = Arc::new(Mutex::new(Operations::default()));
    let before = account.usage().work;
    crate::observer_dispatch::with_observer(
        Observer(Arc::clone(&operations)),
        || -> TestResult {
            let reader = selected.into_endpoint_view(&mut account)?;
            super::oracle(&reader.read_component(0, 0, &mut account)?, [1.0; 3])
        },
    )?;
    assert_eq!(account.usage().work - before, 38);
    let operations = operations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        operations.0,
        ["endpoint view binding", "endpoint view conversion"]
    );
    Ok(())
}
