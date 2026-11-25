// SPDX-License-Identifier: AGPL-3.0-only
//! A field-visiting subscriber records attempted retained phases and exact Work events.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// Shared local subscriber state for one selected observation control.
#[derive(Clone, Default)]
pub(super) struct OwnerObserver {
    /// Next span identity.
    next: Arc<AtomicU64>,
    /// Assigned span names.
    names: Arc<Mutex<BTreeMap<u64, String>>>,
    /// All visited field records.
    fields: Arc<Mutex<Vec<(u64, String, String)>>>,
    /// Count of exact Work events under this subscriber.
    work: Arc<AtomicU64>,
}

/// One field visitor writes into the observer's shared buffer.
struct OwnerFields {
    /// Span whose fields are being visited.
    id: u64,
    /// Shared record buffer.
    fields: Arc<Mutex<Vec<(u64, String, String)>>>,
}

impl tracing::field::Visit for OwnerFields {
    /// Retain the exact debug representation of each recorded field.
    /// # Panics
    /// Panics only if this local test's field lock is poisoned.
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.lock().expect("observer field lock").push((
            self.id,
            field.name().to_owned(),
            format!("{value:?}"),
        ));
    }
}

impl tracing::Subscriber for OwnerObserver {
    /// Recheck interest for each span while other scoped subscribers run in parallel.
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }

    /// Enable all local phases and exact Work events.
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }

    /// Assign one identity and visit each span's initial fields.
    /// # Panics
    /// Panics only if this local test's name lock is poisoned.
    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.names
            .lock()
            .expect("observer name lock")
            .insert(id, span.metadata().name().to_owned());
        span.record(&mut OwnerFields {
            id,
            fields: self.fields.clone(),
        });
        tracing::span::Id::from_u64(id)
    }

    /// Visit all later span result and receipt updates.
    fn record(&self, span: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        values.record(&mut OwnerFields {
            id: span.into_u64(),
            fields: self.fields.clone(),
        });
    }

    /// Causal links are outside this local phase control.
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    /// Count only exact Work events.
    fn event(&self, event: &tracing::Event<'_>) {
        if event.metadata().target() == "nrz::work" {
            self.work.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Span entry is not needed to inspect recorded fields.
    fn enter(&self, _: &tracing::span::Id) {}

    /// Span exit is not needed to inspect recorded fields.
    fn exit(&self, _: &tracing::span::Id) {}
}

impl OwnerObserver {
    /// Count exact Work events observed so far.
    pub(super) fn work_events(&self) -> u64 {
        self.work.load(Ordering::Relaxed)
    }

    /// Return all span identifiers with the requested phase name.
    /// # Panics
    /// Panics only if this local test's name lock is poisoned.
    pub(super) fn spans(&self, name: &str) -> Vec<u64> {
        self.names
            .lock()
            .expect("observer name lock")
            .iter()
            .filter(|(_, observed)| observed.as_str() == name)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return the last visited value of one named field on one span.
    /// # Panics
    /// Panics only if this local test's field lock is poisoned.
    pub(super) fn field(&self, id: u64, name: &str) -> Option<String> {
        self.fields
            .lock()
            .expect("observer field lock")
            .iter()
            .rev()
            .find(|(span, key, _)| *span == id && key == name)
            .map(|(_, _, value)| value.clone())
    }
}
