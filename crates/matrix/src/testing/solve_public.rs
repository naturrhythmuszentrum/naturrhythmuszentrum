// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact elimination and backend diagnostics for public solve tests.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::entry;
use crate::solve_testing::TestResult;
use faer::prelude::*;
use nrz_matrix::{Heap, PublishedBundle};
use nrz_scalar::Scalar;

/// Exact rational Gauss-Jordan elimination on independently decoded binary inputs.
pub(crate) fn exact(operator: &[f64], rhs: &[f64], order: usize) -> TestResult<Vec<Rational>> {
    let mut coefficients = operator
        .iter()
        .map(|value| Rational::binary(*value))
        .collect::<TestResult<Vec<_>>>()?;
    let mut result = rhs
        .iter()
        .map(|value| Rational::binary(*value))
        .collect::<TestResult<Vec<_>>>()?;
    for pivot in 0..order {
        let row = (pivot..order)
            .find(|row| coefficients[*row + order * pivot].numerator != 0)
            .ok_or("singular oracle")?;
        for column in 0..order {
            coefficients.swap(pivot + order * column, row + order * column);
        }
        for column in 0..(result.len() / order) {
            result.swap(pivot + order * column, row + order * column);
        }
        eliminate(&mut coefficients, &mut result, order, pivot)?;
    }
    Ok(result)
}

/// Normalize one exact pivot and eliminate it without production scalar arithmetic.
fn eliminate(
    operator: &mut [Rational],
    rhs: &mut [Rational],
    order: usize,
    pivot: usize,
) -> TestResult {
    let divisor = operator[pivot + order * pivot];
    for column in 0..order {
        operator[pivot + order * column] = operator[pivot + order * column].divide(divisor)?;
    }
    for column in 0..(rhs.len() / order) {
        rhs[pivot + order * column] = rhs[pivot + order * column].divide(divisor)?;
    }
    for row in 0..order {
        if row == pivot {
            continue;
        }
        let multiplier = operator[row + order * pivot];
        for column in 0..order {
            operator[row + order * column] = operator[row + order * column]
                .subtract(multiplier.multiply(operator[pivot + order * column])?)?;
        }
        for column in 0..(rhs.len() / order) {
            rhs[row + order * column] = rhs[row + order * column]
                .subtract(multiplier.multiply(rhs[pivot + order * column])?)?;
        }
    }
    Ok(())
}

/// Check every successful center and both companion meanings against exact rational truth.
pub(crate) fn check<P: Scalar + 'static>(
    heap: &Heap,
    bundle: PublishedBundle<P>,
    operator: &[f64],
    rhs: &[f64],
    order: usize,
) -> TestResult {
    let truth = exact(operator, rhs, order)?;
    let backend_operator =
        faer::Mat::<f64>::from_fn(order, order, |row, column| operator[row + order * column]);
    let backend_rhs = faer::Mat::<f64>::from_fn(order, rhs.len() / order, |row, column| {
        rhs[row + order * column]
    });
    let independent = backend_operator.full_piv_lu().solve(&backend_rhs);
    for (index, expected) in truth.into_iter().enumerate() {
        let (center, total, error) = entry(heap, bundle, index)?;
        assert!(expected.covered(center, total)?);
        assert!(expected.covered(center, error)?);
        assert!(expected.covered(independent[(index % order, index / order)], 1.0e-10)?);
    }
    Ok(())
}
