// SPDX-License-Identifier: AGPL-3.0-only
//! Literal projected traversal phase names remain observable at the prepaid Work boundary.

use super::super::{Setup, TestResult, candidate};
use crate::{ApplicationRequest, Elementwise};
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Each observation uses one synchronous dispatcher.
static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Record only operation fields from Work spans.
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
        metadata.target() == "nrz::work"
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

/// Two selected two-component cells cost the design's 78 units with either traversal name.
/// ´test:unit:loan-projected-unary-work-phases´
#[test]
fn loan_projected_unary_work_phases() -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (function, phase) in [
        (Elementwise::Reciprocal, "projected reciprocal traversal"),
        (Elementwise::SquareRoot, "projected square root traversal"),
    ] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            ..
        } = super::super::setup::<f64>()?;
        super::unary::operation(&mut request, function)?;
        let owner = match heap.request_projected_elementwise(request, &mut account) {
            ApplicationRequest::ApplicationReady(owner) => owner,
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let operations = Arc::new(Mutex::new(Operations::default()));
        let before = account.usage().work;
        let owner = tracing::subscriber::with_default(Observer(Arc::clone(&operations)), || {
            candidate(owner, &mut account)
        })?;
        assert_eq!(account.usage().work - before, 78);
        let operations = operations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(operations.0, ["pointwise evaluation", phase]);
        drop(owner);
    }
    Ok(())
}
