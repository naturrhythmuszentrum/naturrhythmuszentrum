// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped subscribers with a process-wide fallback for callsite interest.

use std::sync::LazyLock;
use tracing::{Dispatch, Subscriber, subscriber};

static NOOP_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(subscriber::NoSubscriber::new()));

/// Install observers through this helper (´rule:matrix:scoped-observer-tests´).
/// Forcing the second dispatcher before the observer narrows the sole-dispatcher
/// registration window to this process's first installation. A registration
/// already in flight can still cache `never`; see the rule's argument.
#[allow(
    clippy::redundant_pub_crate,
    reason = "sibling tests access this helper through a private module"
)]
pub(super) fn with_observer<T, S>(observer: S, run: impl FnOnce() -> T) -> T
where
    S: Subscriber + Send + Sync + 'static,
{
    LazyLock::force(&NOOP_DISPATCHER);
    subscriber::with_default(observer, run)
}
