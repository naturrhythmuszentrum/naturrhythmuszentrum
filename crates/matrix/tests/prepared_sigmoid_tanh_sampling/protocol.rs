// SPDX-License-Identifier: AGPL-3.0-only
//! Sampling declarations cannot export arbitrary private binding strings.
use super::*;
#[path = "process.rs"]
mod process;
use std::sync::Mutex;
/// Every subscriber replacement in this executable shares one guard.
static DISPATCHERS: Mutex<()> = Mutex::new(());
/// Public digest placeholders are syntactically valid but make no provenance claim.
fn arguments() -> Vec<String> {
    vec![
        "4".into(),
        "disabled".into(),
        "1".into(),
        "1".into(),
        "5".into(),
        "1".into(),
        "a".repeat(64),
        "0".into(),
        "2026-11-13".into(),
        "b".repeat(40),
        "c".repeat(64),
        "uncertified-sigmoid".into(),
    ]
}
/// The shared process carrier checks exact affinity, warm-up separation and five-process declarations.
/// Working-area collisions, bounded retries, early-return cleanup and bounded waits are controlled.
/// # Errors
/// Returns a valid-fixture declaration error.
/// # Panics
/// Panics if missing provenance or an arbitrary printable binding is admitted.
/// ´claim:matrix:prepared-sigmoid-tanh-sampling-protocol´
/// ´test:integration:prepared-sigmoid-tanh-sampling-protocol´
#[test]
fn prepared_sigmoid_tanh_sampling_protocol() -> TestResult {
    process::controls()?;
    let args = arguments();
    let invocation = Invocation::parse(&args)?;
    invocation.protocol.check_affinity("Cpus_allowed_list: 1")?;
    for status in [
        "",
        "Cpus_allowed_list: 0",
        "Cpus_allowed_list: 2",
        "Cpus_allowed_list: 1-2",
        "Cpus_allowed_list: 1,2",
    ] {
        assert!(invocation.protocol.check_affinity(status).is_err());
    }
    for (index, value) in [
        (0, "0"),
        (0, "65537"),
        (1, "unknown"),
        (7, "2"),
        (2, "0"),
        (2, "+1"),
        (3, "6"),
        (4, "4"),
        (5, "0"),
        (6, "private-build"),
        (8, "2026-02-29"),
        (9, "private-revision"),
        (10, "/private/target/binary"),
        (11, "private-command"),
    ] {
        let mut bad = args.clone();
        bad[index] = value.into();
        assert!(Invocation::parse(&bad).is_err(), "parameter {index}");
    }
    for n in 0..args.len() {
        assert!(Invocation::parse(&args[..n]).is_err());
    }
    let mut warm = args;
    warm[3] = "0".into();
    warm[5] = "0".into();
    let warm = Invocation::parse(&warm)?;
    assert!(!warm.protocol.measured());
    assert!(invocation.protocol.measured());
    assert!(
        invocation
            .protocol
            .command
            .starts_with("taskset -c 1 release/examples/prepared_sigmoid_tanh_sampling ")
    );
    assert!(!invocation.protocol.source(None).contains("private-build"));
    Ok(())
}
/// A real blocked child is killed and reaped before its working area is removed.
/// # Errors
/// Returns working-area, child launch or readiness failures.
/// # Panics
/// Panics if kill, reap or directory lifetime assertions fail.
/// ´claim:matrix:prepared-sigmoid-tanh-sampling-child-cleanup´
/// ´test:integration:prepared-sigmoid-tanh-sampling-child-cleanup´
#[test]
fn prepared_sigmoid_tanh_sampling_child_cleanup() -> TestResult {
    process::blocking_child()
}
/// Complete JSON retains all sampler fields; comparable execution suppresses an outer subscriber.
/// # Errors
/// Returns preparation, sampling, serialization or guard errors.
/// # Panics
/// Panics if output omits evidence, tracing leaks into comparable execution or trace loss claims success.
/// ´claim:matrix:prepared-sigmoid-tanh-sampling-output´
/// ´test:integration:prepared-sigmoid-tanh-sampling-output´
#[test]
fn prepared_sigmoid_tanh_sampling_output() -> TestResult {
    if process::sentinel_child()? {
        return Ok(());
    }
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let invocation = Invocation::parse(&arguments())?;
    let mut output = Vec::new();
    let clock = Clock::default();
    let (result, outer) =
        trace::observed(8192, || write_run(&invocation, 8192, &mut output, &clock));
    assert!(
        result.is_ok(),
        "full deterministic collection: {result:?}; {}",
        String::from_utf8_lossy(&output)
    );
    assert!(
        outer.ledgers.is_empty() && outer.clocks.is_empty(),
        "comparable sampling disables even an outer subscriber"
    );
    let text = String::from_utf8(output)?;
    for key in [
        "settings",
        "analysis",
        "seed",
        "resamples",
        "mean",
        "slope",
        "confidence_interval",
        "items_per_second",
        "elements_per_second",
        "iterations",
        "outcome",
        "stop",
        "elements_per_item",
        "executable_sha256",
        "command",
        "date",
        "commit",
    ] {
        assert!(
            text.contains(&format!("\"{key}\":")),
            "complete field {key}"
        );
    }
    assert!(text.contains("\"collection_complete\":true"));
    assert!(text.contains("within_process_intervals=bootstrap"));
    assert!(text.contains("platform-no-engine-work"));
    for sentinel in [
        "sentinel-working-area",
        "sentinel-environment-value",
        "sentinel-account-value",
        "sentinel-login-value",
    ] {
        assert!(
            !text.contains(sentinel),
            "writer excludes ambient sentinel {sentinel}"
        );
    }
    for token in
        text.split(|c: char| c.is_whitespace() || matches!(c, '\"' | '\'' | '=' | ':' | ','))
    {
        assert!(!token.starts_with('/'), "writer excludes absolute paths");
    }
    let mut args = arguments();
    args[1] = "enabled".into();
    let invocation = Invocation::parse(&args)?;
    let mut enabled = Vec::new();
    write_run(&invocation, 8192, &mut enabled, &Clock::default())?;
    assert!(String::from_utf8(enabled)?.contains("\"collection_complete\":true"));
    let mut failed = Vec::new();
    assert!(
        write_run(&invocation, 0, &mut failed, &clock).is_err(),
        "trace loss is a collection failure"
    );
    assert!(String::from_utf8(failed)?.contains("\"collection_complete\":false"));
    let invocation = Invocation::parse(&arguments())?;
    let mut stalled = Vec::new();
    assert!(
        write_run(&invocation, 8192, &mut stalled, &Frozen).is_err(),
        "clock resolution loss is not success"
    );
    let stalled = String::from_utf8(stalled)?;
    assert!(stalled.contains("\"collection_complete\":false"));
    assert!(
        stalled.contains("ClockResolution"),
        "unresolved timing keeps its stop reason"
    );
    Ok(())
}

/// Monotonic deterministic collection intervals, independent of machine speed.
#[derive(Default)]
struct Clock {
    /// Time outside resampling analysis.
    ticks: std::cell::Cell<u64>,
    /// Analysis checkpoints need no synthetic wall cost.
    analysis: std::cell::Cell<bool>,
}
impl SamplingClock for Clock {
    fn now(&self) -> std::time::Duration {
        let value = self.ticks.get();
        self.ticks.set(value + u64::from(!self.analysis.get()));
        std::time::Duration::from_millis(value)
    }
    fn checkpoint(&self, phase: nrz_matrix::sampling::SamplingPhase) {
        self.analysis.set(matches!(
            phase,
            nrz_matrix::sampling::SamplingPhase::Analysis(_)
        ));
    }
}

/// A clock unable to resolve the timed operation supplies no elapsed estimate.
struct Frozen;
impl SamplingClock for Frozen {
    fn now(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}
