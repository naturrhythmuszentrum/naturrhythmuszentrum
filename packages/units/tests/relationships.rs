// SPDX-License-Identifier: AGPL-3.0-only
//! Value identity, reference binding and distinct diagnostic boundaries.

use nrz_algebra::{Error, Expr, Limits, Rational, Span, Symbol, parse};
use nrz_units::{
    BaseDimension, BinaryOp, Companion, Context, CountDomain, Declaration, Descriptor, Dimension,
    Evidence, ExponentPolicy, Failure, Kind, Measure, Observable, Registry, Role, Scale,
    UncertaintyMode, UnitError, uncertainty,
};

/// Fallible fixtures report their setup errors directly.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse an original source tree independently of the checking context's limits.
fn expression(source: &str) -> Result<Expr, nrz_algebra::ParseError> {
    parse(source, Limits::default())
}

/// Declare an ordinary numerical input.
fn input(context: &mut Context, name: &str, value: &Descriptor, nonzero: bool) -> TestResult {
    context.declare(
        Symbol::new(name)?,
        Declaration::Input {
            descriptor: value.clone(),
            nonzero,
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Declare a relationship-bearing uncertainty input.
fn bound(
    context: &mut Context,
    name: &str,
    descriptor: &Descriptor,
    value: &str,
    mode: UncertaintyMode,
    reference: Option<&str>,
) -> TestResult {
    context.declare(
        Symbol::new(name)?,
        Declaration::UncertaintyInput {
            descriptor: descriptor.clone(),
            companion: Companion {
                mode,
                value: Symbol::new(value)?,
                other: None,
                reference: reference.map(Symbol::new).transpose()?,
            },
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Construct the named length kind used by independent identity tests.
fn length() -> Result<Descriptor, Box<dyn std::error::Error>> {
    Ok(Descriptor::new(
        Kind::Named(
            Symbol::new("length")?,
            Dimension::base(BaseDimension::Length),
        ),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Ratio coefficients scale absolute, variance and relative forms without changing modes.
///
/// TODO ´todo:test:test-per-entry-reference-scales-survive´: test per entry reference scales survive views and paired covariance conversion under rule:nrz:correlation-and-relative-shapes.
///
/// ´test:integration:ratio-uncertainty-scaling-and-references´
#[test]
fn ratio_uncertainty_scaling_and_references() -> TestResult {
    let ratio = Descriptor::ratio();
    let standard = uncertainty(UncertaintyMode::Standard, &ratio, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &ratio, None, None)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &ratio,
        None,
        Some((&ratio, true)),
    )?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "eff", &ratio, true)?;
    input(&mut context, "r", &ratio, false)?;
    bound(
        &mut context,
        "sigma",
        &standard,
        "eff",
        UncertaintyMode::Standard,
        None,
    )?;
    bound(
        &mut context,
        "var",
        &variance,
        "eff",
        UncertaintyMode::Variance,
        None,
    )?;
    bound(
        &mut context,
        "rel",
        &relative,
        "eff",
        UncertaintyMode::RelativeStandard,
        Some("eff"),
    )?;
    for (source, expected) in [
        ("sigma/2", &standard),
        ("0.5*sigma", &standard),
        ("sigma/r", &standard),
        ("var/2", &variance),
        ("rel/2", &relative),
        ("sigma/eff", &relative),
    ] {
        context.check_against(&expression(source)?, expected)?;
    }
    assert_eq!(
        context
            .check(&expression("sigma/2")?)?
            .normal_form(Limits::default())?,
        context
            .check(&expression("0.5*sigma")?)?
            .normal_form(Limits::default())?
    );
    let checked = context.check(&expression("sigma/eff")?)?;
    let relation = checked.companion().ok_or("missing relative identity")?;
    assert_eq!(relation.value, Symbol::new("eff")?);
    assert_eq!(relation.reference, Some(Symbol::new("eff")?));
    Ok(())
}

/// Variance and relative companions must describe the values and divisors actually used.
///
/// TODO ´todo:test:test-equal-shape-values-retain-distinct´: test equal shape values retain distinct identities and shared source relationships under def:nrz:companion-shape and dec:nrz:shape-descriptor-ownership.
///
/// ´test:integration:uncertainty-value-and-reference-identities´
#[test]
fn uncertainty_value_and_reference_identities() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    for name in ["x", "y", "r", "other"] {
        input(&mut context, name, &length, true)?;
    }
    bound(
        &mut context,
        "u_x",
        &standard,
        "x",
        UncertaintyMode::Standard,
        None,
    )?;
    bound(
        &mut context,
        "u_y",
        &standard,
        "y",
        UncertaintyMode::Standard,
        None,
    )?;
    let square = context.check_against(&expression("u_x*u_x")?, &variance)?;
    assert_eq!(
        square.companion().ok_or("missing variance identity")?.value,
        Symbol::new("x")?
    );
    let error = context
        .check(&expression("u_x*u_y")?)
        .err()
        .ok_or("expected refusal")?;
    assert_eq!(
        error.failure,
        Failure::Units(UnitError::UnsupportedKindRule)
    );
    for (name, body, descriptor, mode, reference) in [
        (
            "wrong_value",
            "u_y^2",
            variance,
            UncertaintyMode::Variance,
            None,
        ),
        (
            "wrong_reference",
            "u_x/other",
            relative,
            UncertaintyMode::RelativeStandard,
            Some("r"),
        ),
    ] {
        context.declare(
            Symbol::new(name)?,
            Declaration::Uncertainty {
                observable: Observable {
                    descriptor,
                    expression: expression(body)?,
                    evidence: Evidence::default(),
                },
                companion: Companion {
                    mode,
                    value: Symbol::new("x")?,
                    other: None,
                    reference: reference.map(Symbol::new).transpose()?,
                },
            },
        )?;
        let error = context
            .check(&expression(name)?)
            .err()
            .ok_or("expected refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
    }
    Ok(())
}

/// Relative inputs and constants cannot bypass a named, checked nonzero reference.
///
/// TODO ´todo:test:test-zero-component-references-and-unavailable´: test zero component references and unavailable zero variance correlations refuse implicit defaults under rule:nrz:correlation-and-relative-shapes.
///
/// ´test:integration:relative-declaration-refusals´
#[test]
fn relative_declaration_refusals() -> TestResult {
    let length = length()?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "x", &length, true)?;
    let declarations = [
        Declaration::Input {
            descriptor: relative.clone(),
            nonzero: false,
            evidence: Evidence::default(),
        },
        Declaration::Constant {
            descriptor: relative.clone(),
            value: Rational::ONE,
            evidence: Evidence::default(),
        },
        Declaration::Derived(Observable {
            descriptor: relative.clone(),
            expression: expression("1")?,
            evidence: Evidence::default(),
        }),
    ];
    for declaration in declarations {
        let error = context
            .declare(Symbol::new("unbound")?, declaration)
            .err()
            .ok_or("expected declaration refusal")?;
        assert_eq!(error.failure, Failure::Units(UnitError::RelativeReference));
    }
    bound(
        &mut context,
        "missing_reference",
        &relative,
        "x",
        UncertaintyMode::RelativeStandard,
        None,
    )?;
    let error = context
        .check(&expression("missing_reference")?)
        .err()
        .ok_or("expected reference refusal")?;
    assert_eq!(error.failure, Failure::Units(UnitError::RelativeReference));
    assert_eq!(error.location.span, None);
    Ok(())
}

/// Equation paths identify the failing side independently of nested operand paths.
///
/// TODO ´todo:test:test-block-and-covariance-index-paths´: test block and covariance index paths identify the failing contraction side under rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:equation-side-diagnostics´
#[test]
fn equation_side_diagnostics() -> TestResult {
    let context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    for (left, right, path) in [("missing", "1", vec![0]), ("1", "missing", vec![1])] {
        let error = context
            .check_equation(&expression(left)?, &expression(right)?)
            .err()
            .ok_or("expected equation refusal")?;
        assert_eq!(error.location.path, path);
        assert_eq!(
            error.failure,
            Failure::UnknownSymbol(Symbol::new("missing")?)
        );
    }
    Ok(())
}

/// Expanded coordinates have a distinct resource failure even for valid physical descriptors.
///
/// ´test:integration:coordinate-resource-diagnostics´
#[test]
fn coordinate_resource_diagnostics() -> TestResult {
    let length = length()?;
    let millimetres = Descriptor::new(
        length.kind().clone(),
        Measure::new("mm", Scale::Exact(Rational::new(1, 1000)?), Rational::ZERO)?,
        Role::Difference,
    )?;
    let mut context = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            nodes: 1,
            ..Limits::default()
        },
    );
    input(&mut context, "metres", &length, false)?;
    input(&mut context, "mm", &millimetres, false)?;
    context.check(&expression("metres")?)?;
    let error = context
        .check(&expression("mm")?)
        .err()
        .ok_or("expected coordinate limit")?;
    assert_eq!(error.failure, Failure::Coordinates(Error::Limit("nodes")));
    let mut expanded = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            nodes: 7,
            ..Limits::default()
        },
    );
    input(&mut expanded, "r", &Descriptor::ratio(), false)?;
    expanded.declare(
        Symbol::new("d")?,
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression: expression("r+r")?,
            evidence: Evidence::default(),
        }),
    )?;
    let error = expanded
        .check(&expression("d+d+d")?)
        .err()
        .ok_or("expected expansion limit")?;
    assert_eq!(error.failure, Failure::Coordinates(Error::Limit("nodes")));
    Ok(())
}

