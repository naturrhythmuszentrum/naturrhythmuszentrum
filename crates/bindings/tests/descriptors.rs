// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor laws without any dependency on the expression checker.

use nrz_cas::Rational;
#[path = "support/arithmetic.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod support;
use support::rational;

use nrz_bindings::{
    BaseDimension, BinaryOp, CountDomain, Descriptor, Dimension, ExponentPolicy, Kind, Measure,
    Registry, Role, Scale, UncertaintyMode, UnitError, uncertainty,
};

/// Fallible setup keeps tests free of unchecked fixture assumptions.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Make a named linear descriptor in coherent measure.
fn named(name: &str, dimension: Dimension) -> Result<Descriptor, Box<dyn std::error::Error>> {
    Ok(Descriptor::new(
        Kind::Named(name.to_owned(), Box::new(dimension)),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Seven-axis arithmetic implements products, quotients and rational powers exactly.
///
/// ´test:integration:dimension-laws´
#[test]
fn dimension_laws() -> TestResult {
    for axis in [
        BaseDimension::Length,
        BaseDimension::Mass,
        BaseDimension::Time,
        BaseDimension::Current,
        BaseDimension::Temperature,
        BaseDimension::Amount,
        BaseDimension::LuminousIntensity,
    ] {
        let dimension = Dimension::base(axis);
        for numerator in -3..=3 {
            for denominator in 1..=3 {
                let exponent = rational(numerator, denominator)?;
                let powered = dimension.power(&exponent, ExponentPolicy::Rational)?;
                assert_eq!(powered.quotient(&powered)?, Dimension::zero());
                assert_eq!(powered.product(&dimension)?, dimension.product(&powered)?);
                let expected = if exponent.is_integer() {
                    Ok(())
                } else {
                    Err(UnitError::NonIntegralDimension)
                };
                assert_eq!(powered.admit(ExponentPolicy::Integral), expected);
            }
        }
    }
    let mass = Dimension::base(BaseDimension::Mass);
    let stiffness = Dimension::integers([0, 1, -2, 0, 0, 0, 0])?;
    assert_eq!(
        stiffness.quotient(&mass)?,
        Dimension::integers([0, 0, -2, 0, 0, 0, 0])?
    );
    assert_eq!(
        Dimension::integers([i64::MAX; 7])?.product(&Dimension::integers([1; 7])?),
        Err(UnitError::ExponentRange)
    );
    Ok(())
}

/// Affine conversions apply offsets only to points and preserve exact round trips.
///
/// TODO ´todo:test:test-shaped-point-origins-never-translate´: test shaped point origins never translate difference or covariance companions under def:nrz:shape-normalization and def:nrz:covariance-shape.
///
/// TODO ´todo:test:test-compatible-measure-variation-across-bindings´: test compatible measure variation across bindings preserves checked conversion evidence under rule:nrz:binding-validation.
///
/// ´test:integration:affine-measures´
#[test]
fn affine_measures() -> TestResult {
    let kind = Kind::Named(
        "temperature".to_owned(),
        Box::new(Dimension::base(BaseDimension::Temperature)),
    );
    let celsius = Descriptor::new(
        kind,
        Measure::new("degC", Scale::Exact(Rational::one()), rational(27315, 100)?)?,
        Role::Point,
    )?;
    let kelvin = celsius.coherent();
    let conversion = celsius.conversion_to(&kelvin)?;
    assert_eq!(conversion.apply(&Rational::zero())?, rational(27315, 100)?);
    fahrenheit_conversion(&celsius)?;
    for coordinate in -10..=10 {
        let original = rational(coordinate, 1)?;
        assert_eq!(
            kelvin
                .conversion_to(&celsius)?
                .apply(&conversion.apply(&original)?)?,
            original
        );
    }
    assert_eq!(
        celsius
            .difference()
            .conversion_to(&kelvin.difference())?
            .shift,
        Rational::zero()
    );
    let registry = Registry::new(ExponentPolicy::Rational);
    assert_eq!(
        registry
            .binary(BinaryOp::Subtract, &celsius, &celsius)?
            .role(),
        Role::Difference
    );
    assert_eq!(
        registry
            .binary(BinaryOp::Add, &celsius, &celsius.difference())?
            .role(),
        Role::Point
    );
    assert_eq!(
        registry
            .binary(BinaryOp::Add, &celsius.difference(), &celsius)?
            .role(),
        Role::Point
    );
    assert_eq!(
        registry.binary(BinaryOp::Add, &celsius, &celsius),
        Err(UnitError::AffineRole)
    );
    assert_eq!(
        registry.binary(BinaryOp::Subtract, &celsius.difference(), &celsius),
        Err(UnitError::AffineRole)
    );
    assert_eq!(
        registry.power(&celsius, &Rational::zero()),
        Err(UnitError::AffineRole)
    );
    assert_eq!(registry.negate(&celsius), Err(UnitError::AffineRole));
    assert_eq!(
        Measure::new("degree", Scale::Symbolic("pi/180".into()), Rational::zero()),
        Err(UnitError::UnsupportedMeasureScale("pi/180".into()))
    );
    assert_eq!(
        Measure::new("bad", Scale::Exact(Rational::zero()), Rational::zero()),
        Err(UnitError::InvalidScale)
    );
    assert_eq!(
        Descriptor::new(
            celsius.kind().clone(),
            celsius.measure().clone(),
            Role::Difference
        ),
        Err(UnitError::OffsetOnDifference)
    );
    Ok(())
}

/// Check exact point and difference conversion into a measure with nonunit scale.
fn fahrenheit_conversion(celsius: &Descriptor) -> TestResult {
    let fahrenheit = Descriptor::new(
        celsius.kind().clone(),
        Measure::new("degF", Scale::Exact(rational(5, 9)?), rational(45967, 180)?)?,
        Role::Point,
    )?;
    let to_fahrenheit = celsius.conversion_to(&fahrenheit)?;
    assert_eq!(to_fahrenheit.scale, rational(9, 5)?);
    assert_eq!(to_fahrenheit.shift, rational(32, 1)?);
    for coordinate in -10..=10 {
        let coordinate = rational(coordinate, 1)?;
        let converted = to_fahrenheit.apply(&coordinate)?;
        assert_eq!(
            fahrenheit.conversion_to(celsius)?.apply(&converted)?,
            coordinate
        );
    }
    assert_eq!(
        fahrenheit
            .conversion_to(celsius)?
            .apply(&rational(32, 1)?)?,
        Rational::zero()
    );
    assert_eq!(
        fahrenheit
            .difference()
            .conversion_to(&celsius.difference())?
            .apply(&rational(18, 1)?)?,
        rational(10, 1)?
    );
    Ok(())
}

/// Semantic rules preserve kinds even when all dimension exponents agree.
///
/// TODO ´todo:test:test-dual-pairing-and-sensitivity-products´: test dual pairing and sensitivity products require explicit semantic relationships under rule:nrz:shape-transpose-and-duality and rule:nrz:shape-sensitivity-propagation.
///
/// ´test:integration:declared-kind-rules´
#[test]
fn declared_kind_rules() -> TestResult {
    let mass = named("mass", Dimension::base(BaseDimension::Mass))?;
    let stiffness = named("stiffness", Dimension::integers([0, 1, -2, 0, 0, 0, 0])?)?;
    let eigenvalue = named("eigenvalue", Dimension::integers([0, 0, -2, 0, 0, 0, 0])?)?;
    let omega = named(
        "angular_frequency",
        Dimension::integers([0, 0, -1, 0, 0, 0, 0])?,
    )?;
    let mode_shape = named("mode_shape", Dimension::zero())?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    assert_eq!(
        registry.binary(BinaryOp::Divide, &stiffness, &mass),
        Err(UnitError::UnsupportedKindRule)
    );
    registry.register_binary(
        BinaryOp::Divide,
        stiffness.kind().clone(),
        mass.kind().clone(),
        eigenvalue.kind().clone(),
    )?;
    assert_eq!(
        registry.binary(BinaryOp::Divide, &stiffness, &mass)?,
        eigenvalue
    );
    registry.register_power(
        eigenvalue.kind().clone(),
        rational(1, 2)?,
        omega.kind().clone(),
    )?;
    assert_eq!(registry.power(&eigenvalue, &rational(1, 2)?)?, omega);
    assert_eq!(
        mode_shape.conversion_to(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    assert_eq!(
        mode_shape.conversion_to(&omega),
        Err(UnitError::DimensionMismatch)
    );
    let energy = named("energy", Dimension::integers([2, 1, -2, 0, 0, 0, 0])?)?;
    let torque = named("torque", energy.dimension().clone())?;
    assert_eq!(
        energy.compatible_kind(&torque),
        Err(UnitError::KindMismatch)
    );
    registry.admit(&energy)?;
    assert_eq!(
        registry.admit(&named("energy", Dimension::zero())?),
        Err(UnitError::ConflictingKind("energy".to_owned()))
    );
    assert_eq!(
        registry.register_binary(
            BinaryOp::Multiply,
            mass.kind().clone(),
            mass.kind().clone(),
            omega.kind().clone()
        ),
        Err(UnitError::DimensionMismatch)
    );
    Ok(())
}

/// Counts retain integer domains and require an explicit ratio relation.
///
/// ´test:integration:count-domains´
#[test]
fn count_domains() -> TestResult {
    let count = Descriptor::new(
        Kind::Count("particles".to_owned(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let registry = Registry::new(ExponentPolicy::Rational);
    assert_eq!(
        count.compatible_kind(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    assert_eq!(
        count.compatible_kind(&named("amount", Dimension::base(BaseDimension::Amount))?),
        Err(UnitError::DimensionMismatch)
    );
    assert_eq!(
        count.check_count_literal(&rational(1, 2)?),
        Err(UnitError::CountValue)
    );
    assert_eq!(
        count.check_count_literal(&rational(-1, 1)?),
        Err(UnitError::CountValue)
    );
    count.check_count_literal(&rational(12, 1)?)?;
    registry.binary(BinaryOp::Add, &count, &count)?;
    assert_eq!(
        registry.binary(BinaryOp::Subtract, &count, &count),
        Err(UnitError::CountDomain)
    );
    assert_eq!(
        registry.binary(BinaryOp::Divide, &count, &count),
        Err(UnitError::CountDomain)
    );
    assert_eq!(registry.count_ratio(&count, &count)?, Descriptor::ratio());
    registry.scale_count(&count, &rational(2, 1)?)?;
    assert_eq!(
        registry.scale_count(&count, &rational(1, 2)?),
        Err(UnitError::CountValue)
    );
    assert_eq!(
        registry.scale_count(&count, &rational(-1, 1)?),
        Err(UnitError::CountDomain)
    );
    let bounded = CountDomain::new(Some(0), Some(10))?;
    assert!(!bounded.admits_addition());
    assert_eq!(
        bounded.admits_scale(&rational(2, 1)?),
        Err(UnitError::CountDomain)
    );
    assert_eq!(
        CountDomain::new(Some(5), Some(2)),
        Err(UnitError::CountDomain)
    );
    Ok(())
}

/// Uncertainty companions have independent dimensions, measures and semantic kinds.
///
/// TODO ´todo:test:test-vector-and-matrix-covariance-extents´: test vector and matrix covariance extents with mixed product dimensions under def:nrz:covariance-shape.
///
/// TODO ´todo:test:test-shared-schemas-preserve-distinct-bound´: test shared schemas preserve distinct bound value and source identities under def:nrz:heap-descriptor.
///
/// ´test:integration:uncertainty-descriptors´
#[test]
fn uncertainty_descriptors() -> TestResult {
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let time = named("time", Dimension::base(BaseDimension::Time))?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let radius = uncertainty(UncertaintyMode::Radius, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    assert_eq!(
        variance.dimension(),
        &length
            .dimension()
            .power(&rational(2, 1)?, ExponentPolicy::Rational)?
    );
    assert_eq!(
        standard.compatible_kind(&radius),
        Err(UnitError::KindMismatch)
    );
    let covariance = uncertainty(UncertaintyMode::Covariance, &length, Some(&time), None)?;
    assert_eq!(
        covariance.dimension(),
        &length.dimension().product(time.dimension())?
    );
    uncertainty_measures(&length, &time)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    assert_eq!(relative.dimension(), &Dimension::zero());
    assert_eq!(
        relative.compatible_kind(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    assert_eq!(
        uncertainty(
            UncertaintyMode::RelativeStandard,
            &length,
            None,
            Some((&length, false))
        ),
        Err(UnitError::RelativeReference)
    );
    assert_eq!(
        uncertainty(
            UncertaintyMode::RelativeStandard,
            &length,
            None,
            Some((&time, true))
        ),
        Err(UnitError::RelativeReference)
    );
    assert_eq!(
        uncertainty(UncertaintyMode::Covariance, &length, None, None),
        Err(UnitError::UncertaintyRelationship)
    );
    let registry = Registry::new(ExponentPolicy::Rational);
    assert_eq!(
        registry.power(&standard, &rational(2, 1)?)?,
        variance.coherent()
    );
    assert_eq!(
        registry.power(&variance, &rational(1, 2)?)?,
        standard.coherent()
    );
    let count = Descriptor::new(
        Kind::Count("events".to_owned(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let count_standard = uncertainty(UncertaintyMode::Standard, &count, None, None)?;
    assert!(count_standard.kind().count_domain().is_none());
    registry.binary(BinaryOp::Multiply, &count_standard, &Descriptor::ratio())?;
    Ok(())
}

/// Check squared and product scales independently of uncertainty identity tests.
///
/// TODO ´todo:test:test-cross-covariance-measure-products-on´: test cross covariance measure products on both logical axes under def:nrz:covariance-shape.
fn uncertainty_measures(length: &Descriptor, time: &Descriptor) -> TestResult {
    let centimetres = Descriptor::new(
        length.kind().clone(),
        Measure::new("cm", Scale::Exact(rational(1, 100)?), Rational::zero())?,
        Role::Difference,
    )?;
    let milliseconds = Descriptor::new(
        time.kind().clone(),
        Measure::new("ms", Scale::Exact(rational(1, 1000)?), Rational::zero())?,
        Role::Difference,
    )?;
    assert_eq!(
        uncertainty(UncertaintyMode::Variance, &centimetres, None, None)?
            .measure()
            .scale(),
        &rational(1, 10_000)?
    );
    assert_eq!(
        uncertainty(
            UncertaintyMode::Covariance,
            &centimetres,
            Some(&milliseconds),
            None
        )?
        .measure()
        .scale(),
        &rational(1, 100_000)?
    );
    Ok(())
}

/// Ratio coefficients preserve uncertainty kinds and relative construction checks a reference.
///
/// TODO ´todo:test:test-correlation-kinds-and-indexed-relative´: test correlation kinds and indexed relative references remain distinct from normalized covariance under rule:nrz:correlation-and-relative-shapes.
///
/// ´test:integration:ratio-uncertainty-descriptor-rules´
#[test]
fn ratio_uncertainty_descriptor_rules() -> TestResult {
    let ratio = Descriptor::ratio();
    let standard = uncertainty(UncertaintyMode::Standard, &ratio, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &ratio, None, None)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &ratio,
        None,
        Some((&ratio, true)),
    )?;
    let registry = Registry::new(ExponentPolicy::Rational);
    for descriptor in [&standard, &variance, &relative] {
        assert_eq!(
            registry.binary(BinaryOp::Divide, descriptor, &ratio)?,
            descriptor.coherent()
        );
    }
    assert_eq!(
        registry.relative(&standard, &ratio, true)?,
        relative.coherent()
    );
    assert_eq!(
        registry.relative(&standard, &ratio, false),
        Err(UnitError::RelativeReference)
    );
    assert_eq!(
        Descriptor::new(
            relative.kind().clone(),
            Measure::coherent(),
            Role::Difference
        ),
        Err(UnitError::RelativeReference)
    );
    assert_eq!(
        registry.binary(BinaryOp::Multiply, &standard, &standard),
        Err(UnitError::UnsupportedKindRule)
    );
    Ok(())
}

/// Unreachable registrations are refused before they can mislead a client.
///
/// ´test:integration:unreachable-operation-registrations´
#[test]
fn unreachable_operation_registrations() -> TestResult {
    let mut registry = Registry::new(ExponentPolicy::Rational);
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    assert_eq!(
        registry.register_power(Kind::Ratio, rational(2, 1)?, Kind::Ratio),
        Err(UnitError::UnsupportedKindRule)
    );
    assert_eq!(
        registry.register_power(
            length.kind().clone(),
            Rational::one(),
            length.kind().clone()
        ),
        Err(UnitError::UnsupportedKindRule)
    );
    let count = Kind::Count("events".to_owned(), CountDomain::NONNEGATIVE);
    assert_eq!(
        registry.register_binary(BinaryOp::Divide, count.clone(), count, Kind::Ratio),
        Err(UnitError::UnsupportedKindRule)
    );
    Ok(())
}

/// Relative kinds cannot be hidden inside another uncertainty's value or covariance member.
///
/// ´test:integration:nested-relative-refusals´
#[test]
fn nested_relative_refusals() -> TestResult {
    let ratio = Descriptor::ratio();
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &ratio,
        None,
        Some((&ratio, true)),
    )?;
    for kind in [
        Kind::Uncertainty(
            UncertaintyMode::Standard,
            Box::new(relative.kind().clone()),
            None,
        ),
        Kind::Uncertainty(
            UncertaintyMode::Covariance,
            Box::new(Kind::Ratio),
            Some(Box::new(relative.kind().clone())),
        ),
    ] {
        assert_eq!(
            Descriptor::new(kind, Measure::coherent(), Role::Difference),
            Err(UnitError::RelativeReference)
        );
    }
    assert_eq!(
        uncertainty(UncertaintyMode::Standard, &relative, None, None),
        Err(UnitError::RelativeReference)
    );
    Ok(())
}
