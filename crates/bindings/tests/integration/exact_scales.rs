// SPDX-License-Identifier: AGPL-3.0-only
//! Exact irrational measure conversion and explicit refusal at rational numerical boundaries.

use nrz_bindings::{
    Context, Declaration, Descriptor, Evidence, ExponentPolicy, Kind, MapError, Measure,
    PairingWitness, ReferenceDefinition, Registry, Role, Scale, SensitivityError,
    SensitivityRecipe, UncertaintyMode, UnitError, uncertainty,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits, LimitsOverride};
use nrz_cas::syntax::Constant;
use nrz_cas::{ExactConstant, Fault, FaultKind, Rational};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

fn rational(numerator: i64, denominator: u64) -> TestResult<Rational> {
    Ok(Rational::from_ratio(
        numerator,
        denominator,
        &mut ledger()?,
    )?)
}

fn pi() -> TestResult<ExactConstant> {
    Ok(ExactConstant::named(Constant::Pi, &mut ledger()?)?)
}

fn angle(name: &str, numerator: i64, denominator: u64) -> TestResult<Descriptor> {
    let scale = pi()?.mul(&rational(numerator, denominator)?.into(), &mut ledger()?)?;
    Ok(Descriptor::new(
        Kind::Angle,
        Measure::new(name, Scale::Expression(scale), Rational::zero())?,
        Role::Difference,
    )?)
}

fn owner() -> TestResult<nrz_bindings::CheckedDeclarations> {
    Ok(Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &[],
    )?
    .freeze()?)
}

