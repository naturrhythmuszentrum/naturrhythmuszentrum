// SPDX-License-Identifier: AGPL-3.0-only
//! The library owns the shared observer dispatcher's registration regression.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tracing::{
    Event, Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

#[derive(Clone, Default)]
struct Counting(Arc<AtomicU64>);

impl Subscriber for Counting {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(self.0.fetch_add(1, Ordering::Relaxed) + 1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

fn emit() {
    let _span = tracing::trace_span!("observer-callsite-registration").entered();
}

/// A callsite first used on a thread without the observer stays visible to the observer.
/// Without the helper's second dispatcher, this fails only when run alone,
/// before another test has created a dispatcher in this process.
///
/// # Panics
///
/// Panics if the worker thread fails or scoped registration loses the span.
/// ´test:unit:observer-callsite-registered-on-another-thread´
#[test]
fn observes_callsite_registered_on_another_thread() {
    let observer = Counting::default();
    crate::observer_dispatch::with_observer(observer.clone(), || {
        std::thread::spawn(emit).join().expect("first registration");
        emit();
    });
    assert_eq!(observer.0.load(Ordering::Relaxed), 1);
}
