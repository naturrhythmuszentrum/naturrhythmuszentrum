// SPDX-License-Identifier: AGPL-3.0-only
//! Public analysis behavior, capacity admission, cancellation and overflow.

use super::*;

/// Fixed comparison tolerance for independently specified modest fixture values.
const TOLERANCE: f64 = 1.0e-11;

/// No statistical helper participates in the expected numerical value.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{actual} != {expected}"
    );
}

/// Explicit test policy with generous finite payload bounds.
fn settings() -> Result<AnalysisSettings, AnalysisError> {
    AnalysisSettings::new(
        10,
        1000,
        0.95,
        42,
        AnalysisCaps {
            report_bytes: 8192,
            scratch_bytes: 65536,
        },
    )
}

/// Ten completed linear rows, each item taking exactly two seconds.
fn rows() -> [SampleRow; 10] {
    std::array::from_fn(|i| {
        let count = u32::try_from(i + 1).expect("ten fixture rows");
        SampleRow {
            iterations: u64::from(count),
            elapsed_seconds: 2.0 * f64::from(count),
        }
    })
}

/// Equal observations give degenerate intervals and both reciprocal units.
/// ´test:unit:sampling-complete-report-and-throughput´
#[test]
fn complete_report_and_throughput() -> Result<(), AnalysisError> {
    let settings = settings()?;
    let rows = rows();
    let report = analyze(&rows, settings, Some(8), |_| false)?;
    assert_eq!(report.settings(), settings);
    assert_eq!(report.settings().seed(), 42);
    assert_eq!(report.source_revision(), SOURCE_REVISION);
    assert_eq!(report.estimator_revision(), ESTIMATOR_REVISION);
    assert_eq!(report.observations().len(), 10);
    assert_eq!(report.label_counts(), [0, 0, 10, 0, 0]);
    assert_eq!(report.elements_per_item(), Some(8));
    for (observation, row) in report.observations().iter().zip(rows) {
        assert_eq!(observation.row.iterations, row.iterations);
        assert_eq!(
            observation.row.elapsed_seconds.to_bits(),
            row.elapsed_seconds.to_bits()
        );
        close(observation.seconds_per_item, 2.0);
    }
    let estimates = report.estimates();
    for (estimate, expected) in [
        (estimates.mean, 2.0),
        (estimates.median, 2.0),
        (estimates.slope, 2.0),
        (estimates.standard_deviation, 0.0),
        (estimates.median_absolute_deviation, 0.0),
    ] {
        close(estimate.point_estimate, expected);
        close(estimate.standard_error, 0.0);
        close(estimate.confidence_interval.lower_bound, expected);
        close(estimate.confidence_interval.upper_bound, expected);
    }
    close(report.throughput().items_per_second.point_estimate, 0.5);
    let elements = report
        .throughput()
        .elements_per_second
        .expect("declared elements");
    close(elements.point_estimate, 4.0);
    close(elements.confidence_interval.lower_bound, 4.0);
    close(elements.confidence_interval.upper_bound, 4.0);
    let batch = report.extrapolate_batch(100)?;
    close(batch.estimated_seconds, 200.0);
    close(batch.confidence_interval.lower_bound, 200.0);
    close(batch.confidence_interval.upper_bound, 200.0);
    let empty = report.extrapolate_batch(0)?;
    assert_eq!(empty.estimated_seconds.to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        empty.confidence_interval.lower_bound.to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        empty.confidence_interval.upper_bound.to_bits(),
        0.0_f64.to_bits()
    );
    let absent = analyze(&rows, settings, None, |_| false)?;
    assert_eq!(absent.throughput().elements_per_second, None);
    Ok(())
}

