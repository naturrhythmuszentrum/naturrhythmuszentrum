// SPDX-License-Identifier: AGPL-3.0-only
//! Native controls keep the original declaration and numerical source interfaces in the loop.

mod abandonment;
mod accounting;
mod admission;
mod boundaries;
mod classification;
mod controls;
mod coverage;
mod failures;
mod fanout;
mod fixture;
mod handback;
mod history;
mod lifecycle;
mod originals;
mod pending;
mod renewal;
mod scoped;
mod work;
use fixture::TestResult;

/// Construction and sealed delivery preserve all five original positions in both formats.
/// ´test:unit:graph-native-lifecycle´
#[test]
fn graph_native_lifecycle() -> TestResult {
    lifecycle::published::<f32>(false)?;
    lifecycle::published::<f64>(false)?;
    lifecycle::published::<f32>(true)?;
    lifecycle::published::<f64>(true)
}

/// Interrupted admission, scoped absence and explicit retry preserve the rightful native owners.
/// ´test:unit:graph-native-control-boundaries´
#[test]
fn graph_native_control_boundaries() -> TestResult {
    controls::admission()?;
    controls::waiting()?;
    controls::notification()?;
    controls::exhaustion()
}

/// Additional consumer retention preserves order through successful retry and exhausted attempts.
/// ´test:unit:graph-native-fanout-retry´
#[test]
fn graph_native_fanout_retry() -> TestResult {
    fanout::retry(2)?;
    fanout::retry(1)
}

/// A failed original debit latches the adapter while the genuine host cancellation remains available.
/// ´test:unit:graph-native-work-stop´
#[test]
fn graph_native_work_stop() -> TestResult {
    work::handback()?;
    work::residence()
}

/// Every original position constrains readiness, including the repeated unused inductance.
/// ´test:unit:graph-native-coverage´
#[test]
fn graph_native_coverage() -> TestResult {
    coverage::run::<f32>()?;
    coverage::run::<f64>()?;
    coverage::zero_source::<f32>()?;
    coverage::zero_source::<f64>()
}

/// Admission failure returns identical proposals and closes only acquired allocation prefixes.
/// ´test:unit:graph-native-admission-prefixes´
#[test]
fn graph_native_admission_prefixes() -> TestResult {
    admission::prefixes()
}

/// Fixed vacant capacity and temporary proposal buffers reach their immutable byte ceilings.
/// ´test:unit:graph-native-admission-byte-boundaries´
#[test]
fn graph_native_admission_byte_boundaries() -> TestResult {
    boundaries::run::<f32>()?;
    boundaries::run::<f64>()
}

/// Scoped closure preserves an unowned expectation and completes after its genuine native return.
/// ´test:unit:graph-native-scoped-drain´
#[test]
fn graph_native_scoped_drain() -> TestResult {
    scoped::run::<f32>(false)?;
    scoped::run::<f64>(false)?;
    scoped::run::<f32>(true)?;
    scoped::run::<f64>(true)
}

/// A refused numerical reservation preserves the exact prepared law and explicit cancellation route.
/// ´test:unit:graph-native-pending-retry-cancel´
#[test]
fn graph_native_pending_retry_cancel() -> TestResult {
    pending::run::<f32>(0)?;
    pending::run::<f64>(0)?;
    pending::run::<f32>(2)?;
    pending::run::<f64>(2)
}

/// Surviving bounded history blocks over-limit admission until its final prepaid release.
/// ´test:unit:graph-native-history-residence´
#[test]
fn graph_native_history_residence() -> TestResult {
    history::run::<f32>()?;
    history::run::<f64>()
}

/// Typed replay refusal, cancel-only failure and original candidate repair preserve phase authority.
/// ´test:unit:graph-native-failure-repair´
#[test]
fn graph_native_failure_repair() -> TestResult {
    failures::typed::<f32>()?;
    failures::typed::<f64>()?;
    failures::execution::<f32>()?;
    failures::execution::<f64>()?;
    failures::repair::<f32>(false)?;
    failures::repair::<f64>(false)?;
    failures::repair::<f32>(true)?;
    failures::repair::<f64>(true)
}

/// Scoped closure never substitutes notification for repair or native cancellation.
/// ´test:unit:graph-native-scoped-handback´
#[test]
fn graph_native_scoped_handback() -> TestResult {
    for cancel in [false, true] {
        handback::run(f32::NAN, cancel)?;
        handback::run(f64::NAN, cancel)?;
    }
    Ok(())
}

/// One common lifetime preserves detached host and checked-reader routes after graph abandonment.
/// ´test:unit:graph-native-abandonment-residence´
#[test]
fn graph_native_abandonment_residence() -> TestResult {
    for cancel in [false, true] {
        abandonment::run::<f32>(cancel)?;
        abandonment::run::<f64>(cancel)?;
    }
    Ok(())
}

/// Independent phase tariffs preserve prior spending, exact neighbours and original Work stops.
/// ´test:unit:graph-native-accounting-neighbours´
#[test]
fn graph_native_accounting_neighbours() -> TestResult {
    accounting::exact()?;
    accounting::gate()?;
    accounting::snapshot()
}
