// SPDX-License-Identifier: AGPL-3.0-only
//! Original-operation checks and declared observable contracts.

use nrz_cas::{
    Fault, FaultKind, InvalidCode, Rational,
    limits::{BudgetKind, Limits, LimitsOverride},
    request::Outcome,
};
#[path = "support/arithmetic.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod support;
use support::rational;
#[path = "support/engine.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod engine;
use engine::{fixture, original, prove};

use nrz_units::{
    BaseDimension, BinaryOp, Companion, Context, CountDomain, Declaration, Descriptor, Dimension,
    Evidence, Expectation, ExponentPolicy, Failure, FunctionSignature, Kind, Measure, Observable,
    Registry, Role, Scale, UncertaintyMode, UnitError, uncertainty,
};

/// Shared fallible fixture result.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A physical kind with a coherent linear measure.
fn named(name: &str, dimension: Dimension) -> Result<Descriptor, Box<dyn std::error::Error>> {
    Ok(Descriptor::new(
        Kind::Named(name.to_owned(), Box::new(dimension)),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Register a source input with a visible provenance and assurance claim.
fn input(context: &mut Context, name: &str, descriptor: &Descriptor, nonzero: bool) -> TestResult {
    context.declare(
        name.to_owned(),
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
        name.to_owned(),
        Declaration::UncertaintyInput {
            descriptor: descriptor.clone(),
            companion: Companion {
                mode,
                value: value.to_owned(),
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
    let diagnostic = original(context, source)
        .and_then(|original| context.check(&original))
        .err()
        .ok_or("expected refusal")?;
    assert_eq!(&diagnostic.failure, expected, "{source}");
    Ok(())
}

/// Original invalid operations cannot disappear through cancellation or multiplication by zero.
///
/// TODO ´todo:test:test-typed-matrix-zeros-and-cancelled´: test typed matrix zeros and cancelled contractions retain original axis checks under rule:nrz:shape-entry-contract and rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:test:test-invalid-original-operations-inside-unselected´: test invalid original operations inside unselected plan alternatives remain refusals under rule:nrz:plan-validation.
///
/// ´test:integration:original-operations-before-normalization´
#[test]
fn original_operations_before_normalization() -> TestResult {
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let time = named("time", Dimension::base(BaseDimension::Time))?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["missing", "t", "x", "zero_length"],
    )?;
    input(&mut context, "x", &length, false)?;
    input(&mut context, "t", &time, false)?;
    for source in ["0*(x+t)", "(x+t)-(x+t)", "(x+t)^0"] {
        refusal(&context, source, &UnitError::DimensionMismatch.into())?;
    }
    refusal(
        &context,
        "0*missing",
        &Failure::UnknownSymbol("missing".to_owned()),
    )?;
    let failure = context
        .check(&original(&context, "0*(x+t)")?)
        .err()
        .ok_or("missing physical diagnostic")?;
    assert_eq!(failure.location.path, vec![1]);
    assert!(failure.location.span.is_some());
    assert_eq!(failure.derived.len(), 2);
    assert!(matches!(
        failure.failure,
        Failure::Units(UnitError::DimensionMismatch)
    ));
    let zero = context.check(&original(&context, "x-x")?)?;
    assert_eq!(zero.descriptor().kind(), length.kind());
    prove(&context, &zero, "0", None)?;
    let error = context
        .check_against(&original(&context, "0")?, &length)
        .err()
        .ok_or("expected physical zero refusal")?;
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    let source = original(&context, "x/x")?;
    let quotient = context.check(&source)?;
    assert_eq!(quotient.original().root, source.root);
    assert_eq!(quotient.original().syntax, source.syntax);
    prove(&context, &quotient, "1", Some("x"))?;
    assert_eq!(quotient.nodes().len(), 3);
    assert!(quotient.declarations().contains_key("x"));
    let undefined = context.check(&original(&context, "0*(1/0)")?)?;
    assert!(matches!(
        context.prove_equal(
            &undefined,
            &original(&context, "0")?,
            LimitsOverride::default()
        )?,
        Outcome::Refused {
            fault: Fault {
                kind: FaultKind::Invalid(InvalidCode::RefutedDomain),
                ..
            },
            ..
        }
    ));
    context.declare(
        "zero_length".to_owned(),
        Declaration::Constant {
            descriptor: length.clone(),
            value: Rational::zero(),
            evidence: Evidence::default(),
        },
    )?;
    context.check_against(&original(&context, "zero_length")?, &length)?;
    Ok(())
}

/// Coordinate conversions retain exact affine and multiplicative evidence.
///
/// TODO ´todo:test:test-operator-normalization-and-paired-covariance´: test operator normalization and paired covariance conversions preserve coordinate meaning under def:nrz:shape-normalization and def:nrz:covariance-shape.
///
/// TODO ´todo:test:test-changed-declaration-snapshots-cannot-reuse´: test changed declaration snapshots cannot reuse old binding conversion checks under rule:nrz:binding-validation.
///
/// ´test:integration:coordinate-evidence´
#[test]
fn coordinate_evidence() -> TestResult {
    let temperature = Kind::Named(
        "temperature".to_owned(),
        Box::new(Dimension::base(BaseDimension::Temperature)),
    );
    let point = Descriptor::new(
        temperature,
        Measure::new(
            "Celsius",
            Scale::Exact(Rational::one()),
            rational(27315, 100)?,
        )?,
        Role::Point,
    )?;
    let length = named("length", Dimension::base(BaseDimension::Length))?;
    let millimetres = Descriptor::new(
        length.kind().clone(),
        Measure::new("mm", Scale::Exact(rational(1, 1000)?), Rational::zero())?,
        Role::Difference,
    )?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["a", "b", "dx", "mm"],
    )?;
    input(&mut context, "a", &point, false)?;
    input(&mut context, "b", &point, false)?;
    input(&mut context, "dx", &point.difference(), false)?;
    input(&mut context, "mm", &millimetres, false)?;
    let difference = context.check(&original(&context, "a-b")?)?;
    assert_eq!(difference.descriptor().role(), Role::Difference);
    let celsius_offset = rational(27315, 100)?;
    assert!(
        difference
            .nodes()
            .iter()
            .any(|node| node.conversion.shift == celsius_offset)
    );
    context.check(&original(&context, "a+dx")?)?;
    context.check(&original(&context, "dx+a")?)?;
    prove(&context, &difference, "a-b", None)?;
    context.check(&original(&context, "a+(b-a)/2")?)?;
    for source in ["a+b", "dx-a", "0*a", "a/a", "a^0", "-a", "0.5*a+0.5*b"] {
        refusal(&context, source, &UnitError::AffineRole.into())?;
    }
    let checked = context.check_against(&original(&context, "mm")?, &length)?;
    let millimetre_scale = rational(1, 1000)?;
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
        Rational::one()
    );
    Ok(())
}

/// Derived eigenvalues and homogeneous blocks check declared physical relations.
///
/// TODO ´todo:test:test-rectangular-block-extents-and-ordered´: test rectangular block extents and ordered equation axes alongside scalar descriptor checks under def:nrz:matrix-shape and rule:nrz:shape-entry-contract.
///
/// TODO ´todo:test:test-parameterized-block-bindings-discharge-concrete´: test parameterized block bindings discharge concrete shape constraints before evaluation under rule:nrz:binding-validation.
///
/// ´test:integration:observables-equations-and-blocks´
#[test]
fn observables_equations_and_blocks() -> TestResult {
    let mass = named("mass", Dimension::base(BaseDimension::Mass))?;
    let stiffness = named("stiffness", Dimension::integers([0, 1, -2, 0, 0, 0, 0])?)?;
    let eigenvalue = named("eigenvalue", Dimension::integers([0, 0, -2, 0, 0, 0, 0])?)?;
    let omega = named(
        "angular_frequency",
        Dimension::integers([0, 0, -1, 0, 0, 0, 0])?,
    )?;
    let mode_shape = named("mode_shape", Dimension::zero())?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Divide,
        stiffness.kind().clone(),
        mass.kind().clone(),
        eigenvalue.kind().clone(),
    )?;
    registry.register_power(
        eigenvalue.kind().clone(),
        rational(1, 2)?,
        omega.kind().clone(),
    )?;
    let mut context = fixture(registry, Limits::default(), &["K", "M", "lambda", "shape"])?;
    input(&mut context, "K", &stiffness, false)?;
    input(&mut context, "M", &mass, true)?;
    input(&mut context, "shape", &mode_shape, false)?;
    context.declare(
        "lambda".to_owned(),
        Declaration::Derived(Observable {
            descriptor: eigenvalue,
            expression: original(&context, "K/M")?,
            evidence: Evidence::default(),
        }),
    )?;
    context.check_against(&original(&context, "sqrt(lambda)")?, &omega)?;
    context.check_equation(&original(&context, "lambda")?, &original(&context, "K/M")?)?;
    let error = context
        .check_against(&original(&context, "shape")?, &omega)
        .err()
        .ok_or("expected shape mismatch")?;
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    assert_eq!(
        context
            .check_block(
                &[original(&context, "K")?, original(&context, "2*K")?],
                &stiffness
            )?
            .len(),
        2
    );
    let failure = context
        .check_block(
            &[original(&context, "K")?, original(&context, "M")?],
            &stiffness,
        )
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
/// TODO ´todo:test:test-changed-function-declarations-require-fresh´: test changed function declarations require fresh reusable plan checks under rule:nrz:plan-validation.
///
/// ´test:integration:function-signatures´
#[test]
fn function_signatures() -> TestResult {
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let shape = named("mode_shape", Dimension::zero())?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["project", "shape", "theta", "unknown"],
    )?;
    input(&mut context, "theta", &angle, false)?;
    input(&mut context, "shape", &shape, false)?;
    for source in [
        "sin(theta)",
        "cos(theta)",
        "tan(theta)",
        "sin(asin(0.5))",
        "exp(log(2))",
    ] {
        context.check_against(&original(&context, source)?, &Descriptor::ratio())?;
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
        &Failure::UnknownFunction("unknown".to_owned()),
    )?;
    let diagnostic = context
        .check(&original(&context, "sin(shape)")?)
        .err()
        .ok_or("missing function diagnostic")?;
    assert_eq!(diagnostic.location.path, vec![0]);
    assert_eq!(
        diagnostic.location.span,
        Some(nrz_cas::syntax::Span { start: 4, end: 9 })
    );
    assert_eq!(
        diagnostic.expected,
        Expectation::Descriptor(Box::new(angle.clone()))
    );
    assert_eq!(diagnostic.derived, vec![shape.clone()]);
    context.declare_function(
        "project".to_owned(),
        FunctionSignature {
            parameters: vec![shape.clone(), angle.clone()],
            result: Descriptor::ratio(),
        },
    )?;
    context.check(&original(&context, "project(shape,theta)")?)?;
    refusal(
        &context,
        "project(theta,shape)",
        &UnitError::KindMismatch.into(),
    )?;
    let error = context
        .declare_function(
            "sin".to_owned(),
            FunctionSignature {
                parameters: vec![angle],
                result: shape,
            },
        )
        .err()
        .ok_or("expected reserved function refusal")?;
    assert_eq!(error.failure, Failure::Duplicate("sin".to_owned()));
    Ok(())
}

/// Count imports and expressions stay discrete while their uncertainty can be continuous.
///
/// ´test:integration:counts-and-metadata´
#[test]
fn counts_and_metadata() -> TestResult {
    let count = Descriptor::new(
        Kind::Count("events".to_owned(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let sigma = uncertainty(UncertaintyMode::Standard, &count, None, None)?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["fractional_count", "label", "n", "r", "sigma_n"],
    )?;
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
        "label".to_owned(),
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
        context.check(&original(&context, source)?)?;
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
    refusal(&context, "0*label", &Failure::Metadata("label".to_owned()))?;
    let error = context
        .declare(
            "fractional_count".to_owned(),
            Declaration::Constant {
                descriptor: count,
                value: rational(1, 2)?,
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
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &[
            "bad_sigma",
            "radius",
            "reference",
            "relative",
            "sigma",
            "variance",
            "x",
        ],
    )?;
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
        "variance".to_owned(),
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: variance.clone(),
                expression: original(&context, "sigma^2")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::Variance,
                value: "x".to_owned(),
                other: None,
                reference: None,
            },
        },
    )?;
    context.declare(
        "relative".to_owned(),
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: relative.clone(),
                expression: original(&context, "sigma/reference")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::RelativeStandard,
                value: "x".to_owned(),
                other: None,
                reference: Some("reference".to_owned()),
            },
        },
    )?;
    context.check_against(&original(&context, "variance")?, &variance)?;
    context.check_against(&original(&context, "sqrt(variance)")?, &standard)?;
    context.check_against(&original(&context, "relative")?, &relative)?;
    for source in ["sigma/x", "sigma/(-reference)"] {
        refusal(&context, source, &UnitError::RelativeReference.into())?;
    }
    for source in ["sigma+radius", "sigma+x"] {
        refusal(&context, source, &UnitError::KindMismatch.into())?;
    }
    refusal(&context, "radius^2", &UnitError::UnsupportedKindRule.into())?;
    context.declare(
        "bad_sigma".to_owned(),
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: length,
                expression: original(&context, "x")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::Standard,
                value: "x".to_owned(),
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
/// TODO ´todo:test:test-undeclared-binding-inputs-fail-before´: test undeclared binding inputs fail before any numerical operation executes under rule:nrz:binding-validation.
///
/// ´test:integration:declaration-refusals´
#[test]
fn declaration_refusals() -> TestResult {
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["a", "b", "missing"],
    )?;
    for (name, body) in [("a", "b+1"), ("b", "a-1")] {
        context.declare(
            name.to_owned(),
            Declaration::Derived(Observable {
                descriptor: Descriptor::ratio(),
                expression: original(&context, body)?,
                evidence: Evidence::default(),
            }),
        )?;
    }
    context.check(&original(&context, "1")?)?;
    let failure = context.check_all().err().ok_or("missing cycle failure")?;
    assert!(matches!(failure.failure, Failure::Cycle(_)));
    assert_eq!(
        failure.location.definitions,
        vec!["a".to_owned(), "b".to_owned()]
    );
    let duplicate = context
        .declare("a".to_owned(), Declaration::Metadata(String::new()))
        .err()
        .ok_or("expected duplicate declaration refusal")?;
    assert_eq!(duplicate.failure, Failure::Duplicate("a".to_owned()));
    let unknown = context
        .check(&original(&context, "missing+1")?)
        .err()
        .ok_or("missing unknown-symbol failure")?;
    assert!(matches!(unknown.failure, Failure::UnknownSymbol(_)));
    let tiny = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            work: 1,
            ..Limits::default()
        },
        &[],
    )?;
    let failure = tiny
        .check(&original(&tiny, "1+2")?)
        .err()
        .ok_or("missing work refusal")?;
    assert!(matches!(
        failure.failure,
        Failure::Algebra(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                limit: 1,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Reproduce the coupled-oscillator descriptor rules without numerical root evaluation.
fn nested_root_context(
    limits: Limits,
    source_padding: usize,
) -> Result<(Context, Descriptor), Box<dyn std::error::Error>> {
    let mass = named("mass", Dimension::base(BaseDimension::Mass))?;
    let stiffness = named("stiffness", Dimension::integers([0, 1, -2, 0, 0, 0, 0])?)?;
    let squared = named(
        "frequency_squared",
        Dimension::integers([0, 0, -2, 0, 0, 0, 0])?,
    )?;
    let angular = named(
        "angular_frequency",
        Dimension::integers([0, 0, -1, 0, 0, 0, 0])?,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Divide,
        stiffness.kind().clone(),
        mass.kind().clone(),
        squared.kind().clone(),
    )?;
    registry.register_power(
        squared.kind().clone(),
        rational(1, 2)?,
        angular.kind().clone(),
    )?;
    let mut context = fixture(registry, limits, &["k", "m", "omega1"])?;
    if source_padding > 0 {
        context.parse(
            "coordinate-budget-padding",
            &format!("{}k", " ".repeat(source_padding)),
        )?;
    }
    input(&mut context, "k", &stiffness, false)?;
    input(&mut context, "m", &mass, true)?;
    context.declare(
        "omega1".to_owned(),
        Declaration::Derived(Observable {
            descriptor: angular.clone(),
            expression: original(&context, "sqrt((2 - sqrt(2)) * k / m)")?,
            evidence: Evidence::default(),
        }),
    )?;
    Ok((context, angular))
}

/// A nested irrational root remains physically checked even in an accumulated CAS store.
///
/// ´test:integration:nested-root-coordinate-batch´
#[test]
fn nested_root_coordinate_batch() -> TestResult {
    let (context, expected) = nested_root_context(Limits::default(), 100_000)?;
    let payload = context.cas()?.counts().payload_bytes;
    // The former three whole-store walks plus one admission charged at least seven payloads.
    assert!(payload > Limits::default().work / 7);
    let expression = original(&context, "omega1")?;
    let checked = context.check_against(&expression, &expected)?;
    assert_eq!(checked.descriptor(), &expected);
    assert_eq!(checked.original().root, expression.root);
    assert!(checked.declarations().contains_key("k"));
    assert!(checked.declarations().contains_key("m"));
    assert!(checked.nodes().iter().any(|node| {
        node.location.path == [0]
            && node.location.definitions == ["omega1"]
            && node.descriptor == expected
    }));
    Ok(())
}

/// Coordinate expansion adds exact half exponents and retains the original refusal location.
///
/// ´test:integration:nested-root-coordinate-budget´
#[test]
fn nested_root_coordinate_budget() -> TestResult {
    let (context, _) = nested_root_context(
        Limits {
            peak_expression_size: 9,
            ..Limits::default()
        },
        0,
    )?;
    let diagnostic = context
        .check(&original(&context, "omega1")?)
        .err()
        .ok_or("expected coordinate size refusal")?;
    assert!(matches!(
        diagnostic.failure,
        Failure::Coordinates(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::PeakExpressionSize,
                limit: 9,
                needed: 11,
                ..
            },
            ..
        })
    ));
    assert_eq!(
        diagnostic.expected,
        Expectation::Rule("bounded exact coordinate expression")
    );
    assert_eq!(diagnostic.location.path, vec![0]);
    assert_eq!(diagnostic.location.definitions, vec!["omega1"]);
    assert_eq!(
        diagnostic.location.span,
        Some(nrz_cas::syntax::Span { start: 0, end: 27 })
    );
    assert_eq!(diagnostic.derived, []);
    Ok(())
}

/// Reproduce the damped oscillator's two-level derived stiffness calculation.
fn derived_chain_rules() -> Result<(Registry, [Descriptor; 5]), Box<dyn std::error::Error>> {
    let mass = named("mass", Dimension::base(BaseDimension::Mass))?;
    let stiffness = named("stiffness", Dimension::integers([0, 1, -2, 0, 0, 0, 0])?)?;
    let damping = named("damping", Dimension::integers([0, 1, -1, 0, 0, 0, 0])?)?;
    let angular = named(
        "angular_frequency",
        Dimension::integers([0, 0, -1, 0, 0, 0, 0])?,
    )?;
    let frequency = named(
        "frequency_squared",
        Dimension::integers([0, 0, -2, 0, 0, 0, 0])?,
    )?;
    let squared = named(
        "stiffness_squared",
        Dimension::integers([0, 2, -4, 0, 0, 0, 0])?,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    for (left, right, output) in [
        (&mass, &frequency, &stiffness),
        (&damping, &angular, &stiffness),
    ] {
        registry.register_binary(
            BinaryOp::Multiply,
            left.kind().clone(),
            right.kind().clone(),
            output.kind().clone(),
        )?;
    }
    for (base, output) in [(&angular, &frequency), (&stiffness, &squared)] {
        registry.register_power(base.kind().clone(), rational(2, 1)?, output.kind().clone())?;
    }
    Ok((registry, [mass, stiffness, damping, angular, squared]))
}

/// Multiple derived bodies share a finite check ledger without paying for unrelated sources.
///
/// ´test:integration:derived-chain-in-large-store´
#[test]
fn derived_chain_in_large_store() -> TestResult {
    let (registry, [mass, stiffness, damping, angular, squared]) = derived_chain_rules()?;
    let mut context = fixture(
        registry,
        Limits::default(),
        &["c", "d", "k", "m", "omega", "q"],
    )?;
    context.parse("chain-padding", &format!("{}k", " ".repeat(100_000)))?;
    for (name, descriptor) in [
        ("c", &damping),
        ("k", &stiffness),
        ("m", &mass),
        ("omega", &angular),
    ] {
        input(&mut context, name, descriptor, false)?;
    }
    for (name, text, descriptor) in [
        ("d", "k-m*omega^2", &stiffness),
        ("q", "d^2+(c*omega)^2", &squared),
    ] {
        context.declare(
            name.to_owned(),
            Declaration::Derived(Observable {
                descriptor: descriptor.clone(),
                expression: original(&context, text)?,
                evidence: Evidence::default(),
            }),
        )?;
    }
    assert!(context.cas()?.counts().payload_bytes > Limits::default().work / 7);
    let checked = context.check_against(&original(&context, "q")?, &squared)?;
    assert_eq!(checked.descriptor(), &squared);
    assert_eq!(
        checked
            .declarations()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["c", "d", "k", "m", "omega", "q"]
    );
    assert!(checked.nodes().iter().any(|node| {
        node.location.definitions == ["q", "d"]
            && node.location.span == Some(nrz_cas::syntax::Span { start: 0, end: 11 })
            && node.descriptor == stiffness
    }));
    Ok(())
}
