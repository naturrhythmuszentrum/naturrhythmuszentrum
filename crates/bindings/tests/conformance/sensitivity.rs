// SPDX-License-Identifier: AGPL-3.0-only
//! Public native sensitivity admission, exact coefficients and semantic refusals.

use nrz_bindings::{
    BaseDimension, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry, Role, Scale,
    SensitivityError, SensitivityKernel, SensitivityObligation, SensitivityRecipe,
};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};

use crate::testing::declarations::{TestResult, derived, input, length};
use crate::testing::sensitivity::{
    coefficient, context, context_with_registry, equal_fraction, request_limits,
};

/// A nonsquare ordered Jacobian retains physical zero quotients and every input position.
/// ´test:integration:native-square-order-and-typed-zeros´
#[test]
fn native_square_order_and_typed_zeros() -> TestResult {
    let distance = length()?;
    let area = Kind::Named(
        "area".into(),
        Box::new(distance.dimension().power(
            &Rational::from_ratio(2, 1, &mut Budget::new(Limits::default())?)?,
            ExponentPolicy::Rational,
        )?),
    );
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_power(
        distance.kind().clone(),
        Rational::from_ratio(2, 1, &mut Budget::new(Limits::default())?)?,
        area,
    )?;
    let mut builder = context_with_registry(registry, &["x", "y", "z"])?;
    input(&mut builder, "x", &distance)?;
    input(&mut builder, "y", &distance)?;
    let time = Descriptor::new(
        Kind::Named(
            "time".into(),
            Box::new(Dimension::base(BaseDimension::Time)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    input(&mut builder, "z", &time)?;
    let originals = [
        builder.parse("first", "x^2")?,
        builder.parse("second", "y^2")?,
    ];
    let checked = builder.check_sensitivity(
        SensitivityRecipe::Square,
        &originals,
        &["y".into(), "z".into(), "x".into()],
        &request_limits(),
    )?;
    assert_eq!(
        checked
            .entries()
            .iter()
            .map(nrz_bindings::SensitivityEntry::kernel)
            .collect::<Vec<_>>(),
        [
            SensitivityKernel::Zero,
            SensitivityKernel::Square { coordinate: 0 },
            SensitivityKernel::Zero,
            SensitivityKernel::Zero,
            SensitivityKernel::Square { coordinate: 2 },
            SensitivityKernel::Zero
        ]
    );
    assert_eq!(checked.obligations(), []);
    let cas = checked.declarations().cas()?;
    for (entry, expected) in checked.jacobian().entries().iter().zip([0, 10, 0, 0, 6, 0]) {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("x", 3), ("y", 5), ("z", 7)])?,
            (expected, 1),
        )?;
    }
    let zero = checked.entries().get(2).ok_or("missing typed zero")?;
    assert_eq!(
        zero.pairing().dimension(),
        &checked
            .outputs()
            .first()
            .ok_or("missing output")?
            .dimension()
            .quotient(time.dimension())?
    );
    assert_eq!(zero.pairing().other(), Some(&time));
    Ok(())
}

/// A reciprocal proof retains its premise even when physical declarations make no nonzero claim.
/// ´test:integration:native-reciprocal-retains-domain-premises´
#[test]
fn native_reciprocal_retains_domain_premises() -> TestResult {
    let mut builder = context(&["x", "y"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    input(&mut builder, "y", &Descriptor::ratio())?;
    let originals = [
        builder.parse("quotient", "1/x")?,
        builder.parse("power", "y^-1")?,
    ];
    let checked = builder.check_sensitivity(
        SensitivityRecipe::Reciprocal,
        &originals,
        &["y".into(), "x".into()],
        &request_limits(),
    )?;
    assert_eq!(
        checked.obligations(),
        [
            SensitivityObligation::Nonzero { coordinate: 0 },
            SensitivityObligation::Nonzero { coordinate: 1 }
        ]
    );
    let witnesses = checked.jacobian().witnesses();
    let [witness] = witnesses else {
        return Err("missing native assembly".into());
    };
    assert!(witness.is_discharged());
    assert_eq!(witness.hypotheses().len(), 2);
    let cas = checked.declarations().cas()?;
    for (entry, expected) in
        checked
            .jacobian()
            .entries()
            .iter()
            .zip([(0, 1), (-1, 25), (-1, 9), (0, 1)])
    {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("x", 3), ("y", 5)])?,
            expected,
        )?;
    }
    Ok(())
}

/// Input scales enter the derivative chain rule and formal quotient measure separately.
/// ´test:integration:native-sensitivity-measure-chain-rule´
#[test]
fn native_sensitivity_measure_chain_rule() -> TestResult {
    for (recipe, body, expected) in [
        (SensitivityRecipe::Square, "x^2", (1, 40)),
        (SensitivityRecipe::Reciprocal, "1/x", (-4, 625)),
    ] {
        let mut builder = context(&["x"])?;
        let scale = Rational::from_ratio(1, 100, &mut Budget::new(Limits::default())?)?;
        let descriptor = Descriptor::new(
            Kind::Ratio,
            Measure::new("percent", Scale::Exact(scale), Rational::zero())?,
            Role::Difference,
        )?;
        input(&mut builder, "x", &descriptor)?;
        let original = builder.parse("scaled", body)?;
        let checked =
            builder.check_sensitivity(recipe, &[original], &["x".into()], &request_limits())?;
        let entry = checked
            .jacobian()
            .entries()
            .first()
            .ok_or("missing derivative")?;
        equal_fraction(
            coefficient(
                &*checked.declarations().cas()?,
                entry.expression(),
                &[("x", 125)],
            )?,
            expected,
        )?;
        assert_eq!(
            entry
                .pairing()
                .measure()
                .rational_scale()?
                .exact_string(&mut Budget::new(Limits::default())?)?,
            "100"
        );
    }
    Ok(())
}

