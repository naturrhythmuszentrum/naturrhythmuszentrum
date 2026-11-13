// SPDX-License-Identifier: AGPL-3.0-only
//! Six private block adapters, with a public-safe process declaration and full reports.
#[path = "prepared_sigmoid_tanh/protocol.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "shared carrier remains private to each executable"
    )
)]
mod protocol;
#[path = "prepared_sigmoid_tanh/sampling.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "adapters remain private to the executable and its integration target"
    )
)]
#[cfg_attr(
    test,
    expect(
        dead_code,
        reason = "callback inspection is exercised by the measurement integration target"
    )
)]
mod sampling;
#[path = "prepared_exponential/sampling_json.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "shared serializer remains private to each executable"
    )
)]
mod sampling_json;
#[path = "prepared_sigmoid_tanh/setup.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "preparation helpers remain executable-private"
    )
)]
mod setup;
#[path = "prepared_sigmoid_tanh/support.rs"]
#[cfg_attr(
    not(test),
    expect(
        clippy::redundant_pub_crate,
        reason = "shared fixtures remain executable-private"
    )
)]
#[expect(
    dead_code,
    reason = "the shared ordinary snapshot vocabulary includes its platform observation variant"
)]
mod support;
#[path = "prepared_exponential/trace.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the bounded observer stays executable-private"
)]
mod trace;
use nrz_matrix::sampling::{InstantClock, SamplingClock, SamplingStop, sample_with_clock};
use nrz_scalar::ArithmeticContext;
use sampling::{Adapter, Block, Phase, Prepared};
use std::io::{self, Write};
use support::{Fixture, TestResult, inputs, ledger};

/// The executable has one fixed path relative to the build target directory.
const EXECUTABLE: &str = "release/examples/prepared_sigmoid_tanh_sampling";
/// Finite observation storage is separate from every execution ledger.
const OBSERVER_CAPACITY: usize = 8192;
/// Closed phase vocabulary prevents arbitrary command text from reaching output.
const PHASES: [Phase; 6] = [
    Phase::NativeSigmoid,
    Phase::NativeTanh,
    Phase::BatchSigmoid,
    Phase::BatchTanh,
    Phase::PlatformSigmoid,
    Phase::PlatformTanh,
];

