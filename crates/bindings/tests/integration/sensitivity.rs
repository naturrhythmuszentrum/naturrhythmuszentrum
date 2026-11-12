// SPDX-License-Identifier: AGPL-3.0-only
//! A client retains its complete original and symbolic evidence through the frozen owner.

use nrz_bindings::{Descriptor, SensitivityRecipe};
use nrz_cas::{
    limits::{Budget, Limits, LimitsOverride},
    source::WalkEvent,
};

use crate::testing::declarations::{TestResult, context, input};
use crate::testing::sensitivity::{coefficient, equal_fraction};

/// Existing checks, native lowering and read-only proof inspection share one immutable owner.
/// ´test:integration:native-sensitivity-frozen-owner-lifecycle´
#[test]
fn native_sensitivity_frozen_owner_lifecycle() -> TestResult {
    let mut builder = context(&["x"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    let original = builder.parse("client square", "x^2")?;
    let earlier = builder.check(&original)?;
    let sensitivity = builder.check_sensitivity(
        SensitivityRecipe::Square,
        std::slice::from_ref(&original),
        &["x".into()],
        &LimitsOverride::default(),
    )?;
    sensitivity.declarations().validate_checked(&earlier)?;
    let cas = sensitivity.declarations().cas()?;
    let walk = cas.walk_original(&original.root, &mut Budget::new(Limits::default())?)?;
    assert!(
        walk.events
            .iter()
            .all(|event| matches!(event, WalkEvent::Visit(_)))
    );
    assert_eq!(
        sensitivity
            .jacobian()
            .originals()
            .first()
            .ok_or("missing original")?
            .original()
            .root,
        original.root
    );
    let entry = sensitivity
        .jacobian()
        .entries()
        .first()
        .ok_or("missing derivative")?;
    equal_fraction(
        coefficient(&cas, entry.expression(), &[("x", -4)])?,
        (-8, 1),
    )?;
    assert!(entry.identity().is_discharged());
    assert_eq!(entry.pairing().target(), &Descriptor::ratio());
    assert_eq!(sensitivity.snapshot_id(), earlier.snapshot_id());
    Ok(())
}

/// Physical input measures remain distinct from the stored-coordinate product law.
/// ´test:integration:quantity-product-sensitivity-coordinate-oracle´
/// ´claim:bindings:quantity-product-sensitivity-coordinate-oracle´ A checked product retains composed noncoherent measures and replayed coordinate derivatives two and three, with its original frozen evidence.
#[test]
fn quantity_product_sensitivity_coordinate_oracle() -> TestResult {
    let mut builder = context(&["x", "y"])?;
    let mut budget = Budget::new(Limits::default())?;
    let first = Descriptor::new(
        nrz_bindings::Kind::Ratio,
        nrz_bindings::Measure::new(
            "first",
            nrz_bindings::Scale::Exact(nrz_cas::Rational::from_ratio(2, 1, &mut budget)?),
            nrz_cas::Rational::zero(),
        )?,
        nrz_bindings::Role::Difference,
    )?;
    let second = Descriptor::new(
        nrz_bindings::Kind::Ratio,
        nrz_bindings::Measure::new(
            "second",
            nrz_bindings::Scale::Exact(nrz_cas::Rational::from_ratio(3, 1, &mut budget)?),
            nrz_cas::Rational::zero(),
        )?,
        nrz_bindings::Role::Difference,
    )?;
    input(&mut builder, "x", &first)?;
    input(&mut builder, "y", &second)?;
    let original = builder.parse("quantity product", "x*y")?;
    let earlier = builder.check(&original)?;
    let proof = builder.check_quantity_product_sensitivity(
        &original,
        &["x".into(), "y".into()],
        &LimitsOverride::default(),
    )?;
    proof.declarations().validate_checked(&earlier)?;
    assert_eq!(proof.inputs(), [first, second]);
    assert_eq!(proof.outputs()[0].measure().name(), "(first*second)");
    assert_eq!(proof.recipe(), SensitivityRecipe::QuantityProduct);
    assert_eq!(
        proof.obligations(),
        [] as [nrz_bindings::SensitivityObligation; 0]
    );
    let cas = proof.declarations().cas()?;
    for (entry, expected) in proof.jacobian().entries().iter().zip([2, 3]) {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("x", 3), ("y", 2)])?,
            (expected, 1),
        )?;
        assert!(entry.identity().is_discharged());
    }
    Ok(())
}

