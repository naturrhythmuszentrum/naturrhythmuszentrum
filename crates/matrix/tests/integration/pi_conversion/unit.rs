// SPDX-License-Identifier: AGPL-3.0-only
//! Typed source-free coefficients retain certified pi bounds without a coordinate-map surrogate.

use super::{exact_policy, oracle, owner, pi};
use crate::{conversion_testing::TestResult, kernel_oracle::Rational};
use nrz_bindings::{
    Budget, Conversion, Descriptor, ExactConstant, Kind, Limits, Measure, Role, Scale,
};
use nrz_matrix::{CertifiedCoefficient, ExactCoefficient};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};
use std::rc::Rc;

/// A one-unit pi quantity is an exact schema definition before numerical admission.
fn unit() -> TestResult<Rc<ExactCoefficient>> {
    let descriptor = Descriptor::new(
        Kind::Ratio,
        Measure::new(
            "pi-unit",
            Scale::pi()?,
            Measure::coherent().offset().clone(),
        )?,
        Role::Difference,
    )?;
    Ok(Rc::new(ExactCoefficient::unit(&descriptor)?))
}

/// Every admitted grammar direction, including signed coefficients, encloses independent rational pi bounds.
/// ´test:integration:certified-unit-coefficient-encloses-exact-definition´
#[test]
fn certified_unit_coefficient_encloses_exact_definition() -> TestResult {
    enclosures::<f32>()?;
    enclosures::<f64>()
}

/// The same numerical region survives dropping caller owners and retains the original exact node.
fn enclosures<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let unit = unit()?;
    let inverse = Rc::new(ExactCoefficient::new(
        &Descriptor::ratio(),
        &degree.coherent().conversion_to(degree)?,
    )?);
    let negative = Rc::new(ExactCoefficient::new(
        &Descriptor::ratio(),
        &Conversion {
            scale: unit
                .exact_map()
                .scale
                .negated(&mut Budget::new(Limits::default())?)?,
            shift: ExactConstant::zero(),
        },
    )?);
    let positive = oracle::factor(1, 1, false, 54)?;
    let minus = Rational::new(-1, 1)?;
    let expected = [
        positive,
        oracle::factor(180, 1, true, 48)?,
        [positive[1].multiply(minus)?, positive[0].multiply(minus)?],
    ];
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    for (definition, endpoints) in [unit, inverse, negative].into_iter().zip(expected) {
        let coefficient = CertifiedCoefficient::new(
            &definition,
            &circle,
            &exact_policy(),
            LiteralLimits::default(),
            &context,
            &mut Budget::new(Limits::default())?,
        )?;
        assert!(std::ptr::eq(coefficient.definition(), definition.as_ref()));
        assert!(coefficient.constant().same_region(&circle));
        assert!(coefficient.same_region(&coefficient.clone()));
        let retained = coefficient.clone();
        drop(coefficient);
        drop(definition);
        oracle::covers(
            endpoints,
            retained.center().get().to_f64(),
            retained.displacement().get().to_f64(),
        )?;
        for endpoint in endpoints {
            assert!(
                Rational::binary(retained.enclosure().lower().get().to_f64())?.at_most(endpoint)?
            );
            assert!(endpoint.at_most(Rational::binary(
                retained.enclosure().upper().get().to_f64()
            )?)?);
        }
    }
    Ok(())
}

/// Unsupported exact coefficients cannot acquire approximate pi authority.
/// ´test:integration:certified-unit-coefficient-refusal-is-atomic´
#[test]
fn certified_unit_coefficient_refusal_is_atomic() -> TestResult {
    use nrz_bindings::readings::algebra::{Location, syntax::Constant};
    let context = ArithmeticContext::new()?;
    let circle = pi::<f64>(&context, &mut Budget::new(Limits::default())?)?;
    let mut setup = Budget::new(Limits::default())?;
    let exact = ExactConstant::named(Constant::Pi, &mut setup)?;
    for value in [
        ExactConstant::named(Constant::E, &mut setup)?,
        exact.add(&ExactConstant::one(), &mut setup)?,
        exact.mul(&exact, &mut setup)?,
    ] {
        let definition = Rc::new(ExactCoefficient::new(
            &Descriptor::ratio(),
            &Conversion {
                scale: value,
                shift: ExactConstant::zero(),
            },
        )?);
        let mut budget = Budget::new(Limits::default())?;
        budget.work(17, Location::new("prior coefficient caller"))?;
        let before = budget.usage();
        assert!(matches!(
            CertifiedCoefficient::new(
                &definition,
                &circle,
                &exact_policy(),
                LiteralLimits::default(),
                &context,
                &mut budget
            ),
            Err(nrz_matrix::ConversionError::Map(
                nrz_bindings::MapError::Units(
                    nrz_bindings::UnitError::UnsupportedExactConstantExport
                )
            ))
        ));
        assert_eq!(budget.usage(), before);
    }
    Ok(())
}

/// Direct typed coefficient admission has literal cumulative Work and byte boundaries.
/// ´test:integration:certified-unit-coefficient-owner-boundaries´
#[test]
fn certified_unit_coefficient_owner_boundaries() -> TestResult {
    boundaries::<f32>(82_072, 56)?;
    boundaries::<f64>(82_120, 72)
}

/// Failed admission preserves caller usage; cloned successful owners retain the same allocation.
fn boundaries<P: Scalar + 'static>(staged: u64, output: u64) -> TestResult {
    let definition = unit()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let exact = Limits {
        work: 28_132,
        staged_payload_bytes: staged,
        output_bytes: output,
        ..Limits::default()
    };
    for resource in 0..4 {
        let mut limits = exact.clone();
        match resource {
            0 => limits.work -= 1,
            1 => limits.staged_payload_bytes -= 1,
            2 => limits.output_bytes -= 1,
            _ => {}
        }
        let mut budget = Budget::new(limits)?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior coefficient caller"),
        )?;
        let before = budget.usage();
        let result = CertifiedCoefficient::new(
            &definition,
            &circle,
            &exact_policy(),
            LiteralLimits::default(),
            &context,
            &mut budget,
        );
        if resource == 3 {
            let coefficient = result?;
            assert_eq!(coefficient.work_used(), 28_115);
            assert_eq!(u64::try_from(coefficient.owned_bytes())?, output);
            assert_eq!(budget.usage().work, 28_132);
            assert!(coefficient.same_region(&coefficient.clone()));
        } else {
            assert!(result.is_err());
            assert_eq!(budget.usage(), before);
        }
    }
    Ok(())
}
