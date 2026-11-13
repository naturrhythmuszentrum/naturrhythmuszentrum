// SPDX-License-Identifier: AGPL-3.0-only
//! Phase and process controls use live results, never committed timing values.
use super::*;
use std::sync::Mutex;
/// Serialize dispatch replacement within this executable.
pub(super) static DISPATCHERS: Mutex<()> = Mutex::new(());
/// Complete process declarations; a distinct prior process is supplied as provenance.
fn arguments() -> Vec<String> {
    [
        "4",
        "disabled",
        "1",
        "1",
        "5",
        "1",
        "first,second",
        "0",
        "2026-11-12",
        "0000000000000000000000000000000000000000",
        "example command",
        "frequency and sibling policy recorded by runner",
    ]
    .map(str::to_owned)
    .to_vec()
}
/// Both families retain full ordered outputs in both modes; actual clock readings bracket setup-complete and result/destruction-complete checkpoints for every phase.
/// # Errors
/// Returns numerical, collection or lock errors.
/// # Panics
/// Panics if a phase changes results, rebuilds a warm authority or retains a released value.
/// ´claim:matrix:prepared-sigmoid-tanh-measurement-phases´
/// ´test:integration:prepared-sigmoid-tanh-measurement-phases´
#[test]
fn prepared_sigmoid_tanh_measurement_phases() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    BOUNDARIES.with_borrow_mut(|events| *events = Some(Vec::new()));
    let (mut observed, receipt) = run(4, true, 4096)?;
    let (mut plain, none) = run(4, false, 4096)?;
    let boundaries = BOUNDARIES
        .with_borrow_mut(Option::take)
        .ok_or("boundary capture")?;
    check_boundaries(&boundaries);
    assert!(none.is_none());
    for row in observed.iter_mut().chain(&mut plain) {
        row.seconds = 0.0;
    }
    assert_eq!(
        observed, plain,
        "observation preserves complete snapshots and accounting"
    );
    assert_eq!(observed.len(), 20);
    let receipt = receipt.ok_or("receipt")?;
    assert!(receipt.complete);
    assert_eq!(
        receipt
            .counters
            .get(&("prepare-warm".into(), "cache hit".into())),
        Some(&2),
        "warm phases find their authority"
    );
    for family in ["sigmoid", "tanh"] {
        assert!(
            receipt.clocks.contains_key(&(
                format!("admit-{family}"),
                "nrz.import".into(),
                "prepared-expm1-coefficients".into()
            )),
            "owner admission includes coefficient import"
        );
        assert!(
            receipt.clocks.contains_key(&(
                format!("native-{family}"),
                "nrz.validation".into(),
                format!("prepared-{family}")
            )),
            "native operand validation is observed"
        );
        let rows = observed
            .iter()
            .filter(|r| r.family == family)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 10);
        for phase in ["native", "read", "uncertified"] {
            let name = format!("{phase}-{family}");
            assert_eq!(
                rows.iter()
                    .find(|r| r.phase == name)
                    .ok_or("phase")?
                    .results
                    .len(),
                4,
                "complete materialization and reading"
            );
        }
        let release = format!("release-{family}");
        assert_eq!(
            receipt.counters.get(&(release, "release".into())),
            Some(&2),
            "actual owner and outputs drop inside release"
        );
        let baseline = format!("uncertified-{family}");
        assert!(
            !receipt.receipts.keys().any(|key| key.2 == baseline),
            "baseline constructs no certificate"
        );
        assert!(
            !receipt.counters.keys().any(|key| key.0 == baseline),
            "baseline makes no engine call"
        );
        assert!(
            receipt
                .boundary_work
                .iter()
                .all(|(key, work)| key.0 != baseline || *work == 0)
        );
    }
    Ok(())
}
/// Comparable samples suppress observation even under an outer collector; collection loss is explicit.
/// # Errors
/// Returns numerical, collection or lock errors.
/// # Panics
/// Panics if disabled samples emit events or a failed collection is reported as success.
/// ´claim:matrix:prepared-sigmoid-tanh-measurement-collection´
/// ´test:integration:prepared-sigmoid-tanh-measurement-collection´
#[test]
fn prepared_sigmoid_tanh_measurement_collection() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    let (result, outer) = trace::observed(4096, || run(4, false, 4096));
    result?;
    assert!(
        outer.clocks.is_empty() && outer.ledgers.is_empty(),
        "comparable samples leave tracing off"
    );
    let mut args = arguments();
    args[1] = "enabled".into();
    let p = protocol::Protocol::parse(&args)?;
    let mut output = Vec::new();
    assert!(
        write_run(&p, 0, &mut output).is_err(),
        "collection failure is not success"
    );
    let output = String::from_utf8(output)?;
    assert!(output.contains("collection-failure") && output.contains("incomplete tracing receipt"));
    assert!(!output.contains("native-tanh"));
    Ok(())
}
/// Missing or inconsistent process provenance refuses; discarded warm-up emits no estimate.
/// # Errors
/// Returns numerical, protocol or lock errors.
/// # Panics
/// Panics if CPU zero, missing warm-up, missing process count or unbalanced order is admitted.
/// ´claim:matrix:prepared-sigmoid-tanh-measurement-protocol´
/// ´test:integration:prepared-sigmoid-tanh-measurement-protocol´
#[test]
fn prepared_sigmoid_tanh_measurement_protocol() -> TestResult {
    let _guard = DISPATCHERS.lock().map_err(|_| "dispatcher guard")?;
    for (index, value) in [
        (2, "0"),
        (4, "4"),
        (5, "0"),
        (6, ""),
        (6, "same,same"),
        (6, "first,second,third"),
        (3, "6"),
    ] {
        let mut args = arguments();
        args[index] = value.into();
        assert!(
            protocol::Protocol::parse(&args).is_err(),
            "incomplete process protocol admitted: {index}={value}"
        );
    }
    for index in 0..12 {
        let mut args = arguments();
        args.remove(index);
        assert!(
            protocol::Protocol::parse(&args).is_err(),
            "missing field {index}"
        );
    }
    let p = protocol::Protocol::parse(&arguments())?;
    let mut measured = Vec::new();
    write_run(&p, 4096, &mut measured)?;
    let measured = String::from_utf8(measured)?;
    assert_eq!(
        measured.lines().count(),
        20,
        "each phase has its own live record"
    );
    for line in measured.lines() {
        assert!(
            line.contains("\"source\":")
                && line.contains("cpu=1")
                && line.contains("processes=5")
                && line.contains("warm_up_pid=1")
                && line.contains("round_order="),
            "each phase retains the complete process protocol"
        );
    }
    p.check_affinity("Cpus_allowed_list:\t1")?;
    for status in [
        "",
        "Cpus_allowed_list: 0",
        "Cpus_allowed_list: 2",
        "Cpus_allowed_list: 1-2",
        "Cpus_allowed_list: 1,2",
    ] {
        assert!(
            p.check_affinity(status).is_err(),
            "affinity mismatch admitted: {status}"
        );
    }
    for ordinal in [3, 2, 5, 1, 4] {
        let mut single = arguments();
        single[6] = "only-build".into();
        single[3] = ordinal.to_string();
        assert_eq!(protocol::Protocol::parse(&single)?.order(), ["only-build"]);
        let mut args = arguments();
        args[3] = ordinal.to_string();
        let p = protocol::Protocol::parse(&args)?;
        assert_eq!(
            p.order(),
            if ordinal % 2 == 0 {
                vec!["second", "first"]
            } else {
                vec!["first", "second"]
            }
        );
        assert!(
            p.source(None).contains("processes=5")
                && p.source(None).contains("cpu=1")
                && p.source(None).contains("warm_up_pid=1")
        );
    }
    let mut args = arguments();
    args[3] = "0".into();
    args[5] = "0".into();
    let p = protocol::Protocol::parse(&args)?;
    let mut output = Vec::new();
    write_run(&p, 4096, &mut output)?;
    let text = String::from_utf8(output)?;
    assert_eq!(
        text.lines().count(),
        1,
        "warm-up is not counted among estimates"
    );
    assert!(text.contains("discarded warm-up process") && !text.contains("native-tanh"));
    Ok(())
}