/// Registered count composition takes precedence over the conservative literal fallback.
///
/// ´test:integration:registered-count-composition´
#[test]
fn registered_count_composition() -> TestResult {
    let count = Descriptor::new(
        Kind::Count(Symbol::new("events")?, CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mean = Descriptor::new(
        Kind::Named(Symbol::new("mean")?, Dimension::ZERO),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        count.kind().clone(),
        Kind::Ratio,
        mean.kind().clone(),
    )?;
    let mut context = Context::new(registry, Limits::default());
    input(&mut context, "n", &count, false)?;
    input(&mut context, "r", &Descriptor::ratio(), false)?;
    context.check_against(&expression("n*r")?, &mean)?;
    context.check_against(&expression("n*2")?, &mean)?;
    Ok(())
}

/// Declaration diagnostics use the inner source and include value/reference descriptors.
///
/// TODO ´todo:test:test-shaped-declaration-snapshots-preserve-provenance´: test shaped declaration snapshots preserve provenance without asserting statistical validity under rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:declaration-source-and-descriptor-evidence´
#[test]
fn declaration_source_and_descriptor_evidence() -> TestResult {
    let length = length()?;
    let mut context = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default());
    input(&mut context, "x", &length, false)?;
    context.declare(
        Symbol::new("bad")?,
        Declaration::Derived(Observable {
            descriptor: length.clone(),
            expression: expression("1")?,
            evidence: Evidence::default(),
        }),
    )?;
    let error = context
        .check(&expression("        bad")?)
        .err()
        .ok_or("expected mismatch")?;
    assert_eq!(error.location.span, Some(Span::new(0, 1)?));
    assert_eq!(error.location.path, vec![0]);
    context.declare(
        Symbol::new("good")?,
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression: expression("1")?,
            evidence: Evidence::default(),
        }),
    )?;
    let checked = context.check(&expression("good")?)?;
    let definitions = vec![Symbol::new("good")?];
    let node = checked
        .nodes()
        .iter()
        .find(|node| node.location.definitions == definitions)
        .ok_or("missing observable body evidence")?;
    assert_eq!(node.location.path, error.location.path);
    assert_eq!(node.location.span, error.location.span);
    assert_eq!(error.location.definitions, vec![Symbol::new("bad")?]);
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    let time = Descriptor::new(
        Kind::Named(Symbol::new("time")?, Dimension::base(BaseDimension::Time)),
        Measure::coherent(),
        Role::Difference,
    )?;
    input(&mut context, "r", &time, true)?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    context.declare(
        Symbol::new("bad_companion")?,
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: relative.clone(),
                expression: expression("u/r")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::RelativeStandard,
                value: Symbol::new("x")?,
                other: None,
                reference: Some(Symbol::new("r")?),
            },
        },
    )?;
    let error = context
        .check(&expression("    bad_companion")?)
        .err()
        .ok_or("expected companion mismatch")?;
    assert_eq!(error.location.span, Some(Span::new(0, 3)?));
    assert_eq!(error.location.path, vec![0]);
    assert_eq!(
        error.location.definitions,
        vec![Symbol::new("bad_companion")?]
    );
    assert_eq!(error.failure, Failure::Units(UnitError::RelativeReference));
    assert_eq!(error.derived, vec![relative, length, time]);
    Ok(())
}

