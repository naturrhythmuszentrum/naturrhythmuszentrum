// SPDX-License-Identifier: AGPL-3.0-only
//! Unsupported contracts, errors, resource caps and independent sampler state.

use super::*;

/// Every failure still disposes outputs and releases private owners, preserving Work.
/// ´test:unit:sampling-host-failure-finalization´
#[test]
fn host_failures() -> Result<(), SamplingError> {
    for (failure, phase) in [
        (Failure::Reset, SamplingPhase::Reset),
        (Failure::Operation, SamplingPhase::Operation),
        (Failure::Disposal, SamplingPhase::Disposal),
        (Failure::Cleanup, SamplingPhase::Cleanup),
    ] {
        let clock = Clock::default();
        let mut operation = Operation::new(&clock);
        operation.failure = Some(failure);
        let report = sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(report.stop(), &SamplingStop::HostError(phase, failure));
        assert_eq!(operation.live.get(), 0);
        assert!(report.analysis().is_none());
        if failure != Failure::Reset {
            assert_eq!(
                report.iterations()[0]
                    .receipt
                    .expect("receipt")
                    .operation
                    .expect("spent work")
                    .spent,
                7
            );
            assert!(report.iterations()[0].cleanup.is_some());
            assert!(report.iterations()[0].disposal.is_some());
        }
    }
    Ok(())
}

/// Noncooperative/reset-unsafe operations are unsupported before reset or execution.
/// ´test:unit:sampling-contract-and-cap-admission´
#[test]
fn contract_and_caps() {
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    operation.supported = false;
    assert!(matches!(
        sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock
        ),
        Err(SamplingError::Unsupported)
    ));
    operation.supported = true;
    for policy in [
        SamplingSettings {
            warm_up: Duration::ZERO,
            ..settings()
        },
        SamplingSettings {
            max_total_iterations: 1_u64 << 54,
            ..settings()
        },
        SamplingSettings {
            max_iterations_per_sample: 9,
            ..settings()
        },
        SamplingSettings {
            caps: AnalysisCaps {
                report_bytes: 1,
                scratch_bytes: 1,
            },
            ..settings()
        },
        SamplingSettings {
            wall_budget: Duration::MAX,
            ..settings()
        },
    ] {
        assert!(
            sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock).is_err()
        );
    }
    assert_eq!(operation.resets, 0);
    assert_eq!(operation.calls, 0);
}

/// Missing accounting and inadequate clock resolution are explicit outcomes.
/// ´test:unit:sampling-incomplete-work-and-resolution´
#[test]
fn incomplete_and_resolution() -> Result<(), SamplingError> {
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    operation.complete = false;
    let report = sample_with_clock(
        &[4, 5],
        &representative(),
        &mut operation,
        settings(),
        &clock,
    )?;
    assert_eq!(report.stop(), &SamplingStop::IncompleteReceipt);
    assert_eq!(
        report.iterations()[0].receipt.expect("preserved").status,
        ReceiptStatus::Incomplete
    );
    operation.complete = true;
    operation.costs[2] = Duration::ZERO;
    let report = sample_with_clock(
        &[4, 5],
        &representative(),
        &mut operation,
        settings(),
        &clock,
    )?;
    assert_eq!(report.stop(), &SamplingStop::ClockResolution);
    assert_eq!(operation.live.get(), 0);
    Ok(())
}

/// Concurrent calls own independent clocks, budgets and seeded statistics.
/// ´test:unit:sampling-concurrent-seeds´
#[test]
fn concurrent_seeds() {
    let prepared = [4, 5];
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for seed in [17, 19] {
            handles.push(scope.spawn(move || {
                let clock = Clock::default();
                let mut operation = Operation::new(&clock);
                let mut policy = settings();
                policy.analysis =
                    AnalysisSettings::new(10, 1000, 0.95, seed, policy.analysis.caps())
                        .expect("policy");
                let report =
                    sample_with_clock(&prepared, &representative(), &mut operation, policy, &clock)
                        .expect("sample");
                assert_eq!(report.stop(), &SamplingStop::Complete);
                assert_eq!(
                    report.analysis().expect("estimates").settings().seed(),
                    seed
                );
                assert_eq!(operation.live.get(), 0);
                assert_eq!(operation.calls, 56);
            }));
        }
        for handle in handles {
            handle.join().expect("independent sampler");
        }
    });
}

/// Each absent field independently defeats a claimed complete receipt without losing evidence.
/// ´test:unit:sampling-independently-missing-evidence´
#[test]
fn independently_missing_evidence() -> Result<(), SamplingError> {
    for missing in [
        MissingEvidence::Entry,
        MissingEvidence::Exit,
        MissingEvidence::Operation,
    ] {
        let clock = Clock::default();
        let mut operation = Operation::new(&clock);
        operation.missing = Some(missing);
        assert!(operation.complete);
        let report = sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(report.stop(), &SamplingStop::IncompleteReceipt);
        assert_eq!(operation.calls, 1);
        assert_eq!(operation.resets, 1);
        assert_eq!(operation.live.get(), 0);
        assert_eq!(report.iterations().len(), 1);
        let record = &report.iterations()[0];
        let entry = Work { owner: 1, spent: 0 };
        let spent = Work { owner: 1, spent: 7 };
        assert_eq!(
            record.entry,
            (missing != MissingEvidence::Entry).then_some(entry)
        );
        assert_eq!(
            record.receipt,
            Some(WorkReceipt {
                exit: (missing != MissingEvidence::Exit).then_some(spent),
                operation: (missing != MissingEvidence::Operation).then_some(spent),
                status: ReceiptStatus::Incomplete,
            })
        );
        assert_eq!(record.outcome, IterationOutcome::Returned(Ok(())));
        assert_eq!(record.elapsed, Some(ms(1)));
        assert_eq!(record.position, SchedulePosition::WarmUp(1));
        assert_eq!(record.disposal, Some(Ok(())));
        assert_eq!(record.cleanup, Some(Ok(())));
        assert!(report.analysis().is_none());
        assert!(
            report
                .extrapolate_batch(100)
                .expect("partial report")
                .is_none()
        );
    }
    Ok(())
}

/// Exact aggregate byte admission includes both evidence vectors and the analysis allowance.
/// ´test:unit:sampling-aggregate-cap-boundary´
#[test]
fn aggregate_cap_boundary() -> Result<(), SamplingError> {
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    let mut policy = settings();
    policy.max_total_iterations = 56;
    policy.max_iterations_per_sample = 10;
    policy.caps.report_bytes = size_of::<SamplingReport<'_, Work, Failure>>()
        + 56 * size_of::<IterationRecord<Work, Failure>>()
        + 10 * size_of::<SampleRow>()
        + policy.analysis.caps().report_bytes;
    let too_small = SamplingSettings {
        caps: AnalysisCaps {
            report_bytes: policy.caps.report_bytes - 1,
            scratch_bytes: policy.caps.scratch_bytes,
        },
        ..policy
    };
    assert!(matches!(
        sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            too_small,
            &clock
        ),
        Err(SamplingError::Capacity)
    ));
    assert_eq!(operation.calls, 0);
    let report = sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock)?;
    assert_eq!(report.stop(), &SamplingStop::Complete);
    assert_eq!(report.iterations().len(), 56);
    assert!(matches!(
        policy::reserve::<u64>(usize::MAX),
        Err(SamplingError::Allocation)
    ));
    let huge = SamplingSettings {
        measurement: Duration::MAX,
        ..settings()
    };
    assert_eq!(
        schedule::linear_step(huge, 1_u64 << 53, ms(1)),
        Err(SamplingError::Arithmetic)
    );
    Ok(())
}