/// Outliers remain in mean, slope and resampling, in their original positions.
/// ´test:unit:sampling-outliers-are-labels-only´
#[test]
fn outliers_are_labels_only() -> Result<(), AnalysisError> {
    let mut rows = [SampleRow {
        iterations: 1,
        elapsed_seconds: 2.0,
    }; 10];
    rows[4].elapsed_seconds = 100.0;
    let report = analyze(&rows, settings()?, Some(3), |_| false)?;
    close(report.estimates().mean.point_estimate, 11.8);
    close(report.estimates().slope.point_estimate, 11.8);
    close(report.estimates().median.point_estimate, 2.0);
    close(
        report.estimates().median_absolute_deviation.point_estimate,
        0.0,
    );
    close(report.fences().lower_inner, 2.0);
    close(report.fences().upper_outer, 2.0);
    assert_eq!(report.label_counts(), [0, 0, 9, 0, 1]);
    assert_eq!(report.observations()[4].label, TukeyLabel::HighSevere);
    let slope = report.estimates().slope;
    let rate = report.throughput().items_per_second;
    close(
        rate.confidence_interval.lower_bound,
        1.0 / slope.confidence_interval.upper_bound,
    );
    close(
        rate.confidence_interval.upper_bound,
        1.0 / slope.confidence_interval.lower_bound,
    );
    let elements = report
        .throughput()
        .elements_per_second
        .expect("declared elements");
    close(
        elements.confidence_interval.lower_bound,
        3.0 / slope.confidence_interval.upper_bound,
    );
    close(
        elements.confidence_interval.upper_bound,
        3.0 / slope.confidence_interval.lower_bound,
    );
    let batch = report.extrapolate_batch(121)?;
    close(
        batch.confidence_interval.lower_bound,
        121.0 * slope.confidence_interval.lower_bound,
    );
    close(
        batch.confidence_interval.upper_bound,
        121.0 * slope.confidence_interval.upper_bound,
    );
    Ok(())
}

/// All invalid settings are refused before a report or operation can exist.
/// ´test:unit:sampling-invalid-settings-and-counts´
#[test]
fn invalid_settings_and_counts() -> Result<(), AnalysisError> {
    let caps = settings()?.caps();
    for count in [0, 1, 9] {
        assert_eq!(
            AnalysisSettings::new(count, 1000, 0.95, 0, caps),
            Err(AnalysisError::SampleCount)
        );
    }
    for count in [0, 999] {
        assert_eq!(
            AnalysisSettings::new(10, count, 0.95, 0, caps),
            Err(AnalysisError::ResampleCount)
        );
    }
    if let Ok(too_large) = usize::try_from(u64::from(u32::MAX) + 1) {
        assert_eq!(
            AnalysisSettings::new(too_large, 1000, 0.95, 0, caps),
            Err(AnalysisError::SampleCount)
        );
        assert_eq!(
            AnalysisSettings::new(10, too_large, 0.95, 0, caps),
            Err(AnalysisError::ResampleCount)
        );
    }
    for confidence in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        0.0,
        1.0,
        2.0,
    ] {
        assert_eq!(
            AnalysisSettings::new(10, 1000, confidence, 0, caps),
            Err(AnalysisError::Confidence)
        );
    }
    let rows = rows();
    assert!(matches!(
        analyze(&rows[..9], settings()?, None, |_| false),
        Err(AnalysisError::RowCount {
            expected: 10,
            actual: 9
        })
    ));
    for elements in [0, (1_u64 << 53) + 1, u64::MAX] {
        assert!(matches!(
            analyze(&rows, settings()?, Some(elements), |_| false),
            Err(AnalysisError::InvalidElements)
        ));
    }
    Ok(())
}