/// Build a derived value with a transitive dependency and one unrelated numerical input.
fn propagation_context(registry: Registry) -> Result<Context, Box<dyn std::error::Error>> {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = Context::new(registry, Limits::default());
    for name in ["x", "y", "outside", "reference"] {
        input(&mut context, name, &length, true)?;
    }
    for name in ["rho", "eff", "r"] {
        input(&mut context, name, &Descriptor::ratio(), false)?;
    }
    for name in ["k1", "k2"] {
        context.declare(
            Symbol::new(name)?,
            Declaration::Constant {
                descriptor: Descriptor::ratio(),
                value: Rational::integer(2),
                evidence: Evidence::default(),
            },
        )?;
    }
    for (name, value) in [("u_x", "x"), ("u_y", "y"), ("u_out", "outside")] {
        bound(
            &mut context,
            name,
            &standard,
            value,
            UncertaintyMode::Standard,
            None,
        )?;
    }
    for (name, body) in [("z", "middle+y"), ("middle", "x")] {
        context.declare(
            Symbol::new(name)?,
            Declaration::Derived(Observable {
                descriptor: length.clone(),
                expression: expression(body)?,
                evidence: Evidence::default(),
            }),
        )?;
    }
    Ok(context)
}

/// Construct an explicit companion relationship for a test declaration.
fn relation(
    mode: UncertaintyMode,
    value: &str,
    other: Option<&str>,
    reference: Option<&str>,
) -> Result<Companion, nrz_algebra::Error> {
    Ok(Companion {
        mode,
        value: Symbol::new(value)?,
        other: other.map(Symbol::new).transpose()?,
        reference: reference.map(Symbol::new).transpose()?,
    })
}