/// Only public hex identities, integers and closed names reach the shared carrier.
#[derive(Debug)]
struct Invocation {
    /// Same protocol checks as ordinary measurement.
    protocol: protocol::Protocol,
    /// Selected block adapter, or all six.
    phases: Vec<Phase>,
    /// Runner-supplied executable identity, not a claim of authenticated provenance.
    executable_digest: String,
}
impl Invocation {
    /// Accept a reproducible command without a caller-controlled command or environment cell.
    fn parse(args: &[String]) -> TestResult<Self> {
        if args.len() != 12 {
            return Err("expected count mode cpu ordinal processes warm-up-pid builds build-index date commit executable-sha256 phase".into());
        }
        let phases = if args[11] == "all" {
            PHASES.to_vec()
        } else {
            vec![
                *PHASES
                    .iter()
                    .find(|phase| phase.name() == args[11])
                    .ok_or("phase")?,
            ]
        };
        if !hex(&args[10], 64) || !args[6].split(',').all(|build| hex(build, 64)) {
            return Err("public build and executable identities must be SHA-256 hex".into());
        }
        for index in [0, 2, 3, 4, 5, 7] {
            if args[index].is_empty() || !args[index].bytes().all(|b| b.is_ascii_digit()) {
                return Err("unsigned decimal process parameter".into());
            }
        }
        let command = format!("taskset -c {} {} {}", args[2], EXECUTABLE, args.join(" "));
        let mut carrier = args[..10].to_vec();
        carrier.push(command);
        carrier.push("shared-core dynamic-frequency conditions recorded in the process manifest; instruction counts unavailable; no isolated or fixed-frequency latency claim".into());
        let protocol = protocol::Protocol::parse(&carrier)?;
        inputs(protocol.count)?;
        Ok(Self {
            protocol,
            phases,
            executable_digest: args[10].clone(),
        })
    }
}
/// A digest is a public identifier rather than an arbitrary binding or filesystem path.
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Prepare once, outside collection, then borrow both immutable owners for every block.
fn reports(invocation: &Invocation, clock: &dyn SamplingClock) -> TestResult<(Vec<u8>, bool)> {
    let block = Block::new(inputs(invocation.protocol.count)?);
    let arithmetic = ArithmeticContext::new()?;
    let mut left = Fixture::new()?;
    let mut right = Fixture::new()?;
    let left_request = support::original(&mut left, false)?;
    let right_request = support::original(&mut right, true)?;
    let sigmoid = setup::sigmoid(&mut left, &left_request, &arithmetic, &mut ledger()?)?;
    let tanh = setup::tanh(&mut right, &right_request, &arithmetic, &mut ledger()?)?;
    let prepared = Prepared {
        sigmoid: &sigmoid,
        tanh: &tanh,
        arithmetic: &arithmetic,
    };
    let shape = [block.values.len()];
    let mut out = Vec::new();
    let mut complete = true;
    write!(out, "[")?;
    for (index, phase) in invocation.phases.iter().copied().enumerate() {
        if index != 0 {
            write!(out, ",")?;
        }
        let mut adapter = Adapter {
            phase,
            allowances: sampling::allowances(),
        };
        let mut representative = sampling::representative(&block, phase, &shape);
        representative.identity.trace_configuration = if invocation.protocol.enabled {
            "enabled-noncomparable"
        } else {
            "disabled"
        };
        #[cfg(test)]
        sampling::begin_call();
        let report = sample_with_clock(
            &prepared,
            &representative,
            &mut adapter,
            sampling::settings()?,
            clock,
        )?;
        complete &= *report.stop() == SamplingStop::Complete;
        sampling_json::write(&mut out, &report)?;
    }
    write!(out, "]")?;
    Ok((out, complete))
}
/// Export complete reports even for stopped collection; trace loss is separately explicit.
fn write_run(
    invocation: &Invocation,
    capacity: usize,
    mut out: impl Write,
    clock: &dyn SamplingClock,
) -> TestResult {
    let p = &invocation.protocol;
    let (reports, observer) = if p.enabled {
        let (reports, observer) = trace::observed(capacity, || reports(invocation, clock));
        (reports, Some(observer))
    } else {
        (
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                reports(invocation, clock)
            }),
            None,
        )
    };
    let (reports, sampled) = reports?;
    let complete = sampled
        && observer
            .as_ref()
            .is_none_or(|receipt| receipt.complete && receipt.observer.capacity == capacity);
    let settings = sampling::settings()?;
    let mut source = p.source(Some((
        settings.analysis.seed(),
        settings.analysis.resamples(),
    )));
    source.push_str("; provenance=runner-declared commit, build and executable digests, ordinal, count and warm-up PID; checked for form and consistency only; affinity=observed; command=reconstructed relative to the default release target directory, not observed; executable digest depends on build directories unless paths are remapped; dependency panic diagnostics may expose build paths on stderr");
    // The protocol fields have a closed, pipe-free grammar; use the shared JSON escaper.
    write!(out, "{{\"preparation_sampled\":false,\"source\":")?;
    sampling_json::string(&mut out, &source)?;
    write!(out, ",\"command\":")?;
    sampling_json::string(&mut out, &p.command)?;
    write!(out, ",\"executable\":")?;
    sampling_json::string(&mut out, EXECUTABLE)?;
    write!(out, ",\"executable_sha256\":")?;
    sampling_json::string(&mut out, &invocation.executable_digest)?;
    write!(out, ",\"commit\":")?;
    sampling_json::string(&mut out, &p.commit)?;
    write!(out, ",\"date\":")?;
    sampling_json::string(&mut out, &p.date)?;
    write!(out, ",\"collection_complete\":{complete},\"reports\":")?;
    out.write_all(&reports)?;
    writeln!(out, "}}")?;
    out.flush()?;
    if !complete {
        return Err("collection failure: sampling stopped or tracing receipt incomplete".into());
    }
    Ok(())
}
/// The shared protocol checks the actual affinity before any preparation or sampling.
fn main() -> TestResult {
    let invocation = Invocation::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    invocation
        .protocol
        .check_affinity(&std::fs::read_to_string("/proc/self/status")?)?;
    write_run(
        &invocation,
        OBSERVER_CAPACITY,
        io::BufWriter::new(io::stdout().lock()),
        &InstantClock::default(),
    )
}

#[cfg(test)]
#[path = "../tests/prepared_sigmoid_tanh_sampling/protocol.rs"]
mod tests;
