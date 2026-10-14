// SPDX-License-Identifier: AGPL-3.0-only
//! Complete public-API client flows for numbers.

use nrz_cas::{Fault, Rational};

use crate::testing::common::default_budget as budget;

/// Rump's exact reference reduction is obtained without any binary floating-point step.
/// ´test:integration:rump-exact-reduction´
#[test]
fn rump_exact_reduction() -> Result<(), Fault> {
    let mut ledger = budget()?;
    let a = Rational::parse("77617", &mut ledger)?;
    let b = Rational::parse("33096", &mut ledger)?;
    let a2 = a.pow(2, &mut ledger)?;
    let b2 = b.pow(2, &mut ledger)?;
    let b4 = b.pow(4, &mut ledger)?;
    let b6 = b.pow(6, &mut ledger)?;
    let b8 = b.pow(8, &mut ledger)?;
    let eleven = Rational::parse("11", &mut ledger)?;
    let one_hundred_twenty_one = Rational::parse("121", &mut ledger)?;
    let two = Rational::parse("2", &mut ledger)?;
    let inner = eleven
        .mul(&a2, &mut ledger)?
        .mul(&b2, &mut ledger)?
        .sub(&b6, &mut ledger)?
        .sub(&one_hundred_twenty_one.mul(&b4, &mut ledger)?, &mut ledger)?
        .sub(&two, &mut ledger)?;
    let first = Rational::parse("333.75", &mut ledger)?.mul(&b6, &mut ledger)?;
    let last = Rational::parse("11/2", &mut ledger)?.mul(&b8, &mut ledger)?;
    let polynomial = first
        .add(&a2.mul(&inner, &mut ledger)?, &mut ledger)?
        .add(&last, &mut ledger)?;
    assert_eq!(polynomial.exact_string(&mut ledger)?, "-2");
    let result = polynomial.add(
        &a.div(&two.mul(&b, &mut ledger)?, &mut ledger)?,
        &mut ledger,
    )?;
    assert_eq!(result.exact_string(&mut ledger)?, "-54767/66192");
    Ok(())
}
