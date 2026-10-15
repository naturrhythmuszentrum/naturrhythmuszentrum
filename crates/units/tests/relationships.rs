// SPDX-License-Identifier: AGPL-3.0-only
//! Value identity, reference binding and distinct diagnostic boundaries.

use nrz_cas::{
    Fault, FaultKind, Rational,
    limits::{BudgetKind, Limits},
    syntax::Span,
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
    Evidence, ExponentPolicy, Failure, Kind, Measure, Observable, Registry, Role, Scale,
    UncertaintyMode, UnitError, uncertainty,
};

/// Fallible fixtures report their setup errors directly.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Declare an ordinary numerical input.
fn input(context: &mut Context, name: &str, value: &Descriptor, nonzero: bool) -> TestResult {
    context.declare(
        name.to_owned(),
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
        name.to_owned(),
        Declaration::UncertaintyInput {
            descriptor: descriptor.clone(),
            companion: Companion {
                mode,
                value: value.to_owned(),
                other: None,
                reference: reference.map(str::to_owned),
            },
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Construct the named length kind used by independent identity tests.
fn length_descriptor() -> Result<Descriptor, Box<dyn std::error::Error>> {
    Ok(Descriptor::new(
        Kind::Named(
            "length".to_owned(),
            Box::new(Dimension::base(BaseDimension::Length)),
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
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["eff", "r", "rel", "sigma", "var"],
    )?;
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
        context.check_against(&original(&context, source)?, expected)?;
    }
    let divided = context.check(&original(&context, "sigma/2")?)?;
    let multiplied = context.check(&original(&context, "0.5*sigma")?)?;
    prove(&context, &divided, "0.5*sigma", None)?;
    prove(&context, &multiplied, "0.5*sigma", None)?;
    let checked = context.check(&original(&context, "sigma/eff")?)?;
    let relation = checked.companion().ok_or("missing relative identity")?;
    assert_eq!(relation.value, "eff".to_owned());
    assert_eq!(relation.reference, Some("eff".to_owned()));
    Ok(())
}

/// Variance and relative companions must describe the values and divisors actually used.
///
/// TODO ´todo:test:test-equal-shape-values-retain-distinct´: test equal shape values retain distinct identities and shared source relationships under def:nrz:companion-shape and dec:nrz:shape-descriptor-ownership.
///
/// TODO ´todo:test:test-reused-descriptor-slots-cannot-revive´: test reused descriptor slots cannot revive previous companion evidence under rule:nrz:descriptor-lifetime-and-mutation.
///
/// ´test:integration:uncertainty-value-and-reference-identities´
#[test]
fn uncertainty_value_and_reference_identities() -> TestResult {
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
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
            "other",
            "r",
            "u_x",
            "u_y",
            "wrong_reference",
            "wrong_value",
            "x",
            "y",
        ],
    )?;
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
    let square = context.check_against(&original(&context, "u_x*u_x")?, &variance)?;
    assert_eq!(
        square.companion().ok_or("missing variance identity")?.value,
        "x".to_owned()
    );
    let error = context
        .check(&original(&context, "u_x*u_y")?)
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
            name.to_owned(),
            Declaration::Uncertainty {
                observable: Observable {
                    descriptor,
                    expression: original(&context, body)?,
                    evidence: Evidence::default(),
                },
                companion: Companion {
                    mode,
                    value: "x".to_owned(),
                    other: None,
                    reference: reference.map(str::to_owned),
                },
            },
        )?;
        let error = context
            .check(&original(&context, name)?)
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
    let length = length_descriptor()?;
    let relative = uncertainty(
        UncertaintyMode::RelativeStandard,
        &length,
        None,
        Some((&length, true)),
    )?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["missing_reference", "unbound", "x"],
    )?;
    input(&mut context, "x", &length, true)?;
    let declarations = [
        Declaration::Input {
            descriptor: relative.clone(),
            nonzero: false,
            evidence: Evidence::default(),
        },
        Declaration::Constant {
            descriptor: relative.clone(),
            value: Rational::one(),
            evidence: Evidence::default(),
        },
        Declaration::Derived(Observable {
            descriptor: relative.clone(),
            expression: original(&context, "1")?,
            evidence: Evidence::default(),
        }),
    ];
    for declaration in declarations {
        let error = context
            .declare("unbound".to_owned(), declaration)
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
        .check(&original(&context, "missing_reference")?)
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
    let context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["missing"],
    )?;
    for (left, right, path) in [("missing", "1", vec![0]), ("1", "missing", vec![1])] {
        let error = context
            .check_equation(&original(&context, left)?, &original(&context, right)?)
            .err()
            .ok_or("expected equation refusal")?;
        assert_eq!(error.location.path, path);
        assert_eq!(error.failure, Failure::UnknownSymbol("missing".to_owned()));
    }
    Ok(())
}

