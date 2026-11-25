// SPDX-License-Identifier: AGPL-3.0-only
//! A scoped subscriber retains ordinary production spans and their joined events.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Captured structured fields with the original entered parent and Work interval.
#[derive(Default, Debug)]
pub(super) struct Entry {
    /// Fixed production vocabulary.
    pub(super) name: String,
    /// Actual field values, including opaque attachment diagnostics.
    pub(super) fields: BTreeMap<String, String>,
    /// Entered parent, resolved by the subscriber rather than inferred from event order.
    pub(super) parent: Option<u64>,
    /// First and final cumulative Work observations.
    pub(super) work: Option<[u64; 2]>,
}
impl Visit for Entry {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "work" {
            self.work.get_or_insert([value; 2])[1] = value;
        }
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}
/// One run owns its entries; production code never writes this test storage directly.
#[derive(Default, Debug)]
pub(super) struct Records {
    /// Index plus one is a subscriber span identifier.
    pub(super) spans: Vec<Entry>,
    /// Ordered production events.
    pub(super) events: Vec<Entry>,
    /// Synchronous entered stack, including nested Work phases.
    stack: Vec<u64>,
}
/// A current-dispatcher toggle tests observation without changing production calls.
struct Observer {
    /// All observation is disabled when false.
    enabled: bool,
    /// Test-owned bounded-by-fixture storage.
    records: Arc<Mutex<Records>>,
}
impl Observer {
    /// Poisoning cannot turn a tracing callback into a second panic.
    fn records(&self) -> std::sync::MutexGuard<'_, Records> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.enabled && matches!(metadata.target(), "nrz::lc" | "nrz::work")
    }
    fn register_callsite(
        &self,
        _metadata: &'static Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut records = self.records();
        let mut entry = Entry {
            name: attributes.metadata().name().to_owned(),
            parent: attributes
                .parent()
                .map(Id::into_u64)
                .or_else(|| records.stack.last().copied()),
            ..Entry::default()
        };
        attributes.record(&mut entry);
        records.spans.push(entry);
        Id::from_u64(
            u64::try_from(records.spans.len())
                .unwrap_or(u64::MAX)
                .max(1),
        )
    }
    fn record(&self, id: &Id, values: &Record<'_>) {
        if let Some(entry) = self
            .records()
            .spans
            .get_mut(usize::try_from(id.into_u64() - 1).unwrap_or(usize::MAX))
        {
            values.record(entry);
        }
    }
    fn event(&self, event: &Event<'_>) {
        let mut records = self.records();
        let mut entry = Entry {
            name: event.metadata().name().to_owned(),
            parent: event
                .parent()
                .map(Id::into_u64)
                .or_else(|| records.stack.last().copied()),
            ..Entry::default()
        };
        event.record(&mut entry);
        records.events.push(entry);
    }
    fn record_follows_from(&self, _id: &Id, _follows: &Id) {}
    fn enter(&self, id: &Id) {
        self.records().stack.push(id.into_u64());
    }
    fn exit(&self, id: &Id) {
        assert_eq!(self.records().stack.pop(), Some(id.into_u64()));
    }
}
/// Run the unchanged production path under one scoped current dispatcher.
pub(super) fn measured<T>(enabled: bool, run: impl FnOnce() -> T) -> (T, Arc<Mutex<Records>>) {
    let records = Arc::new(Mutex::new(Records::default()));
    let observer = Observer {
        enabled,
        records: Arc::clone(&records),
    };
    (
        crate::observer_dispatch::with_observer(observer, run),
        records,
    )
}
