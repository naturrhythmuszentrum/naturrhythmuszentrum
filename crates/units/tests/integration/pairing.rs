// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen declaration evidence supplies companion schemas without resolving numerical regions.

use nrz_cas::Rational;
use nrz_units::{
    BinaryOp, Companion, Declaration, Dimension, EvaluationDescriptor, Evidence, ExponentPolicy,
    NamedReference, PairingKind, PairingWitness, Registry, UncertaintyMode, uncertainty,
};

use crate::testing::declarations::{TestResult, context, context_with_registry, derived, input};
use crate::testing::pairing::{LENGTH, SPEED, TIME, difference, rational};

/// A frozen covariance retains its ordered declaration targets and its parameter error schema.
/// ´test:integration:frozen-covariance-pairing-lifecycle´
#[test]
fn frozen_covariance_pairing_lifecycle() -> TestResult {
    let mut builder = context(&["distance", "duration", "cross"])?;
    let length = difference("length", LENGTH, (1, 100))?;
    let time = difference("time", TIME, (1, 1000))?;
    input(&mut builder, "distance", &length)?;
    input(&mut builder, "duration", &time)?;
    builder.declare(
        "cross".into(),
        Declaration::UncertaintyInput {
            descriptor: uncertainty(UncertaintyMode::Covariance, &length, Some(&time), None)?,
            companion: Companion {
                mode: UncertaintyMode::Covariance,
                value: "distance".into(),
                other: Some("duration".into()),
                reference: None,
            },
            evidence: Evidence {
                source: "declared joint model".into(),
                assurance: "declared".into(),
            },
        },
    )?;
    let frozen = builder.freeze()?;
    let checked = frozen.checked("cross")?;
    let relation = checked.companion().ok_or("missing checked relation")?;
    let first = frozen.checked(&relation.value)?.descriptor();
    let second = frozen
        .checked(relation.other.as_deref().ok_or("second target missing")?)?
        .descriptor();
    let pairing = PairingWitness::uncertainty(relation.mode, first, Some(second), None)?;
    let conversion = pairing.check_entry(frozen.registry(), checked.descriptor())?;
    assert_eq!(conversion.scale, Rational::one());
    assert_eq!(conversion.shift, Rational::zero());
    let evaluation = EvaluationDescriptor::new(&pairing);
    assert_eq!(
        evaluation.dimension(),
        &Dimension::integers([1, 0, 1, 0, 0, 0, 0])?
    );
    assert_eq!(evaluation.measure().scale(), &rational(1, 100_000)?);
    assert_eq!(evaluation.target().other(), Some(second));
    frozen.validate_checked(checked)?;
    Ok(())
}

/// A checked original gain expression supplies a registered coefficient for formal propagation.
/// ´test:integration:frozen-sensitivity-pairing-lifecycle´
#[test]
fn frozen_sensitivity_pairing_lifecycle() -> TestResult {
    let length = difference("length", LENGTH, (1, 1))?;
    let time = difference("time", TIME, (1, 1))?;
    let speed = difference("speed", SPEED, (1, 1))?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        speed.kind().clone(),
        time.kind().clone(),
        length.kind().clone(),
    )?;
    let mut builder = context_with_registry(registry, &["t", "gain", "position"])?;
    input(&mut builder, "t", &time)?;
    input(&mut builder, "gain", &speed)?;
    derived(&mut builder, "position", "gain*t", &length)?;
    let frozen = builder.freeze()?;
    let output = frozen.checked("position")?;
    let source = frozen.checked("t")?;
    let coefficient = frozen.checked("gain")?;
    let sensitivity = PairingWitness::sensitivity(output.descriptor(), source.descriptor())?;
    sensitivity.check_entry(frozen.registry(), coefficient.descriptor())?;
    let covariance =
        PairingWitness::uncertainty(UncertaintyMode::Variance, source.descriptor(), None, None)?;
    let result = sensitivity.contract_covariance(&covariance, &sensitivity)?;
    assert_eq!(
        result.dimension(),
        &Dimension::integers([2, 0, 0, 0, 0, 0, 0])?
    );
    assert_eq!(
        result.kind(),
        PairingKind::Uncertainty(UncertaintyMode::Covariance)
    );
    assert_eq!(result.target(), output.descriptor());
    assert_eq!(result.other(), Some(output.descriptor()));
    let evaluation = EvaluationDescriptor::new(&sensitivity);
    assert_eq!(evaluation.dimension(), &Dimension::integers(SPEED)?);
    assert_eq!(evaluation.target().other(), Some(source.descriptor()));
    Ok(())
}

/// Named nonzero evidence comes from the frozen declaration and survives relative evaluation.
/// ´test:integration:frozen-relative-reference-lifecycle´
#[test]
fn frozen_relative_reference_lifecycle() -> TestResult {
    let mut builder = context(&["x", "reference"])?;
    let length = difference("length", LENGTH, (1, 100))?;
    input(&mut builder, "x", &length)?;
    builder.declare(
        "reference".into(),
        Declaration::Input {
            descriptor: length.clone(),
            nonzero: true,
            evidence: Evidence::default(),
        },
    )?;
    let frozen = builder.freeze()?;
    let declaration = frozen
        .declarations()
        .get("reference")
        .ok_or("reference missing")?;
    let reference = NamedReference::new(
        "reference",
        frozen.checked("reference")?.descriptor(),
        declaration.is_nonzero(),
    )?;
    let relative = PairingWitness::uncertainty(
        UncertaintyMode::RelativeRadius,
        frozen.checked("x")?.descriptor(),
        None,
        Some(&reference),
    )?;
    let evaluation = EvaluationDescriptor::new(&relative);
    assert_eq!(evaluation.dimension(), &Dimension::zero());
    assert_eq!(
        evaluation
            .target()
            .reference()
            .ok_or("reference lost")?
            .name(),
        "reference"
    );
    Ok(())
}
