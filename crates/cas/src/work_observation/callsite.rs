// SPDX-License-Identifier: AGPL-3.0-only
//! A non-registering interest load keeps call-making branches outside ordinary helper bodies.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tracing::subscriber::Interest;
use tracing::{Level, Metadata};

/// The tracing registry owns interest updates; unknown and non-never interest share the cold route.
#[derive(Debug)]
pub(super) struct Callsite {
    /// Zero is never-interest; one is unknown/sometimes and two is always-interest.
    interest: AtomicU8,
    /// Start registration exactly once without blocking a concurrent or reentrant boundary.
    registration: AtomicBool,
    /// Filtering and value construction use the same callsite metadata.
    metadata: &'static Metadata<'static>,
}

impl Callsite {
    /// Unregistered sites require the cold route even before any subscriber has been installed.
    pub(super) const fn new(metadata: &'static Metadata<'static>) -> Self {
        Self {
            interest: AtomicU8::new(1),
            registration: AtomicBool::new(false),
            metadata,
        }
    }

    /// This load never registers a callsite or calls a dispatcher.
    #[inline]
    pub(super) fn potentially_enabled(&self) -> bool {
        Level::TRACE <= tracing::level_filters::STATIC_MAX_LEVEL
            && Level::TRACE <= tracing::level_filters::LevelFilter::current()
            && self.interest.load(Ordering::Relaxed) != 0
    }

    /// Wrappers and lifecycle events also bypass cold dispatch on cached never-interest.
    #[inline]
    pub(super) fn enabled(&'static self) -> bool {
        self.potentially_enabled() && self.enabled_cold()
    }

    /// Even always-interest can come from another thread's subscriber.
    #[cold]
    #[inline(never)]
    fn enabled_cold(&'static self) -> bool {
        if self
            .registration
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            // Keep the registry on its multi-dispatcher route while registering. Its sole-dispatcher
            // shortcut otherwise queries this thread even when the sole subscriber lives elsewhere.
            // As with matrix's scoped-observer test rule, an already-running JustOne
            // registration can still finish after this dispatcher is installed.
            let _registration_dispatch =
                tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
            tracing::callsite::register(self);
        }
        tracing::dispatcher::get_default(|dispatch| dispatch.enabled(self.metadata))
    }

    /// Controls distinguish the aggregate interests without changing the production cache.
    #[cfg(test)]
    pub(super) fn interest(&self) -> Interest {
        match self.interest.load(Ordering::Relaxed) {
            0 => Interest::never(),
            2 => Interest::always(),
            _ => Interest::sometimes(),
        }
    }

    /// Expose the exact metadata used by the current dispatcher's enablement decision.
    pub(super) const fn metadata(&self) -> &'static Metadata<'static> {
        self.metadata
    }
}

impl tracing::callsite::Callsite for Callsite {
    fn set_interest(&self, interest: Interest) {
        let value = if interest.is_never() {
            0
        } else if interest.is_always() {
            2
        } else {
            1
        };
        self.interest.store(value, Ordering::SeqCst);
    }

    fn metadata(&self) -> &Metadata<'_> {
        self.metadata
    }
}

/// Keep each field inventory attached to its registered metadata.
macro_rules! callsite {
    ($name:literal, $kind:expr, $($field:literal),+ $(,)?) => {{
        static CALLSITE: Callsite = Callsite::new(&METADATA);
        static METADATA: tracing::Metadata<'static> = tracing::metadata! {
            name: $name,
            target: "nrz::work",
            level: tracing::Level::TRACE,
            fields: &[$($field),+],
            callsite: &CALLSITE,
            kind: $kind,
        };
        &CALLSITE
    }};
}

pub(super) use callsite;