/// An ordered coordinate law cannot be reassigned to another syntax or input inventory.
/// ´test:integration:quantity-product-sensitivity-refusal-twins´
/// ´claim:bindings:quantity-product-sensitivity-refusal-twins´ Wrong original syntax and reordered quantity coordinates refuse while the matching original product is admitted.
#[test]
fn quantity_product_sensitivity_refusal_twins() -> TestResult {
    for (expression, keys, accepted) in [
        ("x+y", ["x", "y"], false),
        ("x*y", ["y", "x"], false),
        ("x*y", ["x", "y"], true),
    ] {
        let mut builder = context(&["x", "y"])?;
        input(&mut builder, "x", &Descriptor::ratio())?;
        input(&mut builder, "y", &Descriptor::ratio())?;
        let original = builder.parse("quantity product twin", expression)?;
        let result = builder.check_quantity_product_sensitivity(
            &original,
            &keys.map(String::from),
            &LimitsOverride::default(),
        );
        if accepted {
            result?;
        } else {
            let Err(fault) = result else {
                panic!("unmatched quantity product must refuse");
            };
            if expression == "x+y" {
                assert!(matches!(
                    fault,
                    nrz_bindings::SensitivityError::UnsupportedRecipe
                ));
            } else {
                assert!(matches!(fault, nrz_bindings::SensitivityError::Coordinates));
            }
        }
    }
    Ok(())
}

/// Complete preparation has a larger reservation boundary than its actual consumed Work.
/// ´test:integration:quantity-product-sensitivity-work-boundary´
/// ´claim:bindings:quantity-product-sensitivity-work-boundary´ Product sensitivity accepts its literal cumulative allowance, charges only actual preparation and four service receipts, and preserves named exhaustion and spent Work one unit below.
#[test]
fn quantity_product_sensitivity_work_boundary() -> TestResult {
    for accepted in [false, true] {
        let mut budget = Budget::new(Limits {
            work: if accepted { 878_136 } else { 878_135 },
            ..Limits::default()
        })?;
        let ratio = Descriptor::ratio();
        let result = nrz_bindings::CheckedSensitivity::quantity_product(
            &nrz_bindings::Registry::new(nrz_bindings::ExponentPolicy::Rational),
            [&ratio, &ratio],
            &mut budget,
        );
        if accepted {
            let proof = result?;
            assert_eq!(budget.usage().work, 818_887);
            assert_eq!(
                proof.jacobian().work(),
                nrz_bindings::SensitivityWork {
                    construction: 24_965,
                    generation: 73_275,
                    replay: 218_157,
                    requests: 4,
                }
            );
        } else {
            let Err(nrz_bindings::SensitivityError::Exhausted { fault, outcome }) = result else {
                panic!("one-below quantity proof allowance must retain named exhaustion");
            };
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::Work
                        | nrz_cas::limits::BudgetKind::ReplayWork,
                    ..
                }
            ));
            assert!(matches!(
                *outcome,
                nrz_cas::request::Outcome::Exhausted { .. }
            ));
            assert_eq!(budget.usage().work, 721_941);
        }
    }
    Ok(())
}

