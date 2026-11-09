// SPDX-License-Identifier: AGPL-3.0-only
//! Deadline admission and the sum of every mandatory finalization component.

use super::*;

/// The reviewed composed fixture has seven milliseconds residual and eleven tail.
/// ´test:unit:sampling-composed-deadline-tail´
#[test]
fn composed_deadline_tail() -> Result<(), SamplingError> {
    for (cleanup, report_cost, delay) in [(0, 0, 0), (2, 4, 0), (2, 4, 5)] {
        let clock = Clock::default();
        clock
            .injection
            .set(Some((SamplingPhase::Report, ms(report_cost))));
        let mut operation = Operation::new(&clock);
        operation.costs = [ms(99), ms(0), ms(8), ms(3), ms(8), ms(cleanup)];
        operation.delay = ms(delay);
        let mut policy = settings();
        policy.wall_budget = ms(100);
        policy.bounds.finalization = ms(report_cost);
        let report = sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock)?;
        assert_eq!(
            report.stop(),
            &SamplingStop::Deadline(SamplingPhase::Operation)
        );
        assert_eq!(operation.calls, 1);
        assert_eq!(operation.live.get(), 0);
        assert_eq!(report.measured_elapsed(), ms(8 + delay));
        assert_eq!(
            report.total_elapsed(),
            ms(118 + cleanup + report_cost + delay)
        );
        assert_eq!(report.overshoot(), ms(18 + cleanup + report_cost + delay));
        assert_eq!(report.finalization_bound(), ms(11 + cleanup + report_cost));
        assert_eq!(
            report.total_elapsed(),
            policy.wall_budget + ms(7) + report.finalization_bound() + ms(delay)
        );
        assert!(report.total_elapsed() > policy.wall_budget + ms(7 + 8));
        assert_eq!(
            report.iterations()[0]
                .receipt
                .expect("mandatory receipt")
                .operation
                .expect("work")
                .spent,
            7
        );
        assert!(report.analysis().is_none());
        assert!(report.extrapolate_batch(100).expect("partial").is_none());
    }
    Ok(())
}

/// Expiry in any collection or analysis phase prevents further ordinary admission.
/// ´test:unit:sampling-deadline-every-phase´
#[test]
fn every_phase() -> Result<(), SamplingError> {
    let phases = [
        SamplingPhase::Validation,
        SamplingPhase::Allocation,
        SamplingPhase::Reset,
        SamplingPhase::Snapshot,
        SamplingPhase::Receipt,
        SamplingPhase::Disposal,
        SamplingPhase::Cleanup,
        SamplingPhase::Schedule,
        SamplingPhase::Analysis(AnalysisPhase::Validation),
        SamplingPhase::Analysis(AnalysisPhase::Allocation),
        SamplingPhase::Analysis(AnalysisPhase::Statistics),
        SamplingPhase::Analysis(AnalysisPhase::Resampling),
        SamplingPhase::Analysis(AnalysisPhase::Sorting),
        SamplingPhase::Analysis(AnalysisPhase::Report),
        SamplingPhase::Report,
    ];
    for phase in phases {
        let clock = Clock::default();
        clock.injection.set(Some((phase, ms(1000))));
        let mut operation = Operation::new(&clock);
        let report = sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(report.stop(), &SamplingStop::Deadline(phase));
        assert_eq!(operation.live.get(), 0);
        assert!(report.analysis().is_none());
        assert_eq!(report.achieved_resamples(), 0);
        if matches!(phase, SamplingPhase::Analysis(_) | SamplingPhase::Report) {
            assert_eq!(report.rows().len(), 10);
            assert_eq!(operation.calls, 56);
        } else {
            assert!(operation.calls <= 1);
        }
    }
    Ok(())
}

/// Expiry inside reset or entry snapshot releases private state without invoking.
/// ´test:unit:sampling-reset-and-snapshot-expiry´
#[test]
fn reset_and_snapshot_expiry() -> Result<(), SamplingError> {
    for cost_index in [0, 1] {
        let clock = Clock::default();
        let mut operation = Operation::new(&clock);
        operation.costs[cost_index] = ms(1000);
        let report = sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(operation.calls, 0);
        assert_eq!(operation.resets, 1);
        assert_eq!(operation.live.get(), 0);
        assert_eq!(report.iterations()[0].outcome, IterationOutcome::NotInvoked);
        assert_eq!(report.iterations()[0].cleanup, Some(Ok(())));
        assert_eq!(report.measured_elapsed(), Duration::ZERO);
    }
    Ok(())
}

/// A partly collected measurement row remains evidence, never a nominal full row.
/// ´test:unit:sampling-partial-row-preservation´
#[test]
fn partial_row() -> Result<(), SamplingError> {
    let clock = Clock::default();
    let mut operation = Operation::new(&clock);
    let mut policy = settings();
    policy.wall_budget = ms(6);
    let report = sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock)?;
    assert_eq!(operation.calls, 6);
    assert_eq!(report.rows().len(), 2);
    assert_eq!(report.iterations().len(), 6);
    assert_eq!(report.measured_elapsed(), ms(6));
    assert_eq!(
        report.iterations()[5].position,
        SchedulePosition::Measurement(2)
    );
    assert!(report.analysis().is_none());
    Ok(())
}

/// A fully observed final row is retained even though no analysis may start after D.
/// ´test:unit:sampling-final-row-at-deadline´
#[test]
fn final_row_at_deadline() -> Result<(), SamplingError> {
    for (budget, expected_rows) in [(2, 1), (4, 2), (56, 10)] {
        let clock = Clock::default();
        let mut operation = Operation::new(&clock);
        let policy = SamplingSettings {
            wall_budget: ms(budget),
            ..settings()
        };
        let report = sample_with_clock(&[4, 5], &representative(), &mut operation, policy, &clock)?;
        assert_eq!(report.rows().len(), expected_rows);
        assert_eq!(operation.calls, budget);
        assert_eq!(operation.live.get(), 0);
        assert!(report.analysis().is_none());
        assert!(report.extrapolate_batch(1).expect("no estimate").is_none());
    }
    Ok(())
}

/// Receipt and disposal expiry preserve ordinary charges and finish all later callbacks.
/// ´test:unit:sampling-mandatory-callback-expiry´
#[test]
fn mandatory_callback_expiry() -> Result<(), SamplingError> {
    for cost_index in [3, 4, 5] {
        let clock = Clock::default();
        let mut operation = Operation::new(&clock);
        operation.costs[cost_index] = ms(1000);
        let report = sample_with_clock(
            &[4, 5],
            &representative(),
            &mut operation,
            settings(),
            &clock,
        )?;
        assert_eq!(operation.calls, 1);
        assert_eq!(operation.live.get(), 0);
        assert_eq!(report.measured_elapsed(), ms(1));
        assert_eq!(report.total_elapsed(), ms(1001));
        assert_eq!(report.iterations()[0].disposal, Some(Ok(())));
        assert_eq!(report.iterations()[0].cleanup, Some(Ok(())));
        assert_eq!(
            report.iterations()[0]
                .receipt
                .expect("receipt")
                .operation
                .expect("work")
                .spent,
            7
        );
        assert!(matches!(report.stop(), SamplingStop::Deadline(_)));
    }
    Ok(())
}