/// Declare an uncertainty body independently of the value's numerical expression.
fn observable_companion(
    context: &mut Context,
    name: &str,
    body: &str,
    descriptor: &Descriptor,
    companion: Companion,
) -> TestResult {
    context.declare(
        Symbol::new(name)?,
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: descriptor.clone(),
                expression: expression(body)?,
                evidence: Evidence::default(),
            },
            companion,
        },
    )?;
    Ok(())
}

/// Derived propagation records contributing companions and can feed relative and variance forms.
///
/// TODO ´todo:test:test-indexed-sensitivity-contractions-retain-source´: test indexed sensitivity contractions retain source closures across multiple outputs under rule:nrz:shape-sensitivity-propagation and rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:derived-uncertainty-propagation´
#[test]
fn derived_uncertainty_propagation() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(Registry::new(ExponentPolicy::Rational))?;
    let target = relation(UncertaintyMode::Standard, "z", None, None)?;
    observable_companion(
        &mut context,
        "u_z",
        "sqrt((k1*u_x)^2+(k2*u_y)^2)",
        &standard,
        target.clone(),
    )?;
    let checked = context.check(&expression("u_z")?)?;
    let expected = nrz_units::CompanionEvidence::Propagated {
        companion: target,
        contributors: std::collections::BTreeMap::from([
            (
                Symbol::new("u_x")?,
                relation(UncertaintyMode::Standard, "x", None, None)?,
            ),
            (
                Symbol::new("u_y")?,
                relation(UncertaintyMode::Standard, "y", None, None)?,
            ),
        ]),
    };
    assert_eq!(checked.companion_evidence(), Some(&expected));
    for (name, body, mode, reference) in [
        ("var_z", "u_z^2", UncertaintyMode::Variance, None),
        (
            "rel_z",
            "u_z/reference",
            UncertaintyMode::RelativeStandard,
            Some("reference"),
        ),
    ] {
        let descriptor = uncertainty(mode, &length, None, reference.map(|_| (&length, true)))?;
        let companion = relation(mode, "z", None, reference)?;
        observable_companion(&mut context, name, body, &descriptor, companion.clone())?;
        let checked = context.check_against(&expression(name)?, &descriptor)?;
        assert_eq!(checked.companion(), Some(&companion));
        assert!(matches!(
            checked.companion_evidence(),
            Some(nrz_units::CompanionEvidence::Propagated { .. })
        ));
    }
    Ok(())
}

