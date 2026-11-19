// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational inequalities use the independent public path's actual tau, with no kernel hook.
use super::{Scalar, TestResult};
use crate::loan::application::tests::binary::oracle::Rational;
use crate::{BinaryElementwise, LcInputReaders, LcOmegaPrepared, LcRequestedState};
use std::cmp::Ordering;
type ExpectedCell<P> = (usize, [u16; 3], Option<[P; 3]>);

pub(super) fn captured<P: Scalar + 'static>(
    prepared: &LcOmegaPrepared<'_, P, 5>,
    cells: &[usize],
    positions: [usize; 2],
    account: &mut crate::ExecutionAccount,
) -> TestResult<Vec<ExpectedCell<P>>> {
    let product = LcInputReaders::captured(
        prepared.declaration.inventory(),
        &prepared.receipt.closure.rows,
    )?
    .into_cells(account)?
    .into_product(positions, account)?;
    let mut expected = Vec::new();
    for cell in cells {
        let observed =
            product.read_requested(*cell, crate::LcQuantityDemand::Omega, None, account)?;
        let LcRequestedState::Ready {
            tau,
            rate: Some(rate),
            values,
        } = observed.state()
        else {
            return Err("positive reference chain".into());
        };
        assert_eq!(values.omega, Some(rate));
        let [left, right] =
            positions.map(|position| observed.inputs().readings()[position].values());
        enclosure(left, right, tau, rate)?;
        expected.push((*cell, [2, 0, 0], Some(rate)));
    }
    Ok(expected)
}
/// The total check composes exact input endpoints; the local check uses the actual supplied tau center.
pub(super) fn enclosure<P: Scalar>(
    left: [P; 3],
    right: [P; 3],
    tau: [P; 3],
    rate: [P; 3],
) -> TestResult {
    let left = [
        Rational::binary(left[0].to_f64())?,
        Rational::binary(left[1].to_f64())?,
    ];
    let right = [
        Rational::binary(right[0].to_f64())?,
        Rational::binary(right[1].to_f64())?,
    ];
    let low = left[0]
        .sub(&left[1])
        .image(&right[0].sub(&right[1]), BinaryElementwise::Product)?;
    let high = left[0]
        .add(&left[1])
        .image(&right[0].add(&right[1]), BinaryElementwise::Product)?;
    root(tau, &low, &high)?;
    let center = Rational::binary(rate[0].to_f64())?;
    let total = Rational::binary(rate[1].to_f64())?;
    let lower = center.sub(&total);
    let upper = center.add(&total);
    let one = Rational::fraction(1, 1)?;
    assert_eq!(lower.compare(&Rational::fraction(0, 1)?), Ordering::Greater);
    assert_ne!(
        lower
            .image(&lower, BinaryElementwise::Product)?
            .image(&high, BinaryElementwise::Product)?
            .compare(&one),
        Ordering::Greater
    );
    assert_ne!(
        upper
            .image(&upper, BinaryElementwise::Product)?
            .image(&low, BinaryElementwise::Product)?
            .compare(&one),
        Ordering::Less
    );
    let reciprocal = one.image(
        &Rational::binary(tau[0].to_f64())?,
        BinaryElementwise::Quotient,
    )?;
    let error = Rational::binary(rate[2].to_f64())?;
    assert_ne!(center.sub(&error).compare(&reciprocal), Ordering::Greater);
    assert_ne!(center.add(&error).compare(&reciprocal), Ordering::Less);
    assert_ne!(error.compare(&total), Ordering::Greater);
    Ok(())
}
fn root<P: Scalar>(tau: [P; 3], low: &Rational, high: &Rational) -> TestResult {
    let center = Rational::binary(tau[0].to_f64())?;
    let total = Rational::binary(tau[1].to_f64())?;
    let lower = center.sub(&total);
    let upper = center.add(&total);
    assert_eq!(lower.compare(&Rational::fraction(0, 1)?), Ordering::Greater);
    assert_ne!(
        lower
            .image(&lower, BinaryElementwise::Product)?
            .compare(low),
        Ordering::Greater
    );
    assert_ne!(
        upper
            .image(&upper, BinaryElementwise::Product)?
            .compare(high),
        Ordering::Less
    );
    Ok(())
}
