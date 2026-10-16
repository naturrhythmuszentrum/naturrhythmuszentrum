// SPDX-License-Identifier: AGPL-3.0-only
//! Small committed Hilbert, Kahan, Wilkinson and scale fixtures have independent exact truth.

use crate::kernel_testing::{execute, output};
use crate::solve_testing::{TestResult, draft};
use nrz_matrix::{Extent, Heap, HeapLimits, Layout};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Every accepted adversarial fixture is checked against rational elimination and separate faer work.
/// ´test:integration:adversarial-square-solves-pass-independent-oracles-in-both-formats´
#[test]
fn adversarial_square_solves_pass_independent_oracles_in_both_formats() -> TestResult {
    corpus::<f32>()?;
    corpus::<f64>()
}

/// Two-by-two Hilbert and Kahan plus a three-by-three Wilkinson growth fixture.
fn corpus<P: Scalar + 'static>() -> TestResult {
    let half = P::ONE / P::from_u8(2);
    let hilbert = [P::ONE, half, half, P::ONE / P::from_u8(3)];
    one(&hilbert, &[P::ONE; 2], 2)?;
    let kahan = [
        P::ONE,
        P::ZERO,
        -(P::from_u8(3) / P::from_u8(5)),
        P::from_u8(4) / P::from_u8(5),
    ];
    one(&kahan, &[P::ONE; 2], 2)?;
    let wilkinson = [
        P::ONE,
        -P::ONE,
        -P::ONE,
        P::ZERO,
        P::ONE,
        -P::ONE,
        P::ONE,
        P::ONE,
        P::ONE,
    ];
    one(&wilkinson, &[P::ONE, P::from_u8(2), P::from_u8(3)], 3)?;
    let large = (0..40).fold(P::ONE, |value, _| value * P::from_u8(2));
    let small = P::ONE / large;
    one(&[small, P::ZERO, P::ZERO, large], &[small, large], 2)
}

/// Exact inputs are decoded from the actual execution precision, including Hilbert thirds.
fn one<P: Scalar + 'static>(operator: &[P], rhs: &[P], order: usize) -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let first =
        heap.import_represented(Extent::new(order, order)?, &Descriptor::ratio(), operator)?;
    let second = heap.import_represented(
        Extent::new(order, rhs.len() / order)?,
        &Descriptor::ratio(),
        rhs,
    )?;
    let outcome = execute(
        &mut heap,
        draft(order, rhs.len() / order)?,
        vec![first.value(), second.value()],
    )?;
    let coefficients = operator
        .iter()
        .map(|value| value.to_f64())
        .collect::<Vec<_>>();
    let right = rhs.iter().map(|value| value.to_f64()).collect::<Vec<_>>();
    crate::solve_public::check(&heap, output(&outcome)?, &coefficients, &right, order)
}

/// Negative strides and repeated RHS gathers retain logical multiplicity through packing and proof.
/// ´test:integration:solve-packs-reversed-operator-and-repeated-rhs-views´
#[test]
fn solve_packs_reversed_operator_and_repeated_rhs_views() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[4.0_f64, 0.0, 0.0, 2.0],
    )?;
    let second =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 8.0])?;
    let operator = heap.alias(
        first.value(),
        &Layout::strided(Extent::new(2, 2)?, 4, 3, -1, -2)?,
    )?;
    let rhs = heap.alias(
        second.value(),
        &Layout::gather(Extent::new(2, 2)?, 2, &[0, 1, 0, 1], 4)?,
    )?;
    let outcome = execute(&mut heap, draft(2, 2)?, vec![operator, rhs])?;
    crate::solve_public::check(
        &heap,
        output(&outcome)?,
        &[2.0, 0.0, 0.0, 4.0],
        &[2.0, 8.0, 2.0, 8.0],
        2,
    )
}
