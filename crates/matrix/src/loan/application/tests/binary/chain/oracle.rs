// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact fractions certify each local law and the composed inverse square root.

use super::super::{self as binary, oracle::Rational};
use super::{TestResult, apply, candidate, published};
use crate::{BinaryElementwise, Elementwise, HostChannel};
use std::cmp::Ordering;

/// Three public selected loads observe the published readings without calling a numerical law.
pub(super) fn readings(
    tile: &crate::PublishedTile<f64>,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
    cell: usize,
    component: usize,
) -> TestResult<[f64; 3]> {
    let reader = tile.reader(heap, account)?;
    Ok([
        reader.read_component(HostChannel::Center, cell, component, account)?,
        reader.read_component(HostChannel::Total, cell, component, account)?,
        reader.read_component(HostChannel::Error, cell, component, account)?,
    ])
}

/// Preserve each immediate published triple before its sole pin moves into the next request.
pub(super) fn snapshot(
    tile: &crate::PublishedTile<f64>,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
    cells: usize,
    components: usize,
) -> TestResult<Vec<Vec<[f64; 3]>>> {
    (0..cells)
        .map(|cell| {
            (0..components)
                .map(|component| readings(tile, heap, account, cell, component))
                .collect()
        })
        .collect()
}

/// Immediate nominal and whole-box images use exact fractions, never an ancestor's error.
/// These positive boxes make the square's endpoint hull equal to the four-corner product hull.
pub(super) fn check(input: [f64; 3], output: [f64; 3], function: Elementwise) -> TestResult {
    assert_eq!(
        Rational::binary(input[0])?
            .sub(&Rational::binary(input[1])?)
            .compare(&Rational::binary(0.0)?),
        Ordering::Greater
    );
    match function {
        Elementwise::Square => binary::oracle::check(
            [input[0]; 2],
            [input[1]; 2],
            BinaryElementwise::Product,
            output,
        ),
        Elementwise::SquareRoot => root(input, output),
        Elementwise::Reciprocal => binary::oracle::check(
            [1.0, input[0]],
            [0.0, input[1]],
            BinaryElementwise::Quotient,
            output,
        ),
    }
}

/// Squared rational inequalities certify the bracket and exact midpoint ties certify rounding.
pub(super) fn root(input: [f64; 3], output: [f64; 3]) -> TestResult {
    let [center, total, error] = output;
    assert!(center.is_finite() && total.is_finite() && error.is_finite());
    assert!(center > 0.0 && error >= 0.0 && error <= total);
    let x = Rational::binary(input[0])?;
    let y = Rational::binary(center)?;
    let zero = Rational::binary(0.0)?;
    for (source_radius, output_radius) in [(input[1], total), (0.0, error)] {
        let r = Rational::binary(source_radius)?;
        let s = Rational::binary(output_radius)?;
        let lower = y.sub(&s);
        let upper = y.add(&s);
        if lower.compare(&zero) == Ordering::Greater {
            assert_ne!(
                lower
                    .image(&lower, BinaryElementwise::Product)?
                    .compare(&x.sub(&r)),
                Ordering::Greater
            );
        }
        assert_ne!(
            upper
                .image(&upper, BinaryElementwise::Product)?
                .compare(&x.add(&r)),
            Ordering::Less
        );
    }
    for (bits, expected) in [
        (center.to_bits() - 1, Ordering::Less),
        (center.to_bits() + 1, Ordering::Greater),
    ] {
        let adjacent = Rational::binary(f64::from_bits(bits))?;
        let midpoint = y
            .add(&adjacent)
            .image(&Rational::binary(2.0)?, BinaryElementwise::Quotient)?;
        let ordering = midpoint
            .image(&midpoint, BinaryElementwise::Product)?
            .compare(&x);
        assert!(ordering == expected || (ordering == Ordering::Equal && center.to_bits() % 2 == 0));
    }
    Ok(())
}