/// Every admitted population is a distinct generated order; oversized populations refuse.
/// # Errors
/// Returns input validation failures.
/// # Panics
/// Panics if generation sorts, duplicates or admits an invalid population.
/// ´claim:matrix:prepared-sigmoid-tanh-measurement-inputs´
/// ´test:integration:prepared-sigmoid-tanh-measurement-inputs´
#[test]
fn prepared_sigmoid_tanh_measurement_inputs() -> TestResult {
    for count in [4096, 4, 256, 16] {
        let points = inputs(count)?;
        let distinct = points
            .iter()
            .map(|x| x.to_bits())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(distinct.len(), points.len());
        assert!(points.windows(2).any(|p| p[0] > p[1]));
        assert!(points.windows(2).any(|p| p[0] < p[1]));
    }
    for count in [0, 3, 5, 4097, u32::MAX] {
        assert!(inputs(count).is_err());
    }
    Ok(())
}

/// Live provenance obeys the engineering calendar and nonblank single-line cell contracts without normalizing its command.
/// # Errors
/// Returns unexpected protocol refusals.
/// # Panics
/// Panics if malformed dates or text pass, or valid leap days and original text change.
/// ´claim:matrix:prepared-sigmoid-tanh-measurement-identity´
/// ´test:integration:prepared-sigmoid-tanh-measurement-identity´
#[test]
fn prepared_sigmoid_tanh_measurement_identity() -> TestResult {
    for value in [
        "not-a-date",
        "2026-13-01",
        "2026-02-29",
        "1900-02-29",
        "0000-01-01",
        "2026-01-00",
        "2026-04-31",
        "2026-1-01",
        "2026-00-01",
        "202a-01-01",
        "+026-01-01",
    ] {
        let mut args = arguments();
        args[8] = value.into();
        assert!(
            protocol::Protocol::parse(&args).is_err(),
            "invalid calendar date admitted: {value}"
        );
    }
    for index in [6, 10, 11] {
        for value in [" ", "", "first|second", "first\nsecond", "first\rsecond"] {
            let mut args = arguments();
            args[index] = value.into();
            assert!(
                protocol::Protocol::parse(&args).is_err(),
                "invalid engineering text admitted: {index}={value:?}"
            );
        }
    }
    for date in ["2024-02-29", "2000-02-29", "2026-04-30", "2026-12-31"] {
        let mut args = arguments();
        args[8] = date.into();
        args[10] = "  exact command  ".into();
        let p = protocol::Protocol::parse(&args)?;
        assert_eq!(p.date, date);
        assert_eq!(p.command, args[10], "original command spelling retained");
    }
    Ok(())
}

