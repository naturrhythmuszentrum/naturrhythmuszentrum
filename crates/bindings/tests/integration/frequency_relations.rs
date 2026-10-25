// SPDX-License-Identifier: AGPL-3.0-only
//! Registered angle measures and typed full-cycle relations have independent exact LC oracles.

use nrz_bindings::{
    Context, Declaration, Descriptor, Evidence, ExponentPolicy, FrequencyDirection, Kind, Measure,
    Registry, Role, Scale, UnitError,
};
use nrz_cas::limits::{Budget, Limits, LimitsOverride};
use nrz_cas::request::Outcome;
use nrz_cas::syntax::Constant;
use nrz_cas::{ExactConstant, Rational};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn rational(numerator: i64, denominator: u64) -> TestResult<Rational> {
    Ok(Rational::from_ratio(
        numerator,
        denominator,
        &mut Budget::new(Limits::default())?,
    )?)
}

fn pi() -> TestResult<ExactConstant> {
    Ok(ExactConstant::named(
        Constant::Pi,
        &mut Budget::new(Limits::default())?,
    )?)
}

fn measured<'a>(registry: &'a Registry, key: &str) -> TestResult<&'a Descriptor> {
    registry
        .measure(key)
        .ok_or_else(|| format!("missing registered measure {key}").into())
}

/// Catalog angle conversions preserve independently specified exact constants in both directions.
/// ´test:integration:registered-angle-measures-roundtrip-exactly´
#[test]
fn registered_angle_measures_roundtrip_exactly() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let radian = measured(&registry, "rad")?;
    let degree = measured(&registry, "degree")?;
    let turn = measured(&registry, "turn")?;
    let mut ledger = Budget::new(Limits::default())?;
    let two_pi = pi()?.mul(&rational(2, 1)?.into(), &mut ledger)?;
    assert_eq!(
        degree
            .conversion_to(radian)?
            .apply_exact(&rational(180, 1)?.into())?,
        pi()?
    );
    assert_eq!(
        radian.conversion_to(degree)?.apply_exact(&pi()?)?,
        rational(180, 1)?
    );
    assert_eq!(
        turn.conversion_to(radian)?
            .apply_exact(&ExactConstant::one())?,
        two_pi
    );
    assert_eq!(
        radian.conversion_to(turn)?.apply_exact(&two_pi)?,
        Rational::one()
    );
    assert_eq!(
        degree.conversion_to(turn)?.scale.as_rational(),
        Some(&rational(1, 360)?)
    );
    assert_eq!(
        turn.conversion_to(degree)?.scale.as_rational(),
        Some(&rational(360, 1)?)
    );
    assert_eq!(
        degree.conversion_to(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    Ok(())
}

/// The exact LC frequency and angular frequency are related through the registered full turn.
/// ´test:integration:registered-frequency-relation-matches-lc-reference´
#[test]
fn registered_frequency_relation_matches_lc_reference() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let hertz = measured(&registry, "Hz")?;
    let angular = measured(&registry, "rad/s")?;
    let relation = registry.frequency_relation(hertz, angular)?;
    let mut ledger = Budget::new(Limits::default())?;
    let frequency = ExactConstant::from(rational(1_000, 1)?).div(&pi()?, &mut ledger)?;
    let omega = ExactConstant::from(rational(2_000, 1)?);
    let expected_factor = pi()?.mul(&rational(2, 1)?.into(), &mut ledger)?;
    assert_eq!(relation.direction(), FrequencyDirection::ToAngular);
    assert_eq!(relation.source(), hertz);
    assert_eq!(relation.target(), angular);
    assert_eq!(relation.cycle(), measured(&registry, "turn")?);
    assert_eq!(relation.conversion().scale, expected_factor);
    assert_eq!(relation.apply_exact(hertz, &frequency)?, omega);
    let inverse = relation.inverse()?;
    assert_eq!(inverse, registry.frequency_relation(angular, hertz)?);
    assert_eq!(inverse.direction(), FrequencyDirection::ToCyclic);
    assert_eq!(inverse.apply_exact(angular, &omega)?, frequency);
    assert_eq!(inverse.inverse()?, relation);
    assert_eq!(hertz.conversion_to(angular), Err(UnitError::KindMismatch));
    assert_eq!(angular.conversion_to(hertz), Err(UnitError::KindMismatch));
    Ok(())
}

/// Source and target measure factors compose with the full turn, rather than replacing it.
/// ´test:integration:frequency-relation-composes-both-coordinate-measures´
#[test]
fn frequency_relation_composes_both_coordinate_measures() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let kilohertz = Descriptor::new(
        Kind::frequency()?,
        Measure::new("kHz", Scale::Exact(rational(1_000, 1)?), Rational::zero())?,
        Role::Difference,
    )?;
    let mut ledger = Budget::new(Limits::default())?;
    let scale = pi()?.div(&rational(30, 1)?.into(), &mut ledger)?;
    let revolutions = Descriptor::new(
        Kind::angular_frequency()?,
        Measure::new("turn/min", Scale::Expression(scale), Rational::zero())?,
        Role::Difference,
    )?;
    let relation = registry.frequency_relation(&kilohertz, &revolutions)?;
    assert_eq!(
        relation.conversion().scale.as_rational(),
        Some(&rational(60_000, 1)?)
    );
    assert_eq!(
        relation.apply_exact(&kilohertz, &ExactConstant::one())?,
        rational(60_000, 1)?
    );
    assert_eq!(
        relation.apply_exact(measured(&registry, "Hz")?, &ExactConstant::one()),
        Err(UnitError::RelationSource)
    );
    Ok(())
}

