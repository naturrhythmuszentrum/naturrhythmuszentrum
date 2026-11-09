// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite-data oracles, explicit resamples and generator pins.

use super::super::CapacityKind;
use super::random::Indices;
use super::*;

/// Explicit absolute comparison tolerance for the fixed small rational fixtures.
const TOLERANCE: f64 = 1.0e-12;

/// Compare numerical results without float equality or shared estimator helpers.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{actual} != {expected}"
    );
}

/// All rows are derived from t = k(k+1), with normalized times 2, 3, 4.
const ROWS: [SampleRow; 3] = [
    SampleRow {
        iterations: 1,
        elapsed_seconds: 2.0,
    },
    SampleRow {
        iterations: 2,
        elapsed_seconds: 6.0,
    },
    SampleRow {
        iterations: 3,
        elapsed_seconds: 12.0,
    },
];

/// Deterministic explicit indices separate estimator checks from RNG changes.
struct Explicit<'a> {
    /// Remaining ordered draw indices.
    indices: std::slice::Iter<'a, usize>,
}

impl Indices for Explicit<'_> {
    fn next(&mut self, upper: usize, control: &mut Control<'_>) -> Result<usize, AnalysisError> {
        control.check(AnalysisPhase::Resampling)?;
        let index = *self.indices.next().ok_or(AnalysisError::InvalidIndex)?;
        if index < upper {
            Ok(index)
        } else {
            Err(AnalysisError::InvalidIndex)
        }
    }
}

/// Direct rational point values and fences, including exact fence equality.
/// ´test:unit:sampling-hand-derived-statistics´
#[test]
fn hand_derived_statistics() -> Result<(), AnalysisError> {
    let mut never = |_| false;
    let mut control = Control::new(&mut never);
    let sample = Sample::new(&[2.0, 3.0, 4.0], &mut control)?;
    close(sample.mean(&mut control)?, 3.0);
    close(sample.variance(3.0, &mut control)?, 1.0);
    let mut sorted = Vec::with_capacity(3);
    let statistics = sample.statistics(&mut sorted, &mut control)?;
    for (actual, expected) in statistics.into_iter().zip([3.0, 1.0, 3.0, 1.4826]) {
        close(actual, expected);
    }
    close(
        Data::new(&ROWS, &mut control)?.slope(&mut control)?,
        25.0 / 7.0,
    );
    let fences = tukey::fences(&[2.0, 3.0, 4.0])?;
    close(fences.lower_outer, -0.5);
    close(fences.lower_inner, 1.0);
    close(fences.upper_inner, 5.0);
    close(fences.upper_outer, 6.5);
    for (value, label) in [
        (-1.0, TukeyLabel::LowSevere),
        (-0.5, TukeyLabel::LowMild),
        (0.0, TukeyLabel::LowMild),
        (1.0, TukeyLabel::NotAnOutlier),
        (5.0, TukeyLabel::NotAnOutlier),
        (6.5, TukeyLabel::HighMild),
        (7.0, TukeyLabel::HighSevere),
    ] {
        assert_eq!(tukey::label(value, fences), label);
    }
    for (percentile, expected) in [
        (0.0, 2.0),
        (12.5, 2.25),
        (25.0, 2.5),
        (50.0, 3.0),
        (75.0, 3.5),
        (100.0, 4.0),
    ] {
        close(percentiles::at(&[2.0, 3.0, 4.0], percentile)?, expected);
    }
    Ok(())
}