/// Factory derivatives are checked against an independent exact evaluator.
/// ´test:integration:quantity-power-sensitivity-oracle´
/// ´claim:bindings:quantity-power-sensitivity-oracle´ Quantity integer-power identities retain physical originals, exact composed descriptors and replayed rational derivatives for signed, zero and unit powers.
#[test]
fn quantity_power_sensitivity_oracle() -> TestResult {
    let registry = nrz_bindings::Registry::new(nrz_bindings::ExponentPolicy::Rational);
    for (exponent, numerator, denominator) in [
        (-2, -2, 27),
        (-1, -1, 9),
        (0, 0, 1),
        (1, 1, 1),
        (2, 6, 1),
        (3, 27, 1),
    ] {
        let proof = nrz_bindings::CheckedSensitivity::quantity_integer_power(
            &registry,
            &Descriptor::ratio(),
            exponent,
            &mut Budget::new(Limits::default())?,
        )?;
        let cas = proof.declarations().cas()?;
        equal_fraction(
            coefficient(
                &cas,
                proof.jacobian().entries()[0].expression(),
                &[("x", 3)],
            )?,
            (numerator, denominator),
        )?;
        assert_eq!(
            proof.recipe(),
            SensitivityRecipe::QuantityIntegerPower { exponent }
        );
        assert!(proof.jacobian().entries()[0].identity().is_discharged());
        proof
            .declarations()
            .validate_checked(&proof.originals()[0])?;
    }
    Ok(())
}

/// Polynomial partials remain tied to the root placeholder and original coordinate.
/// ´test:integration:quantity-root-sensitivity-oracle´
/// ´claim:bindings:quantity-root-sensitivity-oracle´ A quantity positive-root identity retains its original root and polynomial relation, checked partials four and minus one at the exact root point, and its undischarged numerical positivity obligation.
#[test]
fn quantity_root_sensitivity_oracle() -> TestResult {
    let registry = nrz_bindings::Registry::new(nrz_bindings::ExponentPolicy::Rational);
    let proof = nrz_bindings::CheckedSensitivity::quantity_positive_root(
        &registry,
        &Descriptor::ratio(),
        &mut Budget::new(Limits::default())?,
    )?;
    let cas = proof.declarations().cas()?;
    for (entry, expected) in proof.jacobian().entries().iter().zip([(4, 1), (-1, 1)]) {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("x", 4), ("t", 2)])?,
            expected,
        )?;
        assert!(entry.identity().is_discharged());
    }
    assert_eq!(
        proof.obligations(),
        &[nrz_bindings::SensitivityObligation::PositiveRoot {
            coordinate: 0,
            output: 0
        }]
    );
    proof
        .declarations()
        .validate_checked(&proof.originals()[0])?;
    proof
        .declarations()
        .validate_checked(&proof.jacobian().originals()[0])?;
    Ok(())
}

/// Both quotient coordinates are proved independently under one retained divisor premise.
/// ´test:integration:quantity-quotient-sensitivity-oracle´
/// ´claim:bindings:quantity-quotient-sensitivity-oracle´ Quantity quotient proofs retain their physical original, ordered coordinate partials one half and minus three quarters and the numerical nonzero-divisor obligation.
#[test]
fn quantity_quotient_sensitivity_oracle() -> TestResult {
    let registry = nrz_bindings::Registry::new(nrz_bindings::ExponentPolicy::Rational);
    let ratio = Descriptor::ratio();
    let proof = nrz_bindings::CheckedSensitivity::quantity_quotient(
        &registry,
        [&ratio, &ratio],
        &mut Budget::new(Limits::default())?,
    )?;
    let cas = proof.declarations().cas()?;
    for (entry, expected) in proof.jacobian().entries().iter().zip([(1, 2), (-3, 4)]) {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("x", 3), ("y", 2)])?,
            expected,
        )?;
        assert!(entry.identity().is_discharged());
    }
    assert_eq!(
        proof.obligations(),
        &[nrz_bindings::SensitivityObligation::Nonzero { coordinate: 1 }]
    );
    proof
        .declarations()
        .validate_checked(&proof.originals()[0])?;
    Ok(())
}
