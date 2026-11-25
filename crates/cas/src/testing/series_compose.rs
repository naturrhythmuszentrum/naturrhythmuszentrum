// SPDX-License-Identifier: AGPL-3.0-only
//! Direct multinomial expansion supplies independent composition expectations.

use super::common::{TestResult, default_budget};
use super::series_proof::{Fixture, object, rational, step};
use crate::test_api::Rational;
use crate::test_api::proof::{Judgment, Statement};
use crate::test_api::series::{
    SeriesCertificate as C, SeriesPowerRow, SeriesProductRow, SeriesStatement as S,
};

/// Enumerate ordered monomials directly, independently of successive-power convolution.
fn monomial(q: &[Rational], exponent: usize, degree: usize) -> TestResult<Rational> {
    if exponent == 0 {
        return rational(if degree == 0 { "1" } else { "0" });
    }
    let mut sum = Rational::zero();
    let width = q.len();
    let count = width
        .checked_pow(u32::try_from(exponent)?)
        .ok_or("oracle extent")?;
    for mut ordinal in 0..count {
        let mut total_degree = 0;
        let mut product = Rational::one();
        for _ in 0..exponent {
            let index = ordinal % width;
            ordinal /= width;
            total_degree += index;
            product = product.mul(&q[index], &mut default_budget()?)?;
        }
        if total_degree == degree {
            sum = sum.add(&product, &mut default_budget()?)?;
        }
    }
    Ok(sum)
}

/// Explicit complete evidence uses direct monomial powers, never a production recurrence.
pub(crate) fn certificate(
    p: &[&str],
    q: &[&str],
    order: usize,
    default_order: bool,
) -> TestResult<C> {
    let p = p
        .iter()
        .map(|s| rational(s))
        .collect::<TestResult<Vec<_>>>()?;
    let q = q
        .iter()
        .map(|s| rational(s))
        .collect::<TestResult<Vec<_>>>()?;
    let mut powers = Vec::new();
    let mut seed = Vec::new();
    for degree in 0..order {
        seed.push(monomial(&q, 0, degree)?);
    }
    for exponent in 1..order {
        for degree in 0..order {
            let mut sum = Rational::zero();
            for index in 0..=degree {
                let left = monomial(&q, exponent - 1, index)?;
                let right = q[degree - index].clone();
                let product = left.mul(&right, &mut default_budget()?)?;
                sum = sum.add(&product, &mut default_budget()?)?;
                powers.push(SeriesPowerRow {
                    exponent: u32::try_from(exponent)?,
                    row: SeriesProductRow {
                        degree: u32::try_from(degree)?,
                        left_index: u32::try_from(index)?,
                        right_index: u32::try_from(degree - index)?,
                        left,
                        right,
                        product,
                        sum: sum.clone(),
                    },
                });
            }
        }
    }
    let mut sums = Vec::new();
    for degree in 0..order {
        let mut sum = Rational::zero();
        for (exponent, left) in p.iter().take(order).enumerate() {
            let right = monomial(&q, exponent, degree)?;
            let product = left.mul(&right, &mut default_budget()?)?;
            sum = sum.add(&product, &mut default_budget()?)?;
            sums.push(SeriesProductRow {
                degree: u32::try_from(degree)?,
                left_index: u32::try_from(exponent)?,
                right_index: u32::try_from(degree)?,
                left: left.clone(),
                right,
                product,
                sum: sum.clone(),
            });
        }
    }
    Ok(C::Compose {
        form: 0,
        order: u32::try_from(order)?,
        default_order,
        zero: Rational::zero(),
        seed,
        powers,
        sums,
    })
}