/// Enumerate all 27 triples; derive statistics with integer sums and order ranks.
/// This oracle never calls an extracted helper.
fn oracle() -> (Vec<usize>, [Vec<f64>; 5]) {
    let mut sequence = Vec::new();
    let mut distributions: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::new());
    for first in 0_u32..3 {
        for second in 0_u32..3 {
            for third in 0_u32..3 {
                let indices = [first, second, third];
                sequence.extend(indices.map(|i| usize::try_from(i).expect("tiny fixture index")));
                let mut times = indices.map(|i| i + 2);
                let sum: u32 = times.iter().sum();
                let squares: u32 = times.iter().map(|i| i * i).sum();
                let mean = f64::from(sum) / 3.0;
                let variance = (f64::from(squares) - f64::from(sum * sum) / 3.0) / 2.0;
                times.sort_unstable();
                let median = times[1];
                let mut distances = times.map(|i| i.abs_diff(median));
                distances.sort_unstable();
                let counts = indices.map(|i| i + 1);
                let numerator: u32 = counts.iter().map(|k| k * k * (k + 1)).sum();
                let denominator: u32 = counts.iter().map(|k| k * k).sum();
                let values = [
                    mean,
                    variance.sqrt(),
                    f64::from(median),
                    f64::from(distances[1]) * 14826.0 / 10000.0,
                    f64::from(numerator) / f64::from(denominator),
                ];
                for (target, value) in distributions.iter_mut().zip(values) {
                    target.push(value);
                }
            }
        }
    }
    (sequence, distributions)
}

/// Independent second-moment standard error and rational percentile rank weights.
fn oracle_uncertainty(mut values: Vec<f64>) -> (f64, f64, f64) {
    let sum: f64 = values.iter().sum();
    let squares: f64 = values.iter().map(|v| v * v).sum();
    let deviation = ((squares - sum * sum / 27.0) / 26.0).max(0.0).sqrt();
    values.sort_by(f64::total_cmp);
    // 26 * 0.025 = 13/20; 26 * 0.975 = 25 + 7/20.
    let lower = 13.0_f64.mul_add(values[1], 7.0 * values[0]) / 20.0;
    let upper = 7.0_f64.mul_add(values[26], 13.0 * values[25]) / 20.0;
    (deviation, lower, upper)
}

/// Exhaustive paired and univariate resamples, complete distributions and CIs.
/// ´test:unit:sampling-exhaustive-bootstrap-oracle´
#[test]
fn exhaustive_bootstrap_oracle() -> Result<(), AnalysisError> {
    let (sequence, expected) = oracle();
    let mut never = |_| false;
    let mut control = Control::new(&mut never);
    let mut memory = Memory::new(CapacityKind::Scratch, 8192, 0)?;
    let mut actual = Distributions::new(27, &mut memory, &mut control)?;
    let mut stage = memory.reserve(3, &mut control)?;
    let mut sorted = memory.reserve(3, &mut control)?;
    let mut explicit = Explicit {
        indices: sequence.iter(),
    };
    actual.univariate(
        &[2.0, 3.0, 4.0],
        27,
        (&mut stage, &mut sorted),
        &mut explicit,
        &mut control,
    )?;
    assert_eq!(explicit.indices.len(), 0);
    let mut explicit = Explicit {
        indices: sequence.iter(),
    };
    actual.paired(
        Data::new(&ROWS, &mut control)?,
        27,
        &mut explicit,
        &mut control,
    )?;
    assert_eq!(explicit.indices.len(), 0);
    let points = [3.0, 1.0, 3.0, 1.4826, 25.0 / 7.0];
    let distributions = [
        &mut actual.mean,
        &mut actual.deviation,
        &mut actual.median,
        &mut actual.mad,
        &mut actual.slope,
    ];
    for ((observed, expected), point) in distributions.into_iter().zip(expected).zip(points) {
        assert_eq!(observed.len(), 27);
        for (&observed, &expected) in observed.iter().zip(&expected) {
            close(observed, expected);
        }
        let (deviation, lower, upper) = oracle_uncertainty(expected);
        let estimate = estimate::assemble(point, observed, 0.95, &mut control)?;
        close(estimate.standard_error, deviation);
        close(estimate.confidence_interval.lower_bound, lower);
        close(estimate.confidence_interval.upper_bound, upper);
    }
    let mean = estimate::assemble(3.0, &mut actual.mean, 0.95, &mut control)?;
    close(mean.confidence_interval.lower_bound, 133.0 / 60.0);
    close(mean.confidence_interval.upper_bound, 227.0 / 60.0);
    assert!((mean.standard_error - 0.480_384_461_4).abs() < 1.0e-10);
    let slope = estimate::assemble(25.0 / 7.0, &mut actual.slope, 0.95, &mut control)?;
    close(slope.confidence_interval.lower_bound, 73.0 / 30.0);
    close(slope.confidence_interval.upper_bound, 747.0 / 190.0);
    assert!((slope.standard_error - 0.501_150_833_9).abs() < 1.0e-10);
    Ok(())
}