/// Enumerate the rectangle's four rational corner products, without rounded endpoint operations.
pub(super) fn rectangle(centers: [f64; 2], radii: [f64; 2]) -> TestResult<[Rational; 2]> {
    let left = Rational::binary(centers[0])?;
    let right = Rational::binary(centers[1])?;
    let lr = Rational::binary(radii[0])?;
    let rr = Rational::binary(radii[1])?;
    let mut lower = left.image(&right, BinaryElementwise::Product)?;
    let mut upper = lower.clone();
    for x in [left.sub(&lr), left.add(&lr)] {
        for y in [right.sub(&rr), right.add(&rr)] {
            let value = x.image(&y, BinaryElementwise::Product)?;
            if value.compare(&lower) == Ordering::Less {
                lower = value.clone();
            }
            if value.compare(&upper) == Ordering::Greater {
                upper = value;
            }
        }
    }
    Ok([lower, upper])
}

/// Inverse-root containment is equivalent to two squared rational inequalities on positive boxes.
fn composed(range: &[Rational; 2], center: f64, radius: f64) -> TestResult<bool> {
    let middle = Rational::binary(center)?;
    let spread = Rational::binary(radius)?;
    let lower = middle.sub(&spread);
    let upper = middle.add(&spread);
    let one = Rational::binary(1.0)?;
    let high = range[0].image(
        &upper.image(&upper, BinaryElementwise::Product)?,
        BinaryElementwise::Product,
    )?;
    let low = range[1].image(
        &lower.image(&lower, BinaryElementwise::Product)?,
        BinaryElementwise::Product,
    )?;
    Ok(upper.compare(&Rational::binary(0.0)?) == Ordering::Greater
        && high.compare(&one) != Ordering::Less
        && (lower.compare(&Rational::binary(0.0)?) != Ordering::Greater
            || low.compare(&one) != Ordering::Greater))
}

/// Product corners, irrational root rounding, immediate reciprocal error and composed bounds differ.
/// ´test:unit:chain-independent-composed-rational-oracle´
#[test]
fn chain_independent_composed_rational_oracle() -> TestResult {
    let mut local_is_not_global = false;
    for centers in [
        [2.0, 3.0],
        [1.1, 1.3],
        [5.0, 7.0],
        [3.0, 11.0],
        [f64::from_bits(0x3ff0_0000_0000_0001), 1.0],
    ] {
        for radii in [[0.0; 2], [0.125, 0.25]] {
            local_is_not_global |= numerical_case(centers, radii)?;
        }
    }
    assert!(
        local_is_not_global,
        "final local nominal error is not the whole-expression rounding error"
    );
    Ok(())
}

/// Separate exact product, square-root and division oracles certify one public three-step chain.
fn numerical_case(centers: [f64; 2], radii: [f64; 2]) -> TestResult<bool> {
    let binary::Fixture {
        mut heap,
        mut account,
        request,
    } = binary::custom::<f64>(
        1,
        1,
        [3; 2],
        2,
        false,
        BinaryElementwise::Product,
        nrz_bindings::Limits::default(),
        |operand, _, _| (centers[operand], radii[operand]),
    )?;
    let owner = binary::ready(heap.request_pointwise_product(request, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    let (product, _) = published(heap.publish_application(owner, &mut account))?;
    let p = readings(&product, &heap, &mut account, 0, 0)?;
    binary::oracle::check(centers, radii, BinaryElementwise::Product, p)?;
    let square_root = apply(
        &mut heap,
        product.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let s = readings(&square_root, &heap, &mut account, 0, 0)?;
    root(p, s)?;
    let reciprocal = apply(
        &mut heap,
        square_root.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut account,
    )?;
    let r = readings(&reciprocal, &heap, &mut account, 0, 0)?;
    binary::oracle::check([1.0, s[0]], [0.0, s[1]], BinaryElementwise::Quotient, r)?;
    assert!(composed(&rectangle(centers, radii)?, r[0], r[1])?);
    Ok(radii.map(f64::to_bits) == [0; 2] && !composed(&rectangle(centers, radii)?, r[0], r[2])?)
}