/// Nonfinite, nonpositive, unrepresentable and underflowed rows stay typed.
/// ´test:unit:sampling-invalid-rows-and-arithmetic´
#[test]
fn invalid_rows_and_arithmetic() -> Result<(), AnalysisError> {
    for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -0.0, -1.0] {
        let mut rows = rows();
        rows[6].elapsed_seconds = time;
        assert!(matches!(
            analyze(&rows, settings()?, None, |_| false),
            Err(AnalysisError::InvalidTime { index: 6 })
        ));
    }
    for iterations in [0, (1_u64 << 53) + 1, u64::MAX] {
        let mut rows = rows();
        rows[3].iterations = iterations;
        assert!(matches!(
            analyze(&rows, settings()?, None, |_| false),
            Err(AnalysisError::InvalidIterations { index: 3 })
        ));
    }
    let underflow = [SampleRow {
        iterations: 2,
        elapsed_seconds: f64::from_bits(1),
    }; 10];
    assert!(matches!(
        analyze(&underflow, settings()?, None, |_| false),
        Err(AnalysisError::NonPositiveTimeBound)
    ));
    let overflow = [SampleRow {
        iterations: 1,
        elapsed_seconds: f64::MAX,
    }; 10];
    assert!(matches!(
        analyze(&overflow, settings()?, None, |_| false),
        Err(AnalysisError::Arithmetic)
    ));
    let mut variance = rows();
    variance[9].elapsed_seconds = 1.0e200;
    assert!(matches!(
        analyze(&variance, settings()?, None, |_| false),
        Err(AnalysisError::Arithmetic)
    ));
    Ok(())
}

/// Every phase can stop without a report, including mid-sort and late assembly.
/// ´test:unit:sampling-cancellation-in-every-analysis-phase´
#[test]
fn cancellation_in_every_analysis_phase() -> Result<(), AnalysisError> {
    for phase in [
        AnalysisPhase::Validation,
        AnalysisPhase::Allocation,
        AnalysisPhase::Statistics,
        AnalysisPhase::Resampling,
        AnalysisPhase::Sorting,
        AnalysisPhase::Report,
    ] {
        let mut checks = 0;
        let result = analyze(&rows(), settings()?, None, |current| {
            if current == phase {
                checks += 1;
            }
            checks == 3
        });
        assert!(
            matches!(result, Err(AnalysisError::Cancelled { phase: observed }) if observed == phase)
        );
        assert_eq!(checks, 3);
    }
    let immediate = analyze(&rows(), settings()?, None, |_| true);
    assert!(matches!(
        immediate,
        Err(AnalysisError::Cancelled {
            phase: AnalysisPhase::Validation
        })
    ));
    Ok(())
}

/// Exact and one-below payload boundaries and checked allocator size overflow.
/// ´test:unit:sampling-capacity-and-allocation-boundaries´
#[test]
fn capacity_and_allocation_boundaries() -> Result<(), AnalysisError> {
    let caps = run::required_caps(10, 1000)?;
    // Seven vector records coexist with a u128-aligned generator in one record;
    // the normalization vector is separate. Include the aggregate tail padding.
    let alignment = align_of::<Vec<f64>>().max(align_of::<oorandom::Rand64>());
    let workspace_records = 7 * size_of::<Vec<f64>>() + size_of::<oorandom::Rand64>();
    let fixed = workspace_records.next_multiple_of(alignment) + size_of::<Vec<f64>>();
    assert_eq!(caps.scratch_bytes, fixed + (30 + 5000) * size_of::<f64>());
    assert_eq!(
        caps.report_bytes,
        size_of::<AnalysisReport>() + 10 * size_of::<Observation>()
    );
    let exact = AnalysisSettings::new(10, 1000, 0.95, 42, caps)?;
    assert_eq!(
        analyze(&rows(), exact, None, |_| false)?
            .observations()
            .len(),
        10
    );
    for kind in [CapacityKind::Report, CapacityKind::Scratch] {
        let mut below = caps;
        match kind {
            CapacityKind::Report => below.report_bytes -= 1,
            CapacityKind::Scratch => below.scratch_bytes -= 1,
        }
        assert_eq!(
            AnalysisSettings::new(10, 1000, 0.95, 42, below),
            Err(AnalysisError::Capacity { kind })
        );
    }
    let mut never = |_| false;
    let mut control = memory::Control::new(&mut never);
    let mut memory = memory::Memory::new(CapacityKind::Scratch, usize::MAX, 0)?;
    assert!(matches!(
        memory.reserve::<f64>(usize::MAX, &mut control),
        Err(AnalysisError::Capacity {
            kind: CapacityKind::Scratch
        })
    ));
    let mut memory = memory::Memory::new(CapacityKind::Scratch, usize::MAX, 0)?;
    assert!(matches!(
        memory.reserve::<u8>(usize::MAX, &mut control),
        Err(AnalysisError::Allocation {
            kind: CapacityKind::Scratch
        })
    ));
    Ok(())
}