/// The literal stream is independently derived from the published u128 recurrence.
/// ´test:unit:sampling-seeded-index-stream´
#[test]
fn seeded_index_stream() -> Result<(), AnalysisError> {
    let mut raw = oorandom::Rand64::new(42);
    let expected_raw = [
        12_410_087_264_455_502_793,
        359_948_335_059_059_064,
        14_020_691_344_464_510_033,
        15_073_013_605_522_435_475,
    ];
    for expected in expected_raw {
        assert_eq!(raw.rand_u64(), expected);
    }
    let mut indices = SeededIndices::new(42);
    let mut never = |_| false;
    let mut control = Control::new(&mut never);
    for expected in [6, 0, 7, 8, 9, 9, 9, 5, 4, 3, 5, 6, 0, 7, 0, 5] {
        assert_eq!(indices.next(10, &mut control)?, expected);
    }
    Ok(())
}

/// Rejections are unbiased, cancellable, finite and cannot substitute an index.
/// ´test:unit:sampling-index-rejection-bounds´
#[test]
fn index_rejection_bounds() -> Result<(), AnalysisError> {
    let mut checks = 0;
    let mut stop = |_| {
        checks += 1;
        checks == 3
    };
    let mut control = Control::new(&mut stop);
    assert_eq!(
        random::uniform(3, || 0, &mut control),
        Err(AnalysisError::Cancelled {
            phase: AnalysisPhase::Resampling
        })
    );
    let mut never = |_| false;
    let mut control = Control::new(&mut never);
    let mut draws = 0;
    assert_eq!(
        random::uniform(
            3,
            || {
                draws += 1;
                0
            },
            &mut control
        ),
        Err(AnalysisError::IndexRejectionLimit)
    );
    assert_eq!(draws, 128);
    let mut draws = [0, u64::MAX].into_iter();
    assert_eq!(
        random::uniform(3, || draws.next().expect("two draws"), &mut control)?,
        2
    );
    assert_eq!(
        random::uniform(0, || 0, &mut control),
        Err(AnalysisError::InvalidIndex)
    );
    Ok(())
}

/// Independent sort controls include reverse order, duplicates and mixed signs.
/// ´test:unit:sampling-sort-and-percentile-boundaries´
#[test]
fn sort_and_percentile_boundaries() -> Result<(), AnalysisError> {
    let mut never = |_| false;
    let mut control = Control::new(&mut never);
    for length in 0_u32..80 {
        let mut values: Vec<f64> = (0..length).rev().map(|i| f64::from(i % 7) - 3.0).collect();
        let mut expected = values.clone();
        expected.sort_by(f64::total_cmp);
        percentiles::sort(&mut values, &mut control)?;
        let actual_bits: Vec<u64> = values.iter().map(|v| v.to_bits()).collect();
        let expected_bits: Vec<u64> = expected.iter().map(|v| v.to_bits()).collect();
        assert_eq!(actual_bits, expected_bits);
    }
    for percentile in [f64::NAN, f64::INFINITY, -1.0, 101.0] {
        assert_eq!(
            percentiles::at(&[1.0, 2.0], percentile),
            Err(AnalysisError::InvalidPercentile)
        );
    }
    assert_eq!(
        percentiles::at(&[], 50.0),
        Err(AnalysisError::InvalidSample)
    );
    close(percentiles::at(&[9.0], 50.0)?, 9.0);
    assert!(matches!(
        Sample::new(&[1.0], &mut control),
        Err(AnalysisError::InvalidSample)
    ));
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            Sample::new(&[1.0, value], &mut control),
            Err(AnalysisError::InvalidSample)
        ));
    }
    assert_eq!(
        regression::Sums::default().slope(),
        Err(AnalysisError::ZeroRegressionDenominator)
    );
    Ok(())
}