/// Independent pi and rational expectations cover both directions of every angle conversion.
/// ´test:integration:irrational-angle-conversions-are-exact´
#[test]
fn irrational_angle_conversions_are_exact() -> TestResult {
    let degree = angle("degree", 1, 180)?;
    let turn = angle("turn", 2, 1)?;
    let radian = degree.coherent();
    let half_pi = pi()?.div(&rational(2, 1)?.into(), &mut ledger()?)?;
    let twice_pi = pi()?.mul(&rational(2, 1)?.into(), &mut ledger()?)?;
    assert_eq!(
        degree
            .conversion_to(&radian)?
            .apply_exact(&rational(90, 1)?.into())?,
        half_pi
    );
    assert_eq!(
        radian.conversion_to(&degree)?.apply_exact(&half_pi)?,
        rational(90, 1)?
    );
    assert_eq!(
        turn.conversion_to(&radian)?
            .apply_exact(&ExactConstant::one())?,
        twice_pi
    );
    assert_eq!(
        radian.conversion_to(&turn)?.apply_exact(&twice_pi)?,
        ExactConstant::one()
    );
    let fraction = degree.conversion_to(&turn)?;
    assert_eq!(fraction.scale.as_rational(), Some(&rational(1, 360)?));
    assert_eq!(fraction.apply(&rational(90, 1)?)?, rational(1, 4)?);
    assert_eq!(
        turn.conversion_to(&degree)?.apply(&rational(1, 4)?)?,
        rational(90, 1)?
    );
    assert_eq!(fraction.shift, Rational::zero());
    assert_eq!(
        degree.conversion_to(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    Ok(())
}

/// Measure admission needs a positive finite witness, independent of the measure's spelling.
/// ´test:integration:scale-positivity-refusals-are-exact´
#[test]
fn scale_positivity_refusals_are_exact() -> TestResult {
    let unknown = pi()?.sub(&ExactConstant::one(), &mut ledger()?)?;
    for value in [ExactConstant::zero(), rational(-1, 1)?.into(), unknown] {
        assert_eq!(
            Measure::new("positive pi", Scale::Expression(value), Rational::zero()),
            Err(UnitError::InvalidScale)
        );
    }
    assert_eq!(
        Measure::new("pi", Scale::Symbolic("undeclared".into()), Rational::zero()),
        Err(UnitError::UnsupportedMeasureScale("undeclared".into()))
    );
    let degree = angle("uninterpreted name", 1, 180)?;
    assert_eq!(
        degree.measure().scale_witness().value(),
        degree.measure().scale()
    );
    assert_eq!(
        Descriptor::new(
            Kind::Count("turns".into(), nrz_bindings::CountDomain::NONNEGATIVE),
            degree.measure().clone(),
            Role::Difference
        ),
        Err(UnitError::CountMeasure)
    );
    Ok(())
}

/// Rational measure origins produce exact irrational coordinate shifts when divided by a scale.
/// ´test:integration:irrational-scales-preserve-affine-offset-rules´
#[test]
fn irrational_scales_preserve_affine_offset_rules() -> TestResult {
    let measure = Measure::new("offset angle", Scale::Expression(pi()?), rational(1, 1)?)?;
    let point = Descriptor::new(Kind::Angle, measure.clone(), Role::Point)?;
    assert_eq!(
        Descriptor::new(Kind::Angle, measure, Role::Difference),
        Err(UnitError::OffsetOnDifference)
    );
    let forward = point.conversion_to(&point.coherent())?;
    assert_eq!(forward.shift, Rational::one());
    let inverse = point.coherent().conversion_to(&point)?;
    let expected_shift = rational(-1, 1)?.into();
    assert_eq!(
        inverse.shift,
        pi()?
            .pow(-1, &mut ledger()?)?
            .mul(&expected_shift, &mut ledger()?)?
    );
    assert_eq!(
        inverse.apply_exact(&forward.apply_exact(&rational(2, 1)?.into())?)?,
        rational(2, 1)?
    );
    assert_eq!(point.difference().measure().offset(), &Rational::zero());
    assert_eq!(
        point
            .difference()
            .conversion_to(&point.coherent().difference())?
            .shift,
        Rational::zero()
    );
    Ok(())
}

/// Variance, covariance and sensitivity retain both exact scale factors without origins.
/// ´test:integration:irrational-companion-and-sensitivity-scales-compose´
#[test]
fn irrational_companion_and_sensitivity_scales_compose() -> TestResult {
    let degree = angle("degree", 1, 180)?;
    let turn = angle("turn", 2, 1)?;
    let variance = uncertainty(UncertaintyMode::Variance, &degree, None, None)?;
    let covariance = uncertainty(UncertaintyMode::Covariance, &degree, Some(&turn), None)?;
    assert_eq!(
        variance.measure().scale(),
        &pi()?
            .pow(2, &mut ledger()?)?
            .mul(&rational(1, 32_400)?.into(), &mut ledger()?)?
    );
    assert_eq!(
        covariance.measure().scale(),
        &pi()?
            .pow(2, &mut ledger()?)?
            .mul(&rational(1, 90)?.into(), &mut ledger()?)?
    );
    let quotient = PairingWitness::sensitivity(&degree, &turn)?;
    assert_eq!(quotient.measure().rational_scale()?, &rational(1, 360)?);
    assert_eq!(covariance.measure().offset(), &Rational::zero());
    Ok(())
}

/// Descriptor maps retain irrational coefficients and refuse the rational decoder by name.
/// ´test:integration:irrational-constant-exports-refuse-explicitly´
#[test]
fn irrational_constant_exports_refuse_explicitly() -> TestResult {
    let owner = owner()?;
    let degree = angle("degree", 1, 180)?;
    let map = owner.conversion_map(&degree, &degree.coherent())?;
    let refusal = MapError::Units(UnitError::UnsupportedExactConstantExport);
    assert_eq!(
        map.scale().export(&LimitsOverride::default()),
        Err(refusal.clone())
    );
    assert_eq!(map.shift().export(&LimitsOverride::default())?, "0");
    assert_eq!(
        map.inverse()?.scale().export(&LimitsOverride::default()),
        Err(refusal.clone())
    );
    assert_eq!(map.scale().exact(), degree.measure().scale());
    assert_eq!(
        degree.measure().rational_scale(),
        Err(UnitError::UnsupportedExactConstantExport)
    );
    let conversion = degree.conversion_to(&degree.coherent())?;
    assert_eq!(
        conversion.export_scale(&LimitsOverride::default()),
        Err(refusal)
    );
    assert_eq!(
        conversion.apply(&Rational::one()),
        Err(UnitError::UnsupportedExactConstantExport)
    );
    Ok(())
}

/// Positive irrational reference definitions normalize exactly and preserve cancellation at export.
/// ´test:integration:irrational-normalization-references-retain-exact-identity´
#[test]
fn irrational_normalization_references_retain_exact_identity() -> TestResult {
    let owner = owner()?;
    let degree = angle("degree", 1, 180)?;
    let radian = degree.coherent();
    let reference = ReferenceDefinition::new("half turn", &radian, Scale::Expression(pi()?))?;
    let map =
        owner.schema_normalization_map(&degree, &reference, None, &LimitsOverride::default())?;
    assert_eq!(map.scale().export(&LimitsOverride::default())?, "1/180");
    assert_eq!(
        map.inverse()?.scale().export(&LimitsOverride::default())?,
        "180"
    );
    let irrational =
        owner.schema_normalization_map(&radian, &reference, None, &LimitsOverride::default())?;
    assert_eq!(
        irrational.scale().export(&LimitsOverride::default()),
        Err(MapError::Units(UnitError::UnsupportedExactConstantExport))
    );
    assert_eq!(
        ReferenceDefinition::new(
            "unproved",
            &radian,
            Scale::Expression(pi()?.sub(&ExactConstant::one(), &mut ledger()?)?)
        ),
        Err(MapError::Reference)
    );
    Ok(())
}

fn input_context(descriptor: &Descriptor, limits: Limits) -> TestResult<Context> {
    let mut builder =
        Context::with_real_symbols(Registry::new(ExponentPolicy::Rational), limits, &["x"])?;
    builder.declare(
        "x".into(),
        Declaration::Input {
            descriptor: descriptor.clone(),
            nonzero: false,
            evidence: Evidence::default(),
        },
    )?;
    Ok(builder)
}

/// Original checking imports the exact scale graph; rational-only sensitivity export refuses it.
/// ´test:integration:irrational-coordinate-checking-and-native-refusal´
#[test]
fn irrational_coordinate_checking_and_native_refusal() -> TestResult {
    let descriptor = Descriptor::new(
        Kind::Ratio,
        angle("pi", 1, 1)?.measure().clone(),
        Role::Difference,
    )?;
    let builder = input_context(&descriptor, Limits::default())?;
    let original = builder.parse("square", "x^2")?;
    let checked = builder.check(&original)?;
    assert_eq!(checked.descriptor(), &Descriptor::ratio());
    assert!(matches!(
        builder.check_sensitivity(
            SensitivityRecipe::Square,
            &[original],
            &["x".into()],
            &LimitsOverride::default()
        ),
        Err(SensitivityError::Units(
            UnitError::UnsupportedExactConstantExport
        ))
    ));
    Ok(())
}

/// New coefficient graph construction respects the schema caller's exact cumulative work boundary.
/// ´test:integration:irrational-schema-coefficient-work-boundary´
#[test]
fn irrational_schema_coefficient_work_boundary() -> TestResult {
    let owner = owner()?;
    let degree = angle("degree", 1, 180)?;
    let reference = ReferenceDefinition::unit("radian", &degree.coherent())?;
    let operation = |work| {
        owner.schema_normalization_map(
            &degree,
            &reference,
            None,
            &LimitsOverride {
                work: Some(work),
                ..LimitsOverride::default()
            },
        )
    };
    let mut low = 0;
    let mut high = Limits::default().work;
    operation(high)?;
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if operation(middle).is_ok() {
            high = middle;
        } else {
            low = middle;
        }
    }
    operation(high)?;
    assert!(matches!(
        operation(high - 1),
        Err(MapError::Algebra(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        }))
    ));
    Ok(())
}

