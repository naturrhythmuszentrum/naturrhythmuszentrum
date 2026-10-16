// SPDX-License-Identifier: AGPL-3.0-only
//! Rational targets use exact elimination and integer comparisons, never production enclosures.

use super::support::{Parameter, ResultOf, Sample};
use crate::kernel_oracle::Rational;

/// Check a returned parameter's complete region against independently derived fractions.
pub(super) fn parameter(actual: &Parameter, expected: &[Rational]) -> ResultOf {
    assert_eq!(actual.shape[0] * actual.shape[1], expected.len());
    assert_eq!(actual.centers.len(), expected.len());
    assert_eq!(actual.errors.len(), expected.len());
    assert_eq!(actual.dimensions.len(), expected.len());
    for ((truth, &center), &error) in expected.iter().zip(&actual.centers).zip(&actual.errors) {
        assert!(
            crate::statistics_oracle::covered(*truth, center, error)?,
            "rational {truth:?} outside parameter {center:?} +/- {error:?}"
        );
    }
    Ok(())
}

/// The total and numerical-only regions target different exact problems when imports are inexact.
pub(super) fn values(actual: &Sample, nominal: &[Rational], family: &[Rational]) -> ResultOf {
    assert_eq!(actual.centers.len(), nominal.len());
    assert_eq!(actual.totals.len(), family.len());
    assert_eq!(actual.errors.len(), nominal.len());
    for (index, &center) in actual.centers.iter().enumerate() {
        assert!(
            crate::statistics_oracle::covered(nominal[index], center, actual.errors[index])?,
            "{} entry {index}: local exact {:?}, center {center:?}, evaluation {:?}",
            actual.name,
            nominal[index],
            actual.errors[index]
        );
        assert!(
            crate::statistics_oracle::covered(family[index], center, actual.totals[index])?,
            "{} entry {index}: family exact {:?}, center {center:?}, total {:?}",
            actual.name,
            family[index],
            actual.totals[index]
        );
    }
    Ok(())
}

/// Independent elimination supplies the inverse and exact represented nominal solution.
pub(super) fn jacobian(operator: &[f64], rhs: &[f64], order: usize) -> ResultOf<Vec<Rational>> {
    let mut identity = vec![0.0; order * order];
    for i in 0..order {
        identity[i + order * i] = 1.0;
    }
    let inverse = crate::solve_public::exact(operator, &identity, order)?;
    let nominal = crate::solve_public::exact(operator, rhs, order)?;
    let mut result = Vec::new();
    for direction in 0..(operator.len() + rhs.len()) {
        for (entry, _) in nominal.iter().enumerate() {
            let row = entry % order;
            let column = entry / order;
            let value = if direction < operator.len() {
                inverse[row + order * (direction % order)]
                    .multiply(nominal[direction / order + order * column])?
                    .multiply(Rational::new(-1, 1)?)?
            } else if (direction - operator.len()) / order == column {
                inverse[row + order * ((direction - operator.len()) % order)]
            } else {
                Rational::new(0, 1)?
            };
            result.push(value);
        }
    }
    Ok(result)
}

/// Exact rank-one covariance retains every sign and off-diagonal entry.
pub(super) fn outer(response: &[Rational], weight: Rational) -> ResultOf<Vec<Rational>> {
    let mut result = Vec::new();
    for &column in response {
        for &row in response {
            result.push(row.multiply(column)?.multiply(weight)?);
        }
    }
    Ok(result)
}

/// Literal numerator/denominator pairs are independent from the production constant decoder.
pub(super) fn fractions(values: &[(i128, i128)]) -> ResultOf<Vec<Rational>> {
    values.iter().map(|&(n, d)| Rational::new(n, d)).collect()
}

/// A later operation's numerical-only target starts from its actual represented operand centers.
pub(super) fn represented(values: &[f64]) -> ResultOf<Vec<Rational>> {
    values
        .iter()
        .map(|&value| Rational::binary(value))
        .collect()
}
