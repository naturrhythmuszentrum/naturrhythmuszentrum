// SPDX-License-Identifier: AGPL-3.0-only
//! Adversarial enclosure test for the exact Rump polynomial value.

use nrz_scalar::{ArithmeticFailure, Interval, Scalar};
use serde_json::Value;

/// Raise an interval to a small nonnegative integer power.
///
/// TODO ´todo:test:check-powered-quantity-descriptors-while-treating´: check powered quantity descriptors while treating the exponent as metadata under req:vision:powers-and-factor-dimensions and rule:vision:counts-and-metadata.
fn power<T: Scalar>(base: Interval<T>, exponent: usize) -> Result<Interval<T>, ArithmeticFailure> {
    let mut product = Interval::point(T::ONE)?;
    for _ in 0..exponent {
        product = product.checked_mul(base)?;
    }
    Ok(product)
}

/// Evaluate the fixture expression in its displayed order.
///
/// TODO ´todo:test:evaluate-a-quantity-carrying-polynomial-with´: evaluate a quantity-carrying polynomial with compatible coefficient dimensions and intrinsic computational uncertainty under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
fn evaluate<T: Scalar>(
    a: T,
    b: T,
    c333: T,
    c11: T,
    quartic_coefficient: T,
    c2: T,
    c55: T,
) -> Result<Interval<T>, ArithmeticFailure> {
    let a = Interval::point(a)?;
    let b = Interval::point(b)?;
    let a2 = power(a, 2)?;
    let b2 = power(b, 2)?;
    let b4 = power(b, 4)?;
    let b6 = power(b, 6)?;
    let b8 = power(b, 8)?;
    let first = Interval::point(c333)?.checked_mul(b6)?;
    let inside = Interval::point(c11)?
        .checked_mul(a2)?
        .checked_mul(b2)?
        .checked_sub(b6)?
        .checked_sub(Interval::point(quartic_coefficient)?.checked_mul(b4)?)?
        .checked_sub(Interval::point(c2)?)?;
    let middle = a2.checked_mul(inside)?;
    let last = Interval::point(c55)?.checked_mul(b8)?;
    let quotient = a.checked_div(Interval::point(c2)?.checked_mul(b)?)?;
    first
        .checked_add(middle)?
        .checked_add(last)?
        .checked_add(quotient)
}

/// Read a declared coherent ratio coordinate before numerical fixture evaluation.
fn field<'a>(entry: &'a Value, group: &str, name: &str) -> &'a str {
    let value = entry[group]
        .as_array()
        .expect("fixture array")
        .iter()
        .find(|value| value["name"].as_str() == Some(name))
        .expect("fixture declaration");
    if group == "parameters" {
        assert_eq!(value["role"], "quantity");
        assert_eq!(value["uncertainty"]["kind"], "none");
    }
    let descriptor = &value["descriptor"];
    assert_eq!(
        descriptor["dimensions"],
        serde_json::json!(["0", "0", "0", "0", "0", "0", "0"])
    );
    assert_eq!(descriptor["affine_role"], "difference");
    assert_eq!(descriptor["measure"]["scale"], "1");
    assert_eq!(descriptor["measure"]["offset"], "0");
    let kind = entry["kinds"]
        .as_array()
        .expect("declared kinds")
        .iter()
        .find(|kind| kind["name"] == descriptor["kind"])
        .expect("declared ratio kind");
    assert_eq!(kind["type"], "ratio");
    assert_eq!(kind["dimensions"], descriptor["dimensions"]);
    value["value"].as_str().expect("fixture value")
}

/// Both supported formats must refuse or contain the exact rational value.
///
/// TODO ´todo:test:extend-the-rump-oracle-with-dimensionless´: extend the Rump oracle with dimensionless quantity and computational-uncertainty kinds while keeping precision selections metadata under rule:vision:uncertainty-unit-algebra and rule:vision:counts-and-metadata.
///
/// ´test:integration:rump-polynomial-enclosure´
#[test]
fn rump_polynomial_enclosure() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/datasets/problems.json");
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("dataset")).expect("JSON");
    let entry = data["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|value| value["problem"]["slug"].as_str() == Some("rump-polynomial"))
        .expect("Rump fixture");
    let a = field(entry, "parameters", "a");
    let b = field(entry, "parameters", "b");
    let exact = field(entry, "observables", "exact_value");
    let (numerator, denominator) = exact.split_once('/').expect("rational");
    let target64 = Interval::point(numerator.parse::<f64>().expect("numerator"))
        .expect("finite")
        .checked_div(
            Interval::point(denominator.parse::<f64>().expect("denominator")).expect("finite"),
        )
        .expect("target");
    let candidate64 = evaluate(
        a.parse::<f64>().expect("a"),
        b.parse::<f64>().expect("b"),
        333.75,
        11.0,
        121.0,
        2.0,
        5.5,
    );
    if let Ok(value) = candidate64 {
        assert!(value.lo() <= target64.lo() && value.hi() >= target64.hi());
    }
    let target32 = Interval::point(numerator.parse::<f32>().expect("numerator"))
        .expect("finite")
        .checked_div(
            Interval::point(denominator.parse::<f32>().expect("denominator")).expect("finite"),
        )
        .expect("target");
    let candidate32 = evaluate(
        a.parse::<f32>().expect("a"),
        b.parse::<f32>().expect("b"),
        333.75,
        11.0,
        121.0,
        2.0,
        5.5,
    );
    if let Ok(value) = candidate32 {
        assert!(value.lo() <= target32.lo() && value.hi() >= target32.hi());
    }
}
