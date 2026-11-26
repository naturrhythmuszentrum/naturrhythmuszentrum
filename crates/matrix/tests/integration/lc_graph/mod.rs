// SPDX-License-Identifier: AGPL-3.0-only
//! Public F controls preserve original authority, native routes and separated residence.
mod drive;
mod fixture;
mod flush;
mod residence;

/// ´test:integration:lc-omega-graph-drive´
#[test]
fn lc_omega_graph_drive() -> fixture::Result {
    drive::run::<f32>()?;
    drive::run::<f64>()
}
/// ´test:integration:lc-omega-graph-scoped-flush´
#[test]
fn lc_omega_graph_scoped_flush() -> fixture::Result {
    flush::run::<f32>(f32::NAN)?;
    flush::run::<f64>(f64::NAN)
}
/// ´test:integration:lc-omega-graph-bounded-residence´
#[test]
fn lc_omega_graph_bounded_residence() -> fixture::Result {
    residence::run::<f32>()?;
    residence::run::<f64>()
}

mod renewal;

mod originals;

mod coverage;

mod stopped;

mod receipts;

mod cancel;

mod history;

mod capacity;
mod fanout;

mod preparation;