/// Expanded coordinates have a distinct resource failure even for valid physical descriptors.
///
/// ´test:integration:coordinate-resource-diagnostics´
#[test]
fn coordinate_resource_diagnostics() -> TestResult {
    let length = length_descriptor()?;
    let millimetres = Descriptor::new(
        length.kind().clone(),
        Measure::new("mm", Scale::Exact(rational(1, 1000)?), Rational::zero())?,
        Role::Difference,
    )?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            peak_expression_size: 1,
            ..Limits::default()
        },
        &["d", "metres", "mm", "r"],
    )?;
    input(&mut context, "metres", &length, false)?;
    input(&mut context, "mm", &millimetres, false)?;
    context.check(&original(&context, "metres")?)?;
    let error = context
        .check(&original(&context, "mm")?)
        .err()
        .ok_or("expected coordinate limit")?;
    assert!(matches!(
        error.failure,
        Failure::Coordinates(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::PeakExpressionSize,
                ..
            },
            ..
        })
    ));
    let mut expanded = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            peak_expression_size: 7,
            ..Limits::default()
        },
        &["d", "metres", "mm", "r"],
    )?;
    input(&mut expanded, "r", &Descriptor::ratio(), false)?;
    expanded.declare(
        "d".to_owned(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression: original(&expanded, "r+r")?,
            evidence: Evidence::default(),
        }),
    )?;
    let error = expanded
        .check(&original(&expanded, "d+d+d")?)
        .err()
        .ok_or("expected expansion limit")?;
    assert!(matches!(
        error.failure,
        Failure::Coordinates(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::PeakExpressionSize,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Registered count composition takes precedence over the conservative literal fallback.
///
/// ´test:integration:registered-count-composition´
#[test]
fn registered_count_composition() -> TestResult {
    let count = Descriptor::new(
        Kind::Count("events".to_owned(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mean = Descriptor::new(
        Kind::Named("mean".to_owned(), Box::new(Dimension::zero())),
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
    let mut context = fixture(registry, Limits::default(), &["n", "r"])?;
    input(&mut context, "n", &count, false)?;
    input(&mut context, "r", &Descriptor::ratio(), false)?;
    context.check_against(&original(&context, "n*r")?, &mean)?;
    context.check_against(&original(&context, "n*2")?, &mean)?;
    Ok(())
}

/// Declaration diagnostics use the inner source and include value/reference descriptors.
///
/// TODO ´todo:test:test-shaped-declaration-snapshots-preserve-provenance´: test shaped declaration snapshots preserve provenance without asserting statistical validity under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:test:test-binding-refusals-retain-point-context´: test binding refusals retain point context alongside original declaration diagnostics under rule:nrz:binding-validation.
///
/// ´test:integration:declaration-source-and-descriptor-evidence´
#[test]
fn declaration_source_and_descriptor_evidence() -> TestResult {
    let length = length_descriptor()?;
    let mut context = fixture(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["bad", "bad_companion", "good", "r", "time", "u", "x"],
    )?;
    input(&mut context, "x", &length, false)?;
    context.declare(
        "bad".to_owned(),
        Declaration::Derived(Observable {
            descriptor: length.clone(),
            expression: original(&context, "1")?,
            evidence: Evidence::default(),
        }),
    )?;
    let error = context
        .check(&original(&context, "        bad")?)
        .err()
        .ok_or("expected mismatch")?;
    assert_eq!(error.location.span, Some(Span { start: 0, end: 1 }));
    assert_eq!(error.location.path, vec![0]);
    context.declare(
        "good".to_owned(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression: original(&context, "1")?,
            evidence: Evidence::default(),
        }),
    )?;
    let checked = context.check(&original(&context, "good")?)?;
    let definitions = vec!["good".to_owned()];
    let node = checked
        .nodes()
        .iter()
        .find(|node| node.location.definitions == definitions)
        .ok_or("missing observable body evidence")?;
    assert_eq!(node.location.path, error.location.path);
    assert_eq!(node.location.span, error.location.span);
    assert_eq!(error.location.definitions, vec!["bad".to_owned()]);
    assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
    let time = Descriptor::new(
        Kind::Named(
            "time".to_owned(),
            Box::new(Dimension::base(BaseDimension::Time)),
        ),
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
        "bad_companion".to_owned(),
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: relative.clone(),
                expression: original(&context, "u/r")?,
                evidence: Evidence::default(),
            },
            companion: Companion {
                mode: UncertaintyMode::RelativeStandard,
                value: "x".to_owned(),
                other: None,
                reference: Some("r".to_owned()),
            },
        },
    )?;
    let error = context
        .check(&original(&context, "    bad_companion")?)
        .err()
        .ok_or("expected companion mismatch")?;
    assert_eq!(error.location.span, Some(Span { start: 0, end: 3 }));
    assert_eq!(error.location.path, vec![0]);
    assert_eq!(error.location.definitions, vec!["bad_companion".to_owned()]);
    assert_eq!(error.failure, Failure::Units(UnitError::RelativeReference));
    assert_eq!(error.derived, vec![relative, length, time]);
    Ok(())
}

/// Build a derived value with a transitive dependency and one unrelated numerical input.
fn propagation_context(
    registry: Registry,
    keys: &[&str],
) -> Result<Context, Box<dyn std::error::Error>> {
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut names = vec![
        "eff",
        "k1",
        "k2",
        "middle",
        "outside",
        "r",
        "reference",
        "rho",
        "u_out",
        "u_x",
        "u_y",
        "x",
        "y",
        "z",
    ];
    names.extend_from_slice(keys);
    let mut context = fixture(registry, Limits::default(), &names)?;
    for name in ["x", "y", "outside", "reference"] {
        input(&mut context, name, &length, true)?;
    }
    for name in ["rho", "eff", "r"] {
        input(&mut context, name, &Descriptor::ratio(), false)?;
    }
    for name in ["k1", "k2"] {
        context.declare(
            name.to_owned(),
            Declaration::Constant {
                descriptor: Descriptor::ratio(),
                value: rational(2, 1)?,
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
            name.to_owned(),
            Declaration::Derived(Observable {
                descriptor: length.clone(),
                expression: original(&context, body)?,
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
) -> Companion {
    Companion {
        mode,
        value: value.to_owned(),
        other: other.map(str::to_owned),
        reference: reference.map(str::to_owned),
    }
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
        name.to_owned(),
        Declaration::Uncertainty {
            observable: Observable {
                descriptor: descriptor.clone(),
                expression: original(context, body)?,
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
/// TODO ´todo:test:test-conflicting-shared-source-snapshots-across´: test conflicting shared source snapshots across binding points remain refusals under rule:nrz:batch-source-covariance.
///
/// ´test:integration:derived-uncertainty-propagation´
#[test]
fn derived_uncertainty_propagation() -> TestResult {
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(
        Registry::new(ExponentPolicy::Rational),
        &["rel_z", "u_z", "var_z"],
    )?;
    let target = relation(UncertaintyMode::Standard, "z", None, None);
    observable_companion(
        &mut context,
        "u_z",
        "sqrt((k1*u_x)^2+(k2*u_y)^2)",
        &standard,
        target.clone(),
    )?;
    let checked = context.check(&original(&context, "u_z")?)?;
    let expected = nrz_units::CompanionEvidence::Propagated {
        companion: target,
        contributors: std::collections::BTreeMap::from([
            (
                "u_x".to_owned(),
                relation(UncertaintyMode::Standard, "x", None, None),
            ),
            (
                "u_y".to_owned(),
                relation(UncertaintyMode::Standard, "y", None, None),
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
        let companion = relation(mode, "z", None, reference);
        observable_companion(&mut context, name, body, &descriptor, companion.clone())?;
        let checked = context.check_against(&original(&context, name)?, &descriptor)?;
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
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(
        Registry::new(ExponentPolicy::Rational),
        &[
            "cancelled_outside",
            "outside_body",
            "unbound",
            "unbound_body",
        ],
    )?;
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
            relation(UncertaintyMode::Standard, "z", None, None),
        )?;
        let error = context
            .check(&original(&context, name)?)
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
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let variance = uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    let coefficient = Descriptor::new(
        Kind::Named("coefficient".to_owned(), Box::new(Dimension::zero())),
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
        rational(2, 1)?,
        variance.kind().clone(),
    )?;
    let mut context =
        propagation_context(registry, &["binary_term", "k", "power_term", "tracked"])?;
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
            relation(UncertaintyMode::Standard, "z", None, None),
        )?;
        let error = context
            .check(&original(&context, name)?)
            .err()
            .ok_or("expected untracked uncertainty refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
        assert_eq!(error.location.path, vec![0]);
    }
    let companion = relation(UncertaintyMode::Standard, "z", None, None);
    observable_companion(
        &mut context,
        "tracked",
        "sqrt(u_x^2)",
        &standard,
        companion.clone(),
    )?;
    let checked = context.check_against(&original(&context, "tracked")?, &standard)?;
    let expected = nrz_units::CompanionEvidence::Propagated {
        companion,
        contributors: std::collections::BTreeMap::from([(
            "u_x".to_owned(),
            relation(UncertaintyMode::Standard, "x", None, None),
        )]),
    };
    assert_eq!(checked.companion_evidence(), Some(&expected));
    Ok(())
}

/// Covariance propagation admits contributors from either value and records the declared pair.
///
/// TODO ´todo:test:test-the-mixed-voltage-current-covariance´: test the mixed voltage current covariance and structured duplication maps exactly under ex:nrz:resistor-shape-covariance and rule:nrz:structured-companion-coordinates.
///
/// TODO ´todo:test:test-common-source-dependencies-survive-propagation´: test common source dependencies survive propagation across distinct binding points under rule:nrz:batch-source-covariance.
///
/// ´test:integration:covariance-propagation´
#[test]
fn covariance_propagation() -> TestResult {
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let covariance = uncertainty(UncertaintyMode::Covariance, &length, Some(&length), None)?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        standard.kind().clone(),
        standard.kind().clone(),
        covariance.kind().clone(),
    )?;
    let mut context = propagation_context(registry, &["bad_cov", "cov_xy"])?;
    let pair = relation(UncertaintyMode::Covariance, "x", Some("y"), None);
    observable_companion(
        &mut context,
        "cov_xy",
        "rho*u_x*u_y",
        &covariance,
        pair.clone(),
    )?;
    let checked = context.check_against(&original(&context, "cov_xy")?, &covariance)?;
    assert_eq!(checked.companion(), Some(&pair));
    let Some(nrz_units::CompanionEvidence::Propagated { contributors, .. }) =
        checked.companion_evidence()
    else {
        return Err("expected propagated covariance evidence".into());
    };
    assert_eq!(
        contributors.keys().cloned().collect::<Vec<_>>(),
        vec!["u_x".to_owned(), "u_y".to_owned()]
    );
    observable_companion(&mut context, "bad_cov", "rho*u_x*u_out", &covariance, pair)?;
    let error = context
        .check(&original(&context, "bad_cov")?)
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
    let length = length_descriptor()?;
    let standard = uncertainty(UncertaintyMode::Standard, &length, None, None)?;
    let mut context = propagation_context(
        Registry::new(ExponentPolicy::Rational),
        &[
            "quantity_expression",
            "quantity_product",
            "quantity_quotient",
            "scaled_z",
        ],
    )?;
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
            relation(UncertaintyMode::Standard, "x", None, None),
        )?;
        let error = context
            .check(&original(&context, name)?)
            .err()
            .ok_or("expected ratio quantity identity refusal")?;
        assert_eq!(
            error.failure,
            Failure::Units(UnitError::UncertaintyIdentity)
        );
    }
    let companion = relation(UncertaintyMode::Standard, "x", None, None);
    for source in ["2*u_x", "u_x/2", "k1*u_x", "u_x/k1"] {
        let checked = context.check(&original(&context, source)?)?;
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
        relation(UncertaintyMode::Standard, "z", None, None),
    )?;
    let checked = context.check(&original(&context, "scaled_z")?)?;
    assert!(matches!(
        checked.companion_evidence(),
        Some(nrz_units::CompanionEvidence::Propagated { .. })
    ));
    context.check(&original(&context, "(2*u_x)^2")?)?;
    let error = context
        .check(&original(&context, "(2*u_x)*(2*u_x)")?)
        .err()
        .ok_or("expected scaled-origin boundary")?;
    assert_eq!(
        error.failure,
        Failure::Units(UnitError::UnsupportedKindRule)
    );
    Ok(())
}