/// Reciprocal overflow and batch overflow produce no finite claim.
/// ´test:unit:sampling-reciprocal-and-batch-overflow´
#[test]
fn reciprocal_and_batch_overflow() -> Result<(), AnalysisError> {
    let tiny = [SampleRow {
        iterations: 1,
        elapsed_seconds: f64::from_bits(1),
    }; 10];
    assert!(matches!(
        analyze(&tiny, settings()?, None, |_| false),
        Err(AnalysisError::Arithmetic)
    ));
    let small = [SampleRow {
        iterations: 1,
        elapsed_seconds: 2.0_f64.powi(-1020),
    }; 10];
    assert!(analyze(&small, settings()?, None, |_| false).is_ok());
    assert!(matches!(
        analyze(&small, settings()?, Some(1_u64 << 53), |_| false),
        Err(AnalysisError::Arithmetic)
    ));
    let large = [SampleRow {
        iterations: 1,
        elapsed_seconds: 2.0_f64.powi(1000),
    }; 10];
    let report = analyze(&large, settings()?, None, |_| false)?;
    assert!(matches!(
        report.extrapolate_batch(1_u64 << 53),
        Err(AnalysisError::Arithmetic)
    ));
    assert!(matches!(
        report.extrapolate_batch(u64::MAX),
        Err(AnalysisError::CountRepresentation)
    ));
    assert_eq!(
        report.extrapolate_batch(0)?.estimated_seconds.to_bits(),
        0.0_f64.to_bits()
    );
    Ok(())
}

/// Identical seed/rows replay exactly and another analysis cannot perturb state.
/// ´test:unit:sampling-independent-seed-replay´
#[test]
fn independent_seed_replay() -> Result<(), AnalysisError> {
    let mut rows = rows();
    rows[4].elapsed_seconds = 31.0;
    let first = analyze(&rows, settings()?, None, |_| false)?;
    let other = AnalysisSettings::new(10, 1000, 0.95, 912, settings()?.caps())?;
    let separate = analyze(&rows, other, None, |_| false)?;
    let replay = analyze(&rows, settings()?, None, |_| false)?;
    for (left, right) in [
        (first.estimates().mean, replay.estimates().mean),
        (first.estimates().slope, replay.estimates().slope),
    ] {
        assert_eq!(
            left.standard_error.to_bits(),
            right.standard_error.to_bits()
        );
        assert_eq!(
            left.confidence_interval.lower_bound.to_bits(),
            right.confidence_interval.lower_bound.to_bits()
        );
        assert_eq!(
            left.confidence_interval.upper_bound.to_bits(),
            right.confidence_interval.upper_bound.to_bits()
        );
    }
    assert_ne!(
        first.estimates().slope.standard_error.to_bits(),
        separate.estimates().slope.standard_error.to_bits()
    );
    Ok(())
}

/// The exact integer domain includes its upper boundary for all three meanings.
/// ´test:unit:sampling-exact-count-upper-boundary´
#[test]
fn exact_count_upper_boundary() -> Result<(), AnalysisError> {
    let maximum = 1_u64 << 53;
    let rows = [SampleRow {
        iterations: maximum,
        elapsed_seconds: 2.0_f64.powi(54),
    }; 10];
    let report = analyze(&rows, settings()?, Some(maximum), |_| false)?;
    close(report.estimates().slope.point_estimate, 2.0);
    let elements = report
        .throughput()
        .elements_per_second
        .expect("declared elements");
    assert_eq!(
        elements.point_estimate.to_bits(),
        2.0_f64.powi(52).to_bits()
    );
    assert_eq!(
        report
            .extrapolate_batch(maximum)?
            .estimated_seconds
            .to_bits(),
        2.0_f64.powi(54).to_bits()
    );
    Ok(())
}