/// Preserve original orders, with ordered explicit restrictions when requested.
pub(crate) fn fixture(
    p: &[&str],
    q: &[&str],
    expected: &[&str],
    default_order: bool,
    reordered: bool,
) -> TestResult<Fixture> {
    let mut f = super::series_proof::fixture(0, reordered)?;
    f.steps.clear();
    let left = object(&mut f.base, &f.field, p)?;
    let right = object(&mut f.base, &f.field, q)?;
    let output = object(&mut f.base, &f.field, expected)?;
    let n = expected.len();
    let mut premises = Vec::new();
    for (input, values) in [(&left, p), (&right, q)] {
        if values.len() > n {
            let output = object(&mut f.base, &f.field, &values[..n])?;
            premises.push(u32::try_from(f.steps.len())?);
            f.steps.push(step(
                &f.base,
                S::Truncate {
                    input: input.clone(),
                    output,
                },
                C::Truncate {
                    form: 0,
                    coefficients: values[..n]
                        .iter()
                        .map(|s| rational(s))
                        .collect::<TestResult<Vec<_>>>()?,
                },
                Vec::new(),
            ));
        }
    }
    let statement = S::Compose {
        left,
        right,
        output,
        order: u32::try_from(n)?,
        default_order,
    };
    let root = step(
        &f.base,
        statement,
        certificate(p, q, n, default_order)?,
        premises,
    );
    f.expected = Judgment {
        statement: root.conclusion.clone(),
        ..f.expected
    };
    f.steps.push(root);
    Ok(f)
}

/// Extract the exact operation without depending on result syntax in a production request.
pub(crate) fn operation(f: &Fixture) -> TestResult<crate::test_api::series::SeriesOperation> {
    let Statement::Series(value) = &f.expected.statement else {
        return Err("series".into());
    };
    let S::Compose {
        left,
        right,
        order,
        default_order,
        ..
    } = value.as_ref()
    else {
        return Err("compose".into());
    };
    Ok(crate::test_api::series::SeriesOperation::Compose {
        left: left.clone(),
        right: right.clone(),
        order: if *default_order { None } else { Some(*order) },
    })
}

/// Resource families distinguish absent output, two owned slots and order one.
pub(crate) fn fixture_kind(kind: usize, reordered: bool) -> TestResult<Fixture> {
    match kind {
        13 => fixture(
            &["0", "1", "1", "0"],
            &["0", "1", "-1", "0"],
            &["0", "1", "0", "-2"],
            true,
            reordered,
        ),
        14 => fixture(
            &["0", "1", "1"],
            &["0", "1", "1"],
            &["0", "1"],
            false,
            reordered,
        ),
        15 => fixture(&["7"], &["0"], &["7"], true, reordered),
        _ => Err("composition resource kind".into()),
    }
}

/// Admit originals only; result syntax and result-only rational literals remain absent.
pub(crate) fn production(kind: usize) -> TestResult<super::series_proof::Production> {
    use crate::test_api::series::SeriesOperation as O;
    let mut f = super::series_proof::production(0)?;
    let O::Truncate { input, .. } = &f.operation else {
        return Err("baseline".into());
    };
    let field = f
        .base
        .cas
        .series_object(input, &mut crate::testing::budgets::graph_budget()?)?
        .field
        .clone();
    let (p, q, order) = match kind {
        13 => (&["0", "1", "1", "0"][..], &["0", "1", "-1", "0"][..], None),
        14 => (&["0", "1", "1"][..], &["0", "1", "1"][..], Some(2)),
        15 => (&["7"][..], &["0"][..], None),
        _ => return Err("composition production kind".into()),
    };
    let left = object(&mut f.base, &field, p)?;
    let right = object(&mut f.base, &field, q)?;
    if let Some(n) = order {
        for input in [&left, &right] {
            f.proofs.push(
                f.base
                    .cas
                    .compute_series(
                        &O::Truncate {
                            input: input.clone(),
                            order: n,
                        },
                        &[],
                        &mut default_budget()?,
                    )?
                    .proof()
                    .clone(),
            );
        }
    }
    f.operation = O::Compose { left, right, order };
    Ok(f)
}
