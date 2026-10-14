// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for numbers.

use std::cmp::Ordering;

use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{Exhaustion, Fault, FaultKind, Location, Rational};

use crate::testing::common::default_budget as budget;

/// Decimal spellings are exact and canonical zero has no negative sign.
/// ´test:integration:exact-literals´
#[test]
fn exact_literals() -> Result<(), Fault> {
    for (text, expected) in [
        ("-0.000", "0"),
        ("0.125", "1/8"),
        ("12.50e-2", "1/8"),
        ("-42/30", "-7/5"),
        (".5", "1/2"),
        ("1e3", "1000"),
        ("+5.", "5"),
    ] {
        let mut ledger = budget()?;
        let value = Rational::parse(text, &mut ledger)?;
        assert_eq!(value.exact_string(&mut ledger)?, expected);
    }
    let mut ledger = budget()?;
    let minimum = Rational::from_ratio(i64::MIN, 1, &mut ledger)?;
    assert_eq!(minimum.exact_string(&mut ledger)?, "-9223372036854775808");
    assert_eq!(
        minimum.negated(&mut ledger)?.exact_string(&mut ledger)?,
        "9223372036854775808"
    );
    assert_eq!(
        Rational::zero().canonical_bytes(&mut ledger)?,
        vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 1]
    );
    Ok(())
}

/// Integer arithmetic is compared with a separately computed bounded host oracle.
/// ´test:integration:small-arithmetic-oracle´
#[test]
fn small_arithmetic_oracle() -> Result<(), Fault> {
    for left in -7_i32..=7 {
        for right in -7_i32..=7 {
            let mut ledger = budget()?;
            let a = Rational::from_ratio(i64::from(left), 1, &mut ledger)?;
            let b = Rational::from_ratio(i64::from(right), 1, &mut ledger)?;
            assert_eq!(
                a.add(&b, &mut ledger)?.exact_string(&mut ledger)?,
                (left + right).to_string(),
            );
            assert_eq!(
                a.mul(&b, &mut ledger)?.exact_string(&mut ledger)?,
                (left * right).to_string(),
            );
            assert_eq!(
                a.sub(&b, &mut ledger)?.exact_string(&mut ledger)?,
                (left - right).to_string(),
            );
            assert_eq!(a.compare(&b, &mut ledger)?, left.cmp(&right));
            if right != 0 {
                assert_eq!(a.div(&b, &mut ledger)?.mul(&b, &mut ledger)?, a);
            }
        }
    }
    Ok(())
}

/// Fraction comparison exercises alternating continued-fraction directions and signs.
/// ´test:integration:fraction-order´
#[test]
fn fraction_order() -> Result<(), Fault> {
    for numerator in -5_i32..=5 {
        for denominator in 1_u32..=6 {
            let mut ledger = budget()?;
            let value =
                Rational::from_ratio(i64::from(numerator), u64::from(denominator), &mut ledger)?;
            let third = Rational::from_ratio(1, 3, &mut ledger)?;
            let expected = (i64::from(numerator) * 3).cmp(&i64::from(denominator));
            assert_eq!(value.compare(&third, &mut ledger)?, expected);
            assert_eq!(third.compare(&value, &mut ledger)?, expected.reverse());
        }
    }
    let mut ledger = budget()?;
    let third = Rational::from_ratio(2, 6, &mut ledger)?;
    assert_eq!(
        third.compare(&Rational::parse("1/3", &mut ledger)?, &mut ledger)?,
        Ordering::Equal
    );
    Ok(())
}

/// The reference power crosses the former machine ceiling and has an explicit budget refusal.
/// ´test:integration:kahan-capacity´
#[test]
fn kahan_capacity() -> Result<(), Fault> {
    let mut ledger = budget()?;
    let base = Rational::from_ratio(4, 5, &mut ledger)?;
    let result = base.pow(78, &mut ledger)?;
    assert_eq!(result.component_bits(), (157, 182));
    let half = base.pow(39, &mut ledger)?;
    assert_eq!(half.mul(&half, &mut ledger)?, result);
    let mut small = Budget::new(Limits {
        component_bits: 181,
        ..Limits::default()
    })?;
    assert!(matches!(
        base.pow(78, &mut small),
        Err(Fault {
            kind: FaultKind::Exhausted {
                reason: Exhaustion::ArithmeticRange,
                budget: BudgetKind::ComponentBits,
                ..
            },
            ..
        }),
    ));
    Ok(())
}

/// Strict undefined powers, malformed literals and zero divisors cannot become exact values.
/// ´test:integration:number-refusals´
#[test]
fn number_refusals() -> Result<(), Fault> {
    for malformed in ["", "+", "1/0", "1/2/3", "1.2.3", "1e", "1e2e3", " 1", "NaN"] {
        assert!(Rational::parse(malformed, &mut budget()?).is_err());
    }
    assert!(Rational::zero().pow(0, &mut budget()?).is_err());
    assert!(Rational::zero().pow(-1, &mut budget()?).is_err());
    assert!(
        Rational::one()
            .div(&Rational::zero(), &mut budget()?)
            .is_err()
    );
    let mut ledger = Budget::new(Limits {
        integer_exponent: u64::MAX,
        ..Limits::default()
    })?;
    assert_eq!(Rational::one().pow(i64::MIN, &mut ledger)?, Rational::one());
    let mut narrow = Budget::new(Limits {
        component_bits: 4,
        intermediate_bits: 4,
        ..Limits::default()
    })?;
    let value = Rational::from_ratio(7, 1, &mut narrow)?;
    assert!(matches!(
        value.mul(&value, &mut narrow),
        Err(Fault {
            kind: FaultKind::Exhausted {
                reason: Exhaustion::ArithmeticRange,
                budget: BudgetKind::IntermediateBits,
                ..
            },
            ..
        }),
    ));
    Ok(())
}

/// The first rejected operation does not spend either of its atomic counters.
/// ´test:integration:ledger-boundary´
#[test]
fn ledger_boundary() -> Result<(), Fault> {
    let mut ledger = Budget::new(Limits {
        work: 7,
        arithmetic_steps: 7,
        ..Limits::default()
    })?;
    let location = Location::new("test boundary");
    ledger.arithmetic(7, location)?;
    let before = ledger.usage();
    assert!(ledger.arithmetic(1, location).is_err());
    assert_eq!(ledger.usage(), before);
    assert!(ledger.work(u64::MAX, location).is_err());
    assert_eq!(ledger.usage(), before);
    assert!(
        Budget::new(Limits {
            component_bits: 0,
            ..Limits::default()
        })
        .is_err()
    );
    assert!(
        Budget::new(Limits {
            cost_quota: 0,
            ..Limits::default()
        })
        .is_err()
    );
    Ok(())
}

/// Cross-cancellation succeeds when the corresponding uncanceled product would exceed the cap.
/// ´test:integration:cross-cancellation´
#[test]
fn cross_cancellation() -> Result<(), Fault> {
    let mut ledger = Budget::new(Limits {
        component_bits: 64,
        intermediate_bits: 64,
        ..Limits::default()
    })?;
    let left = Rational::from_ratio(9_223_372_036_854_775_807, 5, &mut ledger)?;
    let right = Rational::from_ratio(5, 9_223_372_036_854_775_807, &mut ledger)?;
    assert_eq!(left.mul(&right, &mut ledger)?, Rational::one());
    Ok(())
}