/// Root and solve schedules retain the same explicit rational-only coefficient refusal.
/// ´test:integration:irrational-implicit-sensitivity-exports-refuse´
#[test]
fn irrational_implicit_sensitivity_exports_refuse() -> TestResult {
    let descriptor = Descriptor::new(
        Kind::Ratio,
        angle("pi", 1, 1)?.measure().clone(),
        Role::Difference,
    )?;
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["x", "t"],
    )?;
    for (key, meaning) in [("x", &descriptor), ("t", &Descriptor::ratio())] {
        builder.declare(
            key.into(),
            Declaration::Input {
                descriptor: meaning.clone(),
                nonzero: false,
                evidence: Evidence::default(),
            },
        )?;
    }
    let original = builder.parse("root", "sqrt(x)")?;
    assert!(matches!(
        builder.check_root_sensitivity(
            &[original],
            &["x".into()],
            &["t".into()],
            &LimitsOverride::default()
        ),
        Err(SensitivityError::Units(
            UnitError::UnsupportedExactConstantExport
        ))
    ));
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["a", "b", "x"],
    )?;
    for key in ["a", "b", "x"] {
        builder.declare(
            key.into(),
            Declaration::Input {
                descriptor: descriptor.clone(),
                nonzero: false,
                evidence: Evidence::default(),
            },
        )?;
    }
    let symbols = nrz_bindings::SolveSymbols {
        order: 1,
        right_hand_sides: 1,
        operator: vec!["a".into()],
        right_hand_side: vec!["b".into()],
        solution: vec!["x".into()],
    };
    assert!(matches!(
        builder.check_solve_sensitivity(&symbols, &LimitsOverride::default()),
        Err(SensitivityError::Units(
            UnitError::UnsupportedExactConstantExport
        ))
    ));
    Ok(())
}
