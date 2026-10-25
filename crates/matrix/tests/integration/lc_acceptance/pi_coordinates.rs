// SPDX-License-Identifier: AGPL-3.0-only
//! Graph-bound pi coordinates retain mixed deterministic and full statistical families.

use super::state_fixture;
use crate::integration::pi_conversion::{exact_policy, oracle, pi};
use crate::{
    kernel_oracle::Rational,
    reading_testing::{TestResult, ledger},
};
use nrz_matrix::{CoordinateReading, PiCoordinateContract};
use nrz_scalar::{ArithmeticContext, Scalar};

#[path = "pi_coordinates/fixture.rs"]
mod fixture;

/// Live normalization and restoration enclose both independent physical endpoint boxes.
/// ´test:integration:pi-live-coordinate-roundtrip-and-covariance´
#[test]
fn pi_live_coordinate_roundtrip_and_covariance() -> TestResult {
    roundtrip::<f32>()?;
    roundtrip::<f64>()
}

/// Shared coefficient regions transform both covariance axes while preserving the original targets.
fn roundtrip<P: Scalar + 'static>() -> TestResult {
    let plan = fixture::plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    let (mut heap, source) = fixture::imported::<P>(&plan)?;
    let normalized = heap.normalize_certified_reading(
        &source,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    check(&normalized, false)?;
    assert!(
        normalized
            .constant()
            .ok_or("pi evidence")?
            .same_region(&circle)
    );
    let warm = heap.normalize_certified_reading(
        &source,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    assert!(std::ptr::eq(
        normalized.coefficients().as_ptr(),
        warm.coefficients().as_ptr()
    ));
    let restored = heap.restore_certified_reading(
        &normalized,
        &contract,
        &fixture::execution(),
        &context,
        &mut ledger()?,
    )?;
    check(&restored, true)?;
    assert_eq!(restored.source().reading(), source.reading());
    assert!(
        restored
            .constant()
            .ok_or("restoration pi")?
            .same_region(&circle)
    );
    heap.compact()?;
    assert_eq!(heap.reading_entries(&source)?.len(), 2);
    Ok(())
}

/// Rational pi bounds are computed independently, including signed covariance entries.
fn check<P: Scalar>(reading: &CoordinateReading<P>, restored: bool) -> TestResult {
    let one = [Rational::new(1, 1)?; 2];
    let factors = if restored {
        [one, one]
    } else {
        [oracle::factor(1, 1, true, 56)?, [Rational::new(2, 1)?; 2]]
    };
    for (index, total, error) in [
        (0, [(5, 2), (7, 2)], [(23, 8), (25, 8)]),
        (1, [(3, 16), (5, 16)], [(15, 64), (17, 64)]),
    ] {
        for (bounds, radii) in [(total, reading.totals()), (error, reading.errors())] {
            for ((n, d), factor) in bounds.into_iter().zip(factors[index]) {
                assert!(
                    factor
                        .multiply(Rational::new(n, d)?)?
                        .covered(reading.centers()[index].to_f64(), radii[index].to_f64())?
                );
            }
        }
        assert!(reading.errors()[index] <= reading.totals()[index]);
    }
    assert_eq!(reading.covariance_count(), 3);
    for block in 0..3 {
        let centers = reading.covariance_centers(block).ok_or("covariance")?;
        let errors = reading.covariance_errors(block).ok_or("parameter errors")?;
        for (index, (n, d)) in [(1, 8), (-1, 64), (-1, 64), (1, 16)]
            .into_iter()
            .enumerate()
        {
            let left = if block == 2 { one } else { factors[index % 2] };
            let right = if block == 1 { one } else { factors[index / 2] };
            let original = Rational::new(n, d)?;
            let endpoints = [
                left[0].multiply(right[0])?.multiply(original)?,
                left[1].multiply(right[1])?.multiply(original)?,
            ];
            oracle::covers(endpoints, centers[index].to_f64(), errors[index].to_f64())?;
        }
    }
    Ok(())
}

#[path = "pi_coordinates/costs.rs"]
mod costs;
#[path = "pi_coordinates/lifecycle.rs"]
mod lifecycle;
#[path = "pi_coordinates/retention.rs"]
mod retention;

#[path = "pi_coordinates/point.rs"]
mod point;
