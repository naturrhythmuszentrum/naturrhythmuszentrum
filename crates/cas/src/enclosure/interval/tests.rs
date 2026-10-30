// SPDX-License-Identifier: AGPL-3.0-only
//! Small integer cross-products are independent of production endpoint arithmetic.

use super::*;
use crate::budget::Limits;

fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

fn interval(lo: i64, hi: i64, denominator: u64) -> Result<Interval, Fault> {
    let mut budget = ledger()?;
    Interval::new(
        Rational::from_ratio(lo, denominator, &mut budget)?,
        Rational::from_ratio(hi, denominator, &mut budget)?,
        &mut budget,
    )
}

type EndpointPair = ((i64, u64), (i64, u64));

fn bounds(value: &Interval) -> Result<EndpointPair, Box<dyn std::error::Error>> {
    let mut budget = ledger()?;
    Ok((
        value.lower.small_ratio(&mut budget)?.ok_or("small lower")?,
        value.upper.small_ratio(&mut budget)?.ok_or("small upper")?,
    ))
}

fn contains_ratio(value: &Interval, n: i64, d: u64) -> Result<(), Box<dyn std::error::Error>> {
    let (lo, hi) = bounds(value)?;
    assert!(i128::from(lo.0) * i128::from(d) <= i128::from(n) * i128::from(lo.1));
    assert!(i128::from(hi.0) * i128::from(d) >= i128::from(n) * i128::from(hi.1));
    Ok(())
}

fn exact_grid(
    value: &Interval,
    lo: i64,
    hi: i64,
    d: i64,
    q: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let scale = 1_i64 << q;
    let lower = (lo * scale).div_euclid(d);
    let upper = -((-hi * scale).div_euclid(d));
    let (a, b) = bounds(value)?;
    assert_eq!(
        i128::from(a.0) * i128::from(scale),
        i128::from(lower) * i128::from(a.1)
    );
    assert_eq!(
        i128::from(b.0) * i128::from(scale),
        i128::from(upper) * i128::from(b.1)
    );
    Ok(())
}

/// Signed quotient/remainder oracles cover every sign and exact grid boundaries.
/// ´test:unit:enclosure-grid-integer-oracle´
/// ´claim:cas:enclosure-grid-integer-oracle´
#[test]
fn enclosure_grid_integer_oracle() -> Result<(), Box<dyn std::error::Error>> {
    for n in -9_i64..=9 {
        for d in 1_u64..=7 {
            for q in 0..=4 {
                let value = interval(n, n, d)?;
                let rounded = value.coarsen(q, &mut ledger()?)?;
                let scale = 1_i64 << q;
                let denominator = i64::try_from(d)?;
                let lower = (n * scale).div_euclid(denominator);
                let upper = -((-n * scale).div_euclid(denominator));
                let (actual_lo, actual_hi) = bounds(&rounded)?;
                assert_eq!(
                    i128::from(actual_lo.0) * i128::from(scale),
                    i128::from(lower) * i128::from(actual_lo.1)
                );
                assert_eq!(
                    i128::from(actual_hi.0) * i128::from(scale),
                    i128::from(upper) * i128::from(actual_hi.1)
                );
                assert_eq!(rounded.coarsen(q, &mut ledger()?)?, rounded);
                assert!(rounded.contains(&value, &mut ledger()?)?);
                let finer = value.coarsen(q + 1, &mut ledger()?)?;
                assert!(rounded.contains(&finer, &mut ledger()?)?);
            }
        }
    }
    Ok(())
}

