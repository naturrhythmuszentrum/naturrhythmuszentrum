// SPDX-License-Identifier: AGPL-3.0-only
//! Original-operation checks and declared observable contracts.

use nrz_algebra::{Error, Expr, Limits, NormalForm, Rational, Symbol, parse};
use nrz_units::{
    BaseDimension, BinaryOp, Companion, Context, CountDomain, Declaration, Descriptor, Dimension,
    Evidence, Expectation, ExponentPolicy, Failure, FunctionSignature, Kind, Measure, Observable,
    Registry, Role, Scale, UncertaintyMode, UnitError, uncertainty,
};

/// Shared fallible fixture result.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse an original expression under the documented default limits.
fn expression(source: &str) -> Result<Expr, nrz_algebra::ParseError> {
    parse(source, Limits::default())
}

/// A physical kind with a coherent linear measure.
fn named(name: &str, dimension: Dimension) -> Result<Descriptor, Box<dyn std::error::Error>> {
    Ok(Descriptor::new(
        Kind::Named(Symbol::new(name)?, dimension),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Register a source input with a visible provenance and assurance claim.
fn input(context: &mut Context, name: &str, descriptor: &Descriptor, nonzero: bool) -> TestResult {
    context.declare(
        Symbol::new(name)?,
        Declaration::Input {
            descriptor: descriptor.clone(),
            nonzero,
            evidence: Evidence {
                source: "fixture".into(),
                assurance: "declared, not certified".into(),
            },
        },
    )?;
    Ok(())
}

/// Bind an absolute uncertainty input to its actual value symbol.
fn uncertainty_input(
    context: &mut Context,
    name: &str,
    descriptor: &Descriptor,
    value: &str,
    mode: UncertaintyMode,
) -> TestResult {
    context.declare(
        Symbol::new(name)?,
        Declaration::UncertaintyInput {
            descriptor: descriptor.clone(),
            companion: Companion {
                mode,
                value: Symbol::new(value)?,
                other: None,
                reference: None,
            },
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Assert the reason for refusal, not merely the absence of a successful result.
fn refusal(context: &Context, source: &str, expected: &Failure) -> TestResult {
    let diagnostic = context
        .check(&expression(source)?)
        .err()
        .ok_or("expected refusal")?;
    assert_eq!(&diagnostic.failure, expected, "{source}");
    Ok(())
}

/// Original invalid operations cannot disappear through cancellation or multiplication by zero.
///
/// TODO ´todo:test:test-typed-matrix-zeros-and-cancelled´: test typed matrix zeros and cancelled contractions retain original axis checks under rule:nrz:shape-entry-contract and rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:original-operations-before-normalization´
#[test]
fn original_operations_before_normalization() -> TestResult {
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let time = named("time", Dimension::base(BaseDimension::Time))?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "x", &length, false)?;
    input(&mut context, "t", &time, false)?;
    for source in ["0*(x+t)", "(x+t)-(x+t)", "(x+t)^0"] {
        refusal(&context, source, &UnitError::DimensionMismatch.into())?;
    }
    refusal(
        &context,
        "0*missing",
        &Failure::UnknownSymbol(Symbol::new("missing")?),
    )?;
    let failure = context
        .check(&expression("0*(x+t)")?)
        .err()
        .ok_or("missing physical diagnostic")?;
    assert_eq!(failure.location.path, vec![1]);
    assert!(failure.location.span.is_some());
    assert_eq!(failure.derived.len(), 2);
    assert!(matches!(
        failure.failure,
        Failure::Units(UnitError::DimensionMismatch)
    ));
    let zero = context.check(&expression("x-x")?)?;
    assert_eq!(zero.descriptor().kind(), length.kind());
    assert_eq!(
        zero.normal_form(Limits::default())?.constant(),
        Some(Rational::ZERO)
    );
    let error = context
        .check_against(&expression("0")?, &length)
        .err()
        .ok_or("expected physical zero refusal")?;
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    let source = expression("x/x")?;
    let quotient = context.check(&source)?;
    assert_eq!(quotient.original(), &source);
    assert_eq!(
        quotient.normal_form(Limits::default())?.constant(),
        Some(Rational::ONE)
    );
    assert_eq!(quotient.nodes().len(), 3);
    assert!(quotient.declarations().contains_key(&Symbol::new("x")?));
    assert_eq!(
        context
            .check(&expression("0*(1/0)")?)?
            .normal_form(Limits::default()),
        Err(Error::Arithmetic(
            nrz_algebra::ArithmeticError::ZeroDenominator
        ))
    );
    context.declare(
        Symbol::new("zero_length")?,
        Declaration::Constant {
            descriptor: length.clone(),
            value: Rational::ZERO,
            evidence: Evidence::default(),
        },
    )?;
    context.check_against(&expression("zero_length")?, &length)?;
    Ok(())
}

/// Coordinate conversions retain exact affine and multiplicative evidence.
///
/// TODO ´todo:test:test-operator-normalization-and-paired-covariance´: test operator normalization and paired covariance conversions preserve coordinate meaning under def:nrz:shape-normalization and def:nrz:covariance-shape.
///
/// ´test:integration:coordinate-evidence´
#[test]
fn coordinate_evidence() -> TestResult {
    let temperature = Kind::Named(
        Symbol::new("temperature")?,
        Dimension::base(BaseDimension::Temperature),
    );
    let point = Descriptor::new(
        temperature,
        Measure::new(
            "Celsius",
            Scale::Exact(Rational::ONE),
            Rational::new(27315, 100)?,
        )?,
        Role::Point,
    )?;
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let millimetres = Descriptor::new(
        length.kind().clone(),
        Measure::new("mm", Scale::Exact(Rational::new(1, 1000)?), Rational::ZERO)?,
        Role::Difference,
    )?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "a", &point, false)?;
    input(&mut context, "b", &point, false)?;
    input(&mut context, "dx", &point.difference(), false)?;
    input(&mut context, "mm", &millimetres, false)?;
    let difference = context.check(&expression("a-b")?)?;
    assert_eq!(difference.descriptor().role(), Role::Difference);
    let celsius_offset = Rational::new(27315, 100)?;
    assert!(
        difference
            .nodes()
            .iter()
            .any(|node| node.conversion.shift == celsius_offset)
    );
    context.check(&expression("a+dx")?)?;
    context.check(&expression("dx+a")?)?;
    assert_eq!(
        difference.normal_form(Limits::default())?,
        NormalForm::new(&expression("a-b")?, Limits::default())?
    );
    context.check(&expression("a+(b-a)/2")?)?;
    for source in ["a+b", "dx-a", "0*a", "a/a", "a^0", "-a", "0.5*a+0.5*b"] {
        refusal(&context, source, &UnitError::AffineRole.into())?;
    }
    let checked = context.check_against(&expression("mm")?, &length)?;
    let millimetre_scale = Rational::new(1, 1000)?;
    assert!(
        checked
            .nodes()
            .iter()
            .any(|node| node.conversion.scale == millimetre_scale)
    );
    assert_eq!(
        checked
            .output_conversion()
            .ok_or("missing output conversion")?
            .scale,
        Rational::ONE
    );
    Ok(())
}

/// Derived eigenvalues and homogeneous blocks check declared physical relations.
///
/// TODO ´todo:test:test-rectangular-block-extents-and-ordered´: test rectangular block extents and ordered equation axes alongside scalar descriptor checks under def:nrz:matrix-shape and rule:nrz:shape-entry-contract.
///
/// ´test:integration:observables-equations-and-blocks´
#[test]
fn observables_equations_and_blocks() -> TestResult {
    let mass = named("mass", Dimension::base(BaseDimension::Mass))?;
    let stiffness = named("stiffness", Dimension::integers([0, 1, -2, 0, 0, 0, 0]))?;
    let eigenvalue = named("eigenvalue", Dimension::integers([0, 0, -2, 0, 0, 0, 0]))?;
    let omega = named(
        "angular_frequency",
        Dimension::integers([0, 0, -1, 0, 0, 0, 0]),
    )?;
    let mode_shape = named("mode_shape", Dimension::ZERO)?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Divide,
        stiffness.kind().clone(),
        mass.kind().clone(),
        eigenvalue.kind().clone(),
    )?;
    registry.register_power(
        eigenvalue.kind().clone(),
        Rational::new(1, 2)?,
        omega.kind().clone(),
    )?;
    let mut context = Context::new(registry, Limits::default());
    input(&mut context, "K", &stiffness, false)?;
    input(&mut context, "M", &mass, true)?;
    input(&mut context, "shape", &mode_shape, false)?;
    context.declare(
        Symbol::new("lambda")?,
        Declaration::Derived(Observable {
            descriptor: eigenvalue,
            expression: expression("K/M")?,
            evidence: Evidence::default(),
        }),
    )?;
    context.check_against(&expression("sqrt(lambda)")?, &omega)?;
    context.check_equation(&expression("lambda")?, &expression("K/M")?)?;
    let error = context
        .check_against(&expression("shape")?, &omega)
        .err()
        .ok_or("expected shape mismatch")?;
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    assert_eq!(
        context
            .check_block(&[expression("K")?, expression("2*K")?], &stiffness)?
            .len(),
        2
    );
    let failure = context
        .check_block(&[expression("K")?, expression("M")?], &stiffness)
        .err()
        .ok_or("missing block failure")?;
    assert_eq!(failure.location.path, vec![1]);
    assert!(matches!(failure.expected, Expectation::Descriptor(_)));
    context.check_all()?;
    Ok(())
}

/// Function signatures distinguish angles, ratios and unrelated dimensionless kinds.
///
/// TODO ´todo:test:test-matrix-function-signatures-reject-wrong´: test matrix function signatures reject wrong axes and unsupported contraction semantics under rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:function-signatures´
#[test]
fn function_signatures() -> TestResult {
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let shape = named("mode_shape", Dimension::ZERO)?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "theta", &angle, false)?;
    input(&mut context, "shape", &shape, false)?;
    for source in [
        "sin(theta)",
        "cos(theta)",
        "tan(theta)",
        "sin(asin(0.5))",
        "exp(log(2))",
    ] {
        context.check_against(&expression(source)?, &Descriptor::ratio())?;
    }
    for source in ["sin(1)", "sin(shape)", "log(theta)", "asin(theta)"] {
        refusal(&context, source, &UnitError::KindMismatch.into())?;
    }
    refusal(
        &context,
        "sin()",
        &Failure::Arity {
            expected: 1,
            derived: 0,
        },
    )?;
    refusal(
        &context,
        "sin(theta,theta)",
        &Failure::Arity {
            expected: 1,
            derived: 2,
        },
    )?;
    refusal(
        &context,
        "unknown(theta)",
        &Failure::UnknownFunction(Symbol::new("unknown")?),
    )?;
    let diagnostic = context
        .check(&expression("sin(shape)")?)
        .err()
        .ok_or("missing function diagnostic")?;
    assert_eq!(diagnostic.location.path, vec![0]);
    assert_eq!(
        diagnostic.location.span,
        Some(nrz_algebra::Span::new(4, 9)?)
    );
    assert_eq!(
        diagnostic.expected,
        Expectation::Descriptor(Box::new(angle.clone()))
    );
    assert_eq!(diagnostic.derived, vec![shape.clone()]);
    context.declare_function(
        Symbol::new("project")?,
        FunctionSignature {
            parameters: vec![shape.clone(), angle.clone()],
            result: Descriptor::ratio(),
        },
    )?;
    context.check(&expression("project(shape,theta)")?)?;
    refusal(
        &context,
        "project(theta,shape)",
        &UnitError::KindMismatch.into(),
    )?;
    let error = context
        .declare_function(
            Symbol::new("sin")?,
            FunctionSignature {
                parameters: vec![angle],
                result: shape,
            },
        )
        .err()
        .ok_or("expected reserved function refusal")?;
    assert_eq!(error.failure, Failure::Duplicate(Symbol::new("sin")?));
    Ok(())
}

/// Count imports and expressions stay discrete while their uncertainty can be continuous.
///
/// ´test:integration:counts-and-metadata´
#[test]
fn counts_and_metadata() -> TestResult {
    let count = Descriptor::new(
        Kind::Count(Symbol::new("events")?, CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let sigma = uncertainty(UncertaintyMode::Standard, &count, None, None)?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "n", &count, true)?;
    uncertainty_input(
        &mut context,
        "sigma_n",
        &sigma,
        "n",
        UncertaintyMode::Standard,
    )?;
    input(&mut context, "r", &Descriptor::ratio(), false)?;
    context.declare(
        Symbol::new("label")?,
        Declaration::Metadata("display only".into()),
    )?;
    for source in [
        "n+n",
        "2*n",
        "n*2",
        "count_ratio(n,n)",
        "0.5*sigma_n",
        "sigma_n/n",
    ] {
        context.check(&expression(source)?)?;
    }
    for source in ["n/n", "n*0.5", "n*r", "-n", "n^0", "n-n"] {
        let error = if source == "n*0.5" {
            UnitError::CountValue
        } else {
            UnitError::CountDomain
        };
        refusal(&context, source, &error.into())?;
    }
    for source in ["n+1", "n-1", "count_ratio(n,r)"] {
        refusal(&context, source, &UnitError::KindMismatch.into())?;
    }
    refusal(
        &context,
        "0*label",
        &Failure::Metadata(Symbol::new("label")?),
    )?;
    let error = context
        .declare(
            Symbol::new("fractional_count")?,
            Declaration::Constant {
                descriptor: count,
                value: Rational::new(1, 2)?,
                evidence: Evidence::default(),
            },
        )
        .err()
        .ok_or("expected count import refusal")?;
    assert_eq!(error.failure, Failure::Units(UnitError::CountValue));
    Ok(())
}

/// Uncertainty expressions validate their own declarations and their reference conditions.
///
/// TODO ´todo:test:test-marginal-arrays-and-full-covariance´: test marginal arrays and full covariance derive independent shapes and paired kinds under def:nrz:companion-shape and def:nrz:covariance-shape.
///
/// ´test:integration:uncertainty-expressions´
#[test]
fn uncertainty_expressions() -> TestResult {
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    let radius = uncertainty(UncertaintyMode::Radius, &length, None, None)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "x", &length, false)?;
    input(&mut context, "reference", &length, true)?;
    uncertainty_input(
        &mut context,
        "sigma",
        &standard,
        "x",
        UncertaintyMode::Standard,
    )?;
    input(&mut context, "radius", &radius, false)?;
    context.declare(
        Symbol::new("variance")?,
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: variance.clone(),
                expression: expression("sigma^2")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::Variance,
                value: Symbol::new("x")?,
                other: None,
                reference: None,
            },
        },
    )?;
    context.declare(
        Symbol::new("relative")?,
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: relative.clone(),
                expression: expression("sigma/reference")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::RelativeStandard,
                value: Symbol::new("x")?,
                other: None,
                reference: Some(Symbol::new("reference")?),
            },
        },
    )?;
    context.check_against(&expression("variance")?, &variance)?;
    context.check_against(&expression("sqrt(variance)")?, &standard)?;
    context.check_against(&expression("relative")?, &relative)?;
    for source in ["sigma/x", "sigma/(-reference)"] {
        refusal(&context, source, &UnitError::RelativeReference.into())?;
    }
    for source in ["sigma+radius", "sigma+x"] {
        refusal(&context, source, &UnitError::KindMismatch.into())?;
    }
    refusal(&context, "radius^2", &UnitError::UnsupportedKindRule.into())?;
    context.declare(
        Symbol::new("bad_sigma")?,
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: length,
                expression: expression("x")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::Standard,
                value: Symbol::new("x")?,
                other: None,
                reference: None,
            },
        },
    )?;
    refusal(&context, "bad_sigma", &UnitError::KindMismatch.into())?;
    Ok(())
}