/// Unrelated and unbound uncertainty operands remain refusals even when multiplied by zero.
///
/// TODO ´todo:test:test-rejected-matrix-propagations-expose-missing´: test rejected matrix propagations expose missing paired contributors and stale target maps under rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:propagation-closure-refusals´
#[test]
fn propagation_closure_refusals() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(Registry::new(ExponentPolicy::Rational))?;
    input(&mut context, "unbound", &standard, false)?;
    for (name, body) in [
        ("outside_body", "sqrt(u_x^2+u_out^2)"),
        ("cancelled_outside", "sqrt(u_x^2+0*u_out^2)"),
        ("unbound_body", "sqrt(u_x^2+unbound^2)"),
    ] {
        observable_companion(
            &mut context,
            name,
            body,
            &standard,
            relation(UncertaintyMode::Standard, "z", None, None)?,
        )?;
        let error = context
            .check(&expression(name)?)
            .err()
            .ok_or("expected contributor refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
        assert_eq!(error.location.path, vec![0]);
    }
    Ok(())
}

/// Registered rules cannot create uncertainty terms without contributing companions.
///
/// TODO ´todo:test:test-registered-covariance-products-cannot-erase´: test registered covariance products cannot erase indexed contributor requirements under rule:nrz:shape-data-time-evidence.
///
/// ´test:integration:registered-uncertainty-terms-require-contributors´
#[test]
fn registered_uncertainty_terms_require_contributors() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    let coefficient = Descriptor::new(
        Kind::Named(Symbol::new("coefficient")?, Dimension::ZERO),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        coefficient.kind().clone(),
        length.kind().clone(),
        standard.kind().clone(),
    )?;
    registry.register_power(
        length.kind().clone(),
        Rational::integer(2),
        variance.kind().clone(),
    )?;
    let mut context = propagation_context(registry)?;
    input(&mut context, "k", &coefficient, false)?;
    for (name, body) in [
        ("binary_term", "sqrt(u_x^2+(k*x)^2)"),
        ("power_term", "sqrt(u_x^2+x^2)"),
    ] {
        observable_companion(
            &mut context,
            name,
            body,
            &standard,
            relation(UncertaintyMode::Standard, "z", None, None)?,
        )?;
        let error = context
            .check(&expression(name)?)
            .err()
            .ok_or("expected untracked uncertainty refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
        assert_eq!(error.location.path, vec![0]);
    }
    let companion = relation(UncertaintyMode::Standard, "z", None, None)?;
    observable_companion(
        &mut context,
        "tracked",
        "sqrt(u_x^2)",
        &standard,
        companion.clone(),
    )?;
    let checked = context.check_against(&expression("tracked")?, &standard)?;
    let expected = nrz_units::CompanionEvidence::Propagated {
        companion,
        contributors: std::collections::BTreeMap::from([(
            Symbol::new("u_x")?,
            relation(UncertaintyMode::Standard, "x", None, None)?,
        )]),
    };
    assert_eq!(checked.companion_evidence(), Some(&expected));
    Ok(())
}