/// Names, equal dimensions and affine role cannot invent the registered full-cycle relation.
/// ´test:integration:frequency-relation-refuses-kind-role-and-registration-mismatches´
#[test]
fn frequency_relation_refuses_kind_role_and_registration_mismatches() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let hertz = measured(&registry, "Hz")?;
    let angular = measured(&registry, "rad/s")?;
    let rate = measured(&registry, "s^-1")?;
    assert_eq!(hertz.dimension(), angular.dimension());
    assert_eq!(angular.dimension(), rate.dimension());
    assert_eq!(rate.conversion_to(hertz), Err(UnitError::KindMismatch));
    assert_eq!(rate.conversion_to(angular), Err(UnitError::KindMismatch));
    assert_eq!(
        registry.frequency_relation(rate, hertz),
        Err(UnitError::KindMismatch)
    );
    assert_eq!(
        registry.frequency_relation(hertz, hertz),
        Err(UnitError::KindMismatch)
    );
    let point = Descriptor::new(hertz.kind().clone(), hertz.measure().clone(), Role::Point)?;
    assert_eq!(
        registry.frequency_relation(&point, angular),
        Err(UnitError::AffineRole)
    );
    let mut empty = Registry::new(ExponentPolicy::Rational);
    empty.register_measure("turn", &Descriptor::ratio())?;
    assert_eq!(
        empty.frequency_relation(hertz, angular),
        Err(UnitError::UnsupportedKindRule)
    );
    assert_eq!(
        empty.register_measure("turn", hertz),
        Err(UnitError::DuplicateRule)
    );
    Ok(())
}

/// The physical LC equation keeps original frequency and cycle declarations before CAS proof.
/// ´test:integration:lc-frequency-equation-retains-original-typed-operands´
#[test]
fn lc_frequency_equation_retains_original_typed_operands() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let angular = measured(&registry, "rad/s")?.clone();
    let cycle = measured(&registry, "turn")?.clone();
    let inverse_pi = pi()?.pow(-1, &mut Budget::new(Limits::default())?)?;
    let frequency = Descriptor::new(
        Kind::frequency()?,
        Measure::new("Hz/pi", Scale::Expression(inverse_pi), Rational::zero())?,
        Role::Difference,
    )?;
    let mut builder =
        Context::with_real_symbols(registry, Limits::default(), &["f", "cycle", "omega"])?;
    for (key, descriptor, value) in [
        ("f", frequency, 1_000),
        ("cycle", cycle, 1),
        ("omega", angular, 2_000),
    ] {
        builder.declare(
            key.into(),
            Declaration::Constant {
                descriptor,
                value: rational(value, 1)?,
                evidence: Evidence::default(),
            },
        )?;
    }
    let left = builder.parse("full cycle", "cycle*f")?;
    let right = builder.parse("angular frequency", "omega")?;
    let (checked, _) = builder.check_equation(&left, &right)?;
    assert_eq!(checked.original().root, left.root);
    assert_eq!(checked.descriptor().kind(), &Kind::angular_frequency()?);
    assert!(checked.declarations().contains_key("f"));
    assert!(checked.declarations().contains_key("cycle"));
    let expected = builder.parse("independent LC coordinate", "2000")?;
    assert!(matches!(
        builder.prove_equal(&checked, &expected, LimitsOverride::default())?,
        Outcome::Proved { .. }
    ));
    Ok(())
}

/// Checked full-cycle maps retain snapshot identity, direction and exact pi factors.
/// ´test:integration:pi-frequency-map-retains-original-relation´
#[test]
fn pi_frequency_map_retains_original_relation() -> TestResult {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let owner = Context::with_real_symbols(registry, Limits::default(), &[])?.freeze()?;
    let hertz = measured(owner.registry(), "Hz")?;
    let angular = measured(owner.registry(), "rad/s")?;
    let forward = owner.frequency_map(hertz, angular)?;
    let reverse = owner.frequency_map(angular, hertz)?;
    assert_eq!(
        forward.id().kind(),
        nrz_bindings::MapKind::ToAngularFrequency
    );
    assert_eq!(
        reverse.id().kind(),
        nrz_bindings::MapKind::ToCyclicFrequency
    );
    assert_eq!(forward.inverse()?, reverse);
    assert_eq!(
        forward.scale().exact(),
        &owner
            .registry()
            .frequency_relation(hertz, angular)?
            .conversion()
            .scale
    );
    for (map, factor, operation) in [
        (&forward, "2", nrz_bindings::PiFactor::Multiply),
        (&reverse, "1/2", nrz_bindings::PiFactor::Divide),
    ] {
        let exported = map.scale().enclosure_export(&LimitsOverride::default())?;
        assert_eq!(exported.rational_factor(), factor);
        assert_eq!(exported.pi_factor(), operation);
        assert!(std::ptr::eq(exported.definition(), map.scale()));
    }
    let foreign = Context::with_real_symbols(
        Registry::with_angle_frequency(ExponentPolicy::Rational)?,
        Limits::default(),
        &[],
    )?
    .freeze()?;
    assert!(forward.validate_snapshot(&foreign).is_err());
    assert!(owner.frequency_map(hertz, hertz).is_err());
    assert!(owner.conversion_map(hertz, angular).is_err());
    Ok(())
}
