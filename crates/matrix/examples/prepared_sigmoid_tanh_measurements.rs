// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone sigmoid and tanh phases; live JSON uses the engineering record fields.
#[path = "prepared_sigmoid_tanh/phases.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "helpers remain private to the executable and its nested integration target"
    )
)]
mod phases;
#[path = "prepared_sigmoid_tanh/protocol.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "helpers remain private to the executable and its nested integration target"
    )
)]
mod protocol;
#[path = "prepared_sigmoid_tanh/support.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "helpers remain private to the executable and its nested integration target"
    )
)]
mod support;
#[path = "prepared_exponential/trace.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the existing bounded observer remains executable-private"
)]
mod trace;
use std::{
    cell::Cell,
    io::{self, Write},
    rc::Rc,
    time::Instant,
};
use support::TestResult;
use support::{Snapshot, inputs};

/// One private live observation, rendered without extending the recorded schema.
#[derive(Debug, PartialEq)]
struct Observation {
    /// Original operation.
    family: &'static str,
    /// Completion boundary.
    phase: &'static str,
    /// Monotonic elapsed seconds.
    seconds: f64,
    /// Caller-ledger delta or explicit platform non-participation.
    work: Option<u64>,
    /// Complete resource snapshots and boundary description.
    evidence: String,
    /// Complete numerical outputs in input order.
    results: Vec<Snapshot>,
}
impl Observation {
    /// Start a row before attaching its optional numerical outputs.
    const fn new(
        family: &'static str,
        phase: &'static str,
        seconds: f64,
        work: Option<u64>,
        evidence: String,
    ) -> Self {
        Self {
            family,
            phase,
            seconds,
            work,
            evidence,
            results: Vec::new(),
        }
    }
}
/// Own the actual value until the measured release, not a detached lifetime token.
#[derive(Debug)]
struct Tracked<T> {
    /// Present until destruction.
    value: Option<T>,
    /// Shared finite owner/output population.
    live: Rc<Cell<u32>>,
}
impl<T> Tracked<T> {
    /// Attach the lifetime counter to an already materialized value.
    fn new(value: T, live: &Rc<Cell<u32>>) -> Self {
        live.set(live.get() + 1);
        Self {
            value: Some(value),
            live: Rc::clone(live),
        }
    }
    /// Borrow while the actual value remains live.
    fn get(&self) -> TestResult<&T> {
        self.value.as_ref().ok_or_else(|| "released value".into())
    }
}
impl<T> Drop for Tracked<T> {
    fn drop(&mut self) {
        drop(self.value.take());
        self.live.set(self.live.get() - 1);
        tracing::event!(name:"nrz.measure.counter",target:"nrz::work",tracing::Level::TRACE,family="",operation="release",count=1_u64);
    }
}
/// One stopwatch whose actual readings are visible to the private test observer.
#[derive(Debug)]
struct Clock {
    /// Boundary being measured.
    phase: &'static str,
    /// Actual monotonic start reading.
    start: Instant,
}
impl Clock {
    /// Observe the start adjacent to the actual clock reading.
    fn start(phase: &'static str) -> Self {
        checkpoint(phase, "start", 0);
        Self {
            phase,
            start: Instant::now(),
        }
    }
    /// Observe the stop adjacent to the actual clock reading.
    fn stop(&self) -> f64 {
        checkpoint(self.phase, "stop", 0);
        self.start.elapsed().as_secs_f64()
    }
}
/// Private observer event, with no measured duration.
#[cfg(test)]
type BoundaryEvent = (&'static str, &'static str, usize);
#[cfg(test)]
std::thread_local! {
    /// Test-only callbacks retain ordering, never measured durations.
    static BOUNDARIES: std::cell::RefCell<Option<Vec<BoundaryEvent>>> = const { std::cell::RefCell::new(None) };
}
/// Setup and completion checkpoints carry actual population or ownership state.
#[cfg(test)]
fn checkpoint(phase: &'static str, event: &'static str, count: usize) {
    BOUNDARIES.with_borrow_mut(|events| {
        if let Some(events) = events {
            events.push((phase, event, count));
        }
    });
}
/// Production clocks retain no test observations.
#[cfg(not(test))]
const fn checkpoint(_phase: &'static str, _event: &'static str, _count: usize) {}
/// Each clock includes its completed operation and consumption, excluding JSON export.
fn timed<T>(phase: &'static str, run: impl FnOnce() -> TestResult<T>) -> TestResult<(T, f64)> {
    let clock = Clock::start(phase);
    let result = {
        let _span = tracing::trace_span!("nrz.measure", family = phase, revision = 1_u64).entered();
        run()
    }?;
    Ok((result, clock.stop()))
}
/// Collection loss is an explicit failure; it cannot change a numerical result.
fn run(
    count: u32,
    enabled: bool,
    capacity: usize,
) -> TestResult<(Vec<Observation>, Option<trace::Report>)> {
    let values = inputs(count)?;
    let measure = || -> TestResult<Vec<Observation>> {
        let mut rows = phases::sigmoid(&values)?;
        rows.extend(phases::tanh(&values)?);
        Ok(rows)
    };
    if enabled {
        let (rows, receipt) = trace::observed(capacity, measure);
        if !receipt.complete || receipt.observer.capacity != capacity {
            return Err("collection failure: incomplete tracing receipt".into());
        }
        if receipt
            .boundary_work
            .iter()
            .any(|((name, _), w)| name.starts_with("native-") && *w != 0)
        {
            return Err("collection failure: native exact Work".into());
        }
        Ok((rows?, Some(receipt)))
    } else {
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            Ok((measure()?, None))
        })
    }
}
/// JSON string escaping follows the existing sampler's explicit serializer.
fn string(out: &mut impl Write, text: &str) -> io::Result<()> {
    write!(out, "\"")?;
    for c in text.chars() {
        match c {
            '"' => write!(out, "\\\"")?,
            '\\' => write!(out, "\\\\")?,
            '\n' => write!(out, "\\n")?,
            '\r' => write!(out, "\\r")?,
            '\t' => write!(out, "\\t")?,
            c if c.is_control() => write!(out, "\\u{:04x}", u32::from(c))?,
            c => write!(out, "{c}")?,
        }
    }
    write!(out, "\"")
}
/// The eight existing engineering fields carry live observations, not committed rows.
fn record(
    out: &mut impl Write,
    p: &protocol::Protocol,
    id: &str,
    metric: &str,
    value: &str,
    unit: &str,
    source: &str,
) -> io::Result<()> {
    write!(out, "{{")?;
    for (i, (key, text)) in [
        ("id", id),
        ("metric", metric),
        ("value", value),
        ("unit", unit),
        ("date", p.date.as_str()),
        ("commit", p.commit.as_str()),
        ("command", p.command.as_str()),
        ("source", source),
    ]
    .into_iter()
    .enumerate()
    {
        if i != 0 {
            write!(out, ",")?;
        }
        string(out, key)?;
        write!(out, ":")?;
        string(out, text)?;
    }
    writeln!(out, "}}")
}
/// One fresh process retains its full protocol on every emitted row.
fn write_run(p: &protocol::Protocol, capacity: usize, mut out: impl Write) -> TestResult {
    match run(p.count, p.enabled, capacity) {
        Ok((rows, receipt)) => {
            if !p.measured() {
                record(
                    &mut out,
                    p,
                    "warm-up",
                    "discarded warm-up process",
                    "1",
                    "process",
                    &p.source(None),
                )?;
                out.flush()?;
                return Ok(());
            }
            for row in rows {
                let id = format!("{}-{}", row.family, row.phase);
                let source = format!(
                    "{}; boundary={}; work={:?}; resources={}; complete_results={:?}; observer={receipt:?}",
                    p.source(None),
                    row.phase,
                    row.work,
                    row.evidence,
                    row.results
                );
                record(
                    &mut out,
                    p,
                    &id,
                    row.phase,
                    &format!("{:.9}", row.seconds),
                    "s",
                    &source,
                )?;
            }
        }
        Err(error) => {
            record(
                &mut out,
                p,
                "collection-failure",
                "collection failed",
                "1",
                "failure",
                &format!("{}; error={error}", p.source(None)),
            )?;
            out.flush()?;
            return Err(error);
        }
    }
    out.flush()?;
    Ok(())
}

/// Validate process provenance before starting any phase.
fn main() -> TestResult {
    let p = protocol::Protocol::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    p.check_affinity(&std::fs::read_to_string("/proc/self/status")?)?;
    write_run(&p, 4096, io::BufWriter::new(io::stdout().lock()))
}

#[cfg(test)]
#[path = "../tests/prepared_sigmoid_tanh_measurements/controls.rs"]
mod tests;

#[cfg(test)]
#[path = "prepared_sigmoid_tanh/sampling.rs"]
mod sampling;
#[cfg(test)]
#[path = "../tests/prepared_sigmoid_tanh_measurements/sampling_controls.rs"]
mod sampling_controls;
#[cfg(test)]
#[path = "prepared_sigmoid_tanh/setup.rs"]
mod setup;