/// Unknown dependencies, unused cycles and exhausted budgets produce typed refusals.
///
/// ´test:integration:declaration-refusals´
#[test]
fn declaration_refusals() -> TestResult {
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    for (name, body) in [("a", "b+1"), ("b", "a-1")] {
        context.declare(
            Symbol::new(name)?,
            Declaration::Derived(Observable {
                descriptor: Descriptor::ratio(),
                expression: expression(body)?,
                evidence: Evidence::default(),
            }),
        )?;
    }
    context.check(&expression("1")?)?;
    let failure = context.check_all().err().ok_or("missing cycle failure")?;
    assert!(matches!(failure.failure, Failure::Cycle(_)));
    assert_eq!(
        failure.location.definitions,
        vec![Symbol::new("a")?, Symbol::new("b")?]
    );
    let duplicate = context
        .declare(Symbol::new("a")?, Declaration::Metadata(String::new()))
        .err()
        .ok_or("expected duplicate declaration refusal")?;
    assert_eq!(duplicate.failure, Failure::Duplicate(Symbol::new("a")?));
    let unknown = context
        .check(&expression("missing+1")?)
        .err()
        .ok_or("missing unknown-symbol failure")?;
    assert!(matches!(unknown.failure, Failure::UnknownSymbol(_)));
    let tiny = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            work: 1,
            ..Limits::default()
        },
    );
    refusal(
        &tiny,
        "1+2",
        &Failure::Algebra(Error::Limit("units traversal")),
    )?;
    Ok(())
}