/// Every private clock event follows real setup and precedes no unfinished result or destruction.
/// # Panics
/// Panics on a misplaced reading or incomplete materialization.
fn check_boundaries(boundaries: &[BoundaryEvent]) {
    assert_eq!(
        boundaries.len(),
        160,
        "every phase has setup, start, completion and stop"
    );
    for events in boundaries.as_chunks::<4>().0 {
        let phase = events[0].0;
        assert!(
            events.iter().all(|event| event.0 == phase),
            "phase clock association: {events:?}"
        );
        assert_eq!(
            events.iter().map(|event| event.1).collect::<Vec<_>>(),
            ["setup", "start", "complete", "stop"],
            "clock boundary order: {phase}"
        );
        let setup = if phase.starts_with("release-") {
            2
        } else if ["native-", "read-", "uncertified-"]
            .iter()
            .any(|prefix| phase.starts_with(prefix))
        {
            4
        } else {
            1
        };
        let complete = if phase.starts_with("release-") {
            0
        } else if ["import-", "native-", "batch-", "read-", "uncertified-"]
            .iter()
            .any(|prefix| phase.starts_with(prefix))
        {
            4
        } else {
            1
        };
        assert_eq!(events[0].2, setup, "setup complete at start: {phase}");
        assert_eq!(
            events[2].2, complete,
            "result or destruction complete at stop: {phase}"
        );
    }
}