/// Covariance propagation admits contributors from either value and records the declared pair.
///
/// TODO ´todo:test:test-the-mixed-voltage-current-covariance´: test the mixed voltage current covariance and structured duplication maps exactly under ex:nrz:resistor-shape-covariance and rule:nrz:structured-companion-coordinates.
///
/// ´test:integration:covariance-propagation´
#[test]
fn covariance_propagation() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let covariance = uncertainty(UncertaintyMode::Covariance, &length, Some(&length), None)?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        standard.kind().clone(),
        standard.kind().clone(),
        covariance.kind().clone(),
    )?;
    let mut context = propagation_context(registry)?;
    let pair = relation(UncertaintyMode::Covariance, "x", Some("y"), None)?;
    observable_companion(
        &mut context,
        "cov_xy",
        "rho*u_x*u_y",
        &covariance,
        pair.clone(),
    )?;
    let checked = context.check_against(&expression("cov_xy")?, &covariance)?;
    assert_eq!(checked.companion(), Some(&pair));
    let Some(nrz_units::CompanionEvidence::Propagated { contributors, .. }) =
        checked.companion_evidence()
    else {
        return Err("expected propagated covariance evidence".into());
    };
    assert_eq!(
        contributors.keys().cloned().collect::<Vec<_>>(),
        vec![Symbol::new("u_x")?, Symbol::new("u_y")?]
    );
    observable_companion(&mut context, "bad_cov", "rho*u_x*u_out", &covariance, pair)?;
    let error = context
        .check(&expression("bad_cov")?)
        .err()
        .ok_or("expected covariance closure refusal")?;
    assert_eq!(
        error.failure,
        Failure::Units(UnitError::UncertaintyIdentity)
    );
    Ok(())
}

/// Ratio quantities require propagation, while literal and typed constant scaling preserve identity.
///
/// TODO ´todo:test:test-matching-axis-descriptors-cannot-substitute´: test matching axis descriptors cannot substitute for relative target identity under def:nrz:companion-shape and rule:nrz:correlation-and-relative-shapes.
///
/// ´test:integration:ratio-quantity-identity-boundary´
#[test]
fn ratio_quantity_identity_boundary() -> TestResult {
    let length = length()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(Registry::new(ExponentPolicy::Rational))?;
    for (name, body) in [
        ("quantity_product", "u_x*eff"),
        ("quantity_quotient", "u_x/r"),
        ("quantity_expression", "u_x*(y/x)"),
    ] {
        observable_companion(
            &mut context,
            name,
            body,
            &standard,
            relation(UncertaintyMode::Standard, "x", None, None)?,
        )?;
        let error = context
            .check(&expression(name)?)
            .err()
            .ok_or("expected ratio quantity identity refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
    }
    let companion = relation(UncertaintyMode::Standard, "x", None, None)?;
    for source in ["2*u_x", "u_x/2", "k1*u_x", "u_x/k1"] {
        let checked = context.check(&expression(source)?)?;
        assert_eq!(checked.companion(), Some(&companion));
        assert!(matches!(
            checked.companion_evidence(),
            Some(nrz_units::CompanionEvidence::Established(_))
        ));
    }
    observable_companion(
        &mut context,
        "scaled_z",
        "u_x*eff",
        &standard,
        relation(UncertaintyMode::Standard, "z", None, None)?,
    )?;
    let checked = context.check(&expression("scaled_z")?)?;
    assert!(matches!(
        checked.companion_evidence(),
        Some(nrz_units::CompanionEvidence::Propagated { .. })
    ));
    context.check(&expression("(2*u_x)^2")?)?;
    let error = context
        .check(&expression("(2*u_x)*(2*u_x)")?)
        .err()
        .ok_or("expected scaled-origin boundary")?;
    assert_eq!(
        error.failure,
        Failure::Units(UnitError::UnsupportedKindRule)
    );
    Ok(())
}