/// A valid but unsupported formula cannot turn descriptor validity into an executable derivative.
/// ´test:integration:native-sensitivity-closed-recipe-refusal´
#[test]
fn native_sensitivity_closed_recipe_refusal() -> TestResult {
    for body in ["x*x", "x^3", "sqrt(x)", "0*x+x^2"] {
        let mut builder = context(&["x"])?;
        input(&mut builder, "x", &Descriptor::ratio())?;
        let original = builder.parse("unsupported", body)?;
        assert!(matches!(
            builder.check_sensitivity(
                SensitivityRecipe::Square,
                &[original],
                &["x".into()],
                &request_limits()
            ),
            Err(SensitivityError::UnsupportedRecipe)
        ));
    }
    Ok(())
}

/// Unused invalid originals preserve their diagnostic instead of being hidden by a native request.
/// ´test:integration:native-sensitivity-checks-unused-originals´
#[test]
fn native_sensitivity_checks_unused_originals() -> TestResult {
    let mut builder = context(&["x", "distance", "unused"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    input(&mut builder, "distance", &length()?)?;
    derived(
        &mut builder,
        "unused",
        "0*(distance+1)",
        &Descriptor::ratio(),
    )?;
    let original = builder.parse("square", "x^2")?;
    let Err(SensitivityError::Original(error)) = builder.check_sensitivity(
        SensitivityRecipe::Square,
        &[original],
        &["x".into()],
        &request_limits(),
    ) else {
        return Err("hidden invalid declaration admitted".into());
    };
    assert_eq!(error.location.definitions, ["unused"]);
    assert!(error.location.span.is_some());
    Ok(())
}

/// Missing, repeated and dependent coordinates refuse before a partial schedule can escape.
/// ´test:integration:native-sensitivity-coordinate-refusals´
#[test]
fn native_sensitivity_coordinate_refusals() -> TestResult {
    for keys in [vec![], vec!["x", "x"], vec!["y"], vec!["alias"]] {
        let mut builder = context(&["x", "y", "alias"])?;
        input(&mut builder, "x", &Descriptor::ratio())?;
        input(&mut builder, "y", &Descriptor::ratio())?;
        derived(&mut builder, "alias", "x", &Descriptor::ratio())?;
        let original = builder.parse("square", "x^2")?;
        let coordinates = keys.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(matches!(
            builder.check_sensitivity(
                SensitivityRecipe::Square,
                &[original],
                &coordinates,
                &request_limits()
            ),
            Err(SensitivityError::Coordinates)
        ));
    }
    Ok(())
}

/// Schedule extent refusal retains the precise bounded CAS fault and never a partial result.
/// ´test:integration:native-sensitivity-extent-refusal´
#[test]
fn native_sensitivity_extent_refusal() -> TestResult {
    let mut builder = context(&["x", "y"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    input(&mut builder, "y", &Descriptor::ratio())?;
    let original = builder.parse("square", "x^2")?;
    let limits = LimitsOverride {
        matrix_entries: Some(1),
        ..request_limits()
    };
    assert!(matches!(
        builder.check_sensitivity(
            SensitivityRecipe::Square,
            &[original],
            &["x".into(), "y".into()],
            &limits
        ),
        Err(SensitivityError::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: nrz_cas::limits::BudgetKind::MatrixEntries,
                ..
            },
            ..
        }))
    ));
    Ok(())
}

/// Affine, discrete and unrelated named kinds cannot acquire native scalar sensitivities.
/// ´test:integration:native-sensitivity-physical-refusals´
#[test]
fn native_sensitivity_physical_refusals() -> TestResult {
    let distance = length()?;
    let point = Descriptor::new(distance.kind().clone(), Measure::coherent(), Role::Point)?;
    let count = Descriptor::new(
        Kind::Count("events".into(), nrz_bindings::CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    for descriptor in [point, count, distance] {
        let mut builder = context(&["x"])?;
        input(&mut builder, "x", &descriptor)?;
        let original = builder.parse("square", "x^2")?;
        assert!(
            builder
                .check_sensitivity(
                    SensitivityRecipe::Square,
                    &[original],
                    &["x".into()],
                    &request_limits()
                )
                .is_err()
        );
    }
    let mut builder = context(&["x", "note"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    builder.declare(
        "note".into(),
        nrz_bindings::Declaration::Metadata("text".into()),
    )?;
    let original = builder.parse("square", "x^2")?;
    assert!(matches!(
        builder.check_sensitivity(
            SensitivityRecipe::Square,
            &[original],
            &["note".into()],
            &request_limits()
        ),
        Err(SensitivityError::Coordinates)
    ));
    Ok(())
}