/// Enumerated integer cross-products independently check the extrema of binary operations.
/// ´test:unit:enclosure-arithmetic-extrema-oracle´
/// ´claim:cas:enclosure-arithmetic-extrema-oracle´
#[test]
fn enclosure_arithmetic_extrema_oracle() -> Result<(), Box<dyn std::error::Error>> {
    for a in -3..=3 {
        for b in a..=3 {
            for c in -3..=3 {
                for d in c..=3 {
                    let left = interval(a, b, 2)?;
                    let right = interval(c, d, 3)?;
                    let sum = left.add(&right, 5, &mut ledger()?)?;
                    let difference = left.sub(&right, 5, &mut ledger()?)?;
                    let product = left.mul(&right, 5, &mut ledger()?)?;
                    let maximum = left.maximum(&right, 5, &mut ledger()?)?;
                    for x in [a, b] {
                        for y in [c, d] {
                            contains_ratio(&sum, 3 * x + 2 * y, 6)?;
                            contains_ratio(&difference, 3 * x - 2 * y, 6)?;
                            contains_ratio(&product, x * y, 6)?;
                            contains_ratio(&maximum, (3 * x).max(2 * y), 6)?;
                        }
                    }
                    exact_grid(&sum, 3 * a + 2 * c, 3 * b + 2 * d, 6, 5)?;
                    exact_grid(&difference, 3 * a - 2 * d, 3 * b - 2 * c, 6, 5)?;
                    let products = [a * c, a * d, b * c, b * d];
                    exact_grid(
                        &product,
                        *products.iter().min().ok_or("min")?,
                        *products.iter().max().ok_or("max")?,
                        6,
                        5,
                    )?;
                    exact_grid(&maximum, (3 * a).max(2 * c), (3 * b).max(2 * d), 6, 5)?;
                    let absolute = left.abs(5, &mut ledger()?)?;
                    exact_grid(
                        &absolute,
                        if a <= 0 && b >= 0 {
                            0
                        } else {
                            a.abs().min(b.abs())
                        },
                        a.abs().max(b.abs()),
                        2,
                        5,
                    )?;
                    contains_ratio(&absolute, a.abs(), 2)?;
                    contains_ratio(&absolute, b.abs(), 2)?;
                    if a <= 0 && b >= 0 {
                        contains_ratio(&absolute, 0, 1)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Divisor separation, interval validation and compatible intersection refuse bad inputs.
/// ´test:unit:enclosure-interval-refusals´
/// ´claim:cas:enclosure-interval-refusals´
#[test]
fn enclosure_interval_refusals() -> Result<(), Box<dyn std::error::Error>> {
    assert!(interval(2, 1, 3).is_err());
    for (lo, hi) in [(-1, 1), (0, 1), (-1, 0), (0, 0)] {
        assert!(interval(lo, hi, 1)?.reciprocal(4, &mut ledger()?).is_err());
    }
    for (lo, hi) in [(1, 3), (-3, -1)] {
        let inverse = interval(lo, hi, 2)?.reciprocal(5, &mut ledger()?)?;
        for endpoint in [lo, hi] {
            contains_ratio(
                &inverse,
                if endpoint < 0 { -2 } else { 2 },
                endpoint.unsigned_abs(),
            )?;
        }
    }
    let left = interval(-2, 3, 1)?;
    let right = interval(1, 4, 1)?;
    assert_eq!(
        left.intersection(&right, &mut ledger()?)?,
        interval(1, 3, 1)?
    );
    assert!(
        left.intersection(&interval(4, 5, 1)?, &mut ledger()?)
            .is_err()
    );
    let mut budget = ledger()?;
    assert!(left.coarsen(u32::MAX, &mut budget).is_err());
    assert_eq!(budget.usage().work, 0);
    Ok(())
}

/// Component, intermediate and staged ceilings independently bound outward rounding.
/// ´test:unit:enclosure-grid-size-pins´
/// ´claim:cas:enclosure-grid-size-pins´
#[test]
fn enclosure_grid_size_pins() -> Result<(), Box<dyn std::error::Error>> {
    let value = interval(1, 1, 3)?;
    for (bits, passes) in [(6, true), (5, false)] {
        let mut budget = Budget::new(Limits {
            component_bits: 5,
            intermediate_bits: bits,
            ..Limits::default()
        })?;
        assert_eq!(value.coarsen(4, &mut budget).is_ok(), passes);
    }
    for (bytes, passes) in [(1536, true), (1535, false)] {
        let mut budget = Budget::new(Limits {
            component_bits: 5,
            intermediate_bits: 64,
            staged_payload_bytes: bytes,
            ..Limits::default()
        })?;
        assert_eq!(value.coarsen(4, &mut budget).is_ok(), passes);
    }
    let mut budget = Budget::new(Limits {
        component_bits: 4,
        intermediate_bits: 64,
        ..Limits::default()
    })?;
    assert!(value.coarsen(4, &mut budget).is_err());
    assert_eq!(budget.usage().work, 0);
    Ok(())
}
