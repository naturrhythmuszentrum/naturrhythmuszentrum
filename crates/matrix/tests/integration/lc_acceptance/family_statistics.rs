// SPDX-License-Identifier: AGPL-3.0-only
//! Original rational data certify every operator/root block, including irrational cross magnitudes.

use super::{family_support as family, reciprocal_statistics as shared, root_support as inputs};
use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{MomentAssurance, ReciprocalDerivativeContract};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// Every covariance coordinate is checked at all four original corners and the nominal point.
/// ´test:integration:lc-operator-root-family-encloses-exact-corners´
#[test]
fn lc_operator_root_family_encloses_exact_corners() -> TestResult {
    for (references, scaling) in [(["5", "1/10", "1/2000"], 1), (["10", "1/5", "1/1000"], 2)] {
        let mut budget = shared::policy()?;
        let mut fixture =
            Fixture::with_scaled_output_store_limits(references, budget.limits().clone())?;
        let contract = shared::derivative(&mut fixture, &mut budget)?;
        corners::<f32>(&fixture, &contract, scaling, &mut budget)?;
        corners::<f64>(&fixture, &contract, scaling, &mut budget)?;
    }
    Ok(())
}

fn corners<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    scaling: i64,
    budget: &mut Budget,
) -> TestResult {
    for point in [(15, 15), (15, 17), (17, 15), (17, 17), (16, 16)] {
        let centers = [
            P::from_u8(point.0) / P::from_u8(64),
            P::from_u8(point.1) / (P::from_u8(128) * P::from_u8(2)),
        ];
        let coefficients = [
            P::ONE / P::from_u8(128),
            P::ONE / (P::from_u8(128) * P::from_u8(4)),
        ];
        let mut case = inputs::prepare(
            fixture,
            contract,
            centers.map(|value| [value, P::ZERO, P::ZERO]),
            coefficients,
            true,
            budget,
        )?;
        let outputs = family::publish(fixture, &mut case, budget)?;
        let context = ArithmeticContext::new()?;
        let values = case.heap.lc_operator_root_covariance(
            outputs.moments,
            &outputs.roots,
            &context,
            budget,
        )?;
        oracle(&values, [i64::from(point.0), i64::from(point.1)], scaling)?;
        assert_eq!(
            case.heap.moments(outputs.moments)?.assurance(),
            MomentAssurance::LinearizedMoments
        );
        assert_eq!(
            outputs.roots.evaluation().moment_assurance(),
            Some(MomentAssurance::LinearizedMoments)
        );
        assert!(case.heap.moments(outputs.moments)?.means().is_none());
        drop(case.original);
        drop(case.selected);
        drop(case.derivative);
        case.heap.compact()?;
        oracle(
            &case.heap.lc_operator_root_covariance(
                outputs.moments,
                &outputs.roots,
                &context,
                budget,
            )?,
            [i64::from(point.0), i64::from(point.1)],
            scaling,
        )?;
    }
    Ok(())
}

/// Original physical coefficients give `dB10=t*V/I*cL/L²` and `dB01=-t*I/V*cC/C²`.
fn oracle<P: Scalar>(values: &[[Enclosure<P>; 8]; 8], point: [i64; 2], scaling: i64) -> TestResult {
    let inductance = Rational::new(point[0], 64)?;
    let capacitance = Rational::new(point[1], 256)?;
    let lower = Rational::new(scaling, 5120)?.divide(&inductance.multiply(&inductance)?)?;
    let upper =
        Rational::new(-scaling, 51_200_000)?.divide(&capacitance.multiply(&capacitance)?)?;
    let slopes = [Rational::new(0, 1)?, lower, upper, Rational::new(0, 1)?];
    let relative = Rational::new(1, 128)?
        .divide(&inductance)?
        .add(&Rational::new(1, 512)?.divide(&capacitance)?)?;
    let variance = relative.multiply(&relative)?.divide(
        &Rational::new(4, 1)?
            .multiply(&inductance)?
            .multiply(&capacitance)?,
    )?;
    for (row, entries) in values.iter().enumerate() {
        for (column, &entry) in entries.iter().enumerate() {
            check(entry, [row, column], &slopes, &variance)?;
        }
    }
    Ok(())
}

/// Cross-block magnitude squares are rational even at the two mixed corners with irrational roots.
fn check<P: Scalar>(
    value: Enclosure<P>,
    indices: [usize; 2],
    slopes: &[Rational; 4],
    variance: &Rational,
) -> TestResult {
    let [row, column] = indices;
    if row < 4 && column < 4 {
        contained(value, &slopes[row].multiply(&slopes[column])?)?;
    } else if row >= 4 && column >= 4 {
        let sign = if row % 2 == 0 || column % 2 == 0 {
            0
        } else if row == column {
            1
        } else {
            -1
        };
        contained(value, &variance.multiply(&Rational::new(sign, 1)?)?)?;
    } else {
        let operator = row.min(column);
        let root = row.max(column);
        if operator == 0 || operator == 3 || root % 2 == 0 {
            contained(value, &Rational::new(0, 1)?)?;
        } else {
            let square = slopes[operator]
                .multiply(&slopes[operator])?
                .multiply(variance)?;
            let positive = (operator == 2) == (root == 5);
            squared(value, &square, positive)?;
        }
    }
    Ok(())
}

fn contained<P: Scalar>(value: Enclosure<P>, expected: &Rational) -> TestResult {
    assert!(expected.enclosed(value.lower().get().to_f64(), value.upper().get().to_f64())?);
    Ok(())
}

/// Sign and exact squared binary endpoints establish containment without numerical square roots.
fn squared<P: Scalar>(value: Enclosure<P>, expected: &Rational, positive: bool) -> TestResult {
    let (lower, upper) = if positive {
        (value.lower().get().to_f64(), value.upper().get().to_f64())
    } else {
        (-value.upper().get().to_f64(), -value.lower().get().to_f64())
    };
    assert!(lower > 0.0 && upper >= lower);
    let lower = Rational::binary(lower)?;
    let upper = Rational::binary(upper)?;
    assert!(lower.multiply(&lower)?.at_most(expected)?);
    assert!(expected.at_most(&upper.multiply(&upper)?)?);
    Ok(())
}
