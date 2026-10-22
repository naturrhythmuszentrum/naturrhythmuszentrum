// SPDX-License-Identifier: AGPL-3.0-only
//! Independent descriptor expectations and explicit semantic refusals for paired entries.

use nrz_bindings::{
    Descriptor, Dimension, EvaluationDescriptor, EvaluationKind, ExponentPolicy, Kind, Measure,
    NamedReference, PairingError, PairingKind, PairingSide, PairingWitness, Registry, Role,
    UncertaintyMode, UnitError,
};
use nrz_cas::Rational;

use crate::testing::declarations::TestResult;
use crate::testing::pairing::{LENGTH, SPEED, TEMPERATURE, TIME, difference, point, rational};

/// Absolute bounds and standard uncertainties retain distinct kinds and remove point origins.
/// ´test:integration:componentwise-pairing-uses-difference-coordinates´
#[test]
fn componentwise_pairing_uses_difference_coordinates() -> TestResult {
    let temperature = point("temperature", TEMPERATURE, (5, 9), (45967, 180))?;
    for mode in [UncertaintyMode::Radius, UncertaintyMode::Standard] {
        let witness = PairingWitness::uncertainty(mode, &temperature, None, None)?;
        assert_eq!(witness.kind(), PairingKind::Uncertainty(mode));
        assert_eq!(witness.dimension(), &Dimension::integers(TEMPERATURE)?);
        assert_eq!(witness.measure().scale(), &rational(5, 9)?);
        assert_eq!(witness.measure().offset(), &Rational::zero());
        assert_eq!(witness.role(), Role::Difference);
        assert_eq!(witness.target(), &temperature);
        assert_eq!(witness.other(), None);
        assert_eq!(
            witness
                .uncertainty_descriptor()
                .ok_or("missing uncertainty")?
                .kind(),
            &Kind::Uncertainty(mode, Box::new(temperature.kind().clone()), None)
        );
    }
    let variance =
        PairingWitness::uncertainty(UncertaintyMode::Variance, &temperature, None, None)?;
    assert_eq!(
        variance.dimension(),
        &Dimension::integers([0, 0, 0, 0, 2, 0, 0])?
    );
    assert_eq!(variance.measure().scale(), &rational(25, 81)?);
    assert_eq!(variance.measure().offset(), &Rational::zero());
    Ok(())
}

/// Cross-covariance uses a sum of dimensions and both difference scales in target order.
/// ´test:integration:covariance-pairing-retains-both-target-sides´
#[test]
fn covariance_pairing_retains_both_target_sides() -> TestResult {
    let length = difference("length", LENGTH, (1, 100))?;
    let time = difference("time", TIME, (1, 1000))?;
    let covariance =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &length, Some(&time), None)?;
    assert_eq!(
        covariance.dimension(),
        &Dimension::integers([1, 0, 1, 0, 0, 0, 0])?
    );
    assert_eq!(covariance.measure().scale(), &rational(1, 100_000)?);
    assert_eq!(covariance.measure().offset(), &Rational::zero());
    assert_eq!(
        covariance
            .uncertainty_descriptor()
            .ok_or("missing covariance")?
            .kind(),
        &Kind::Uncertainty(
            UncertaintyMode::Covariance,
            Box::new(length.kind().clone()),
            Some(Box::new(time.kind().clone()))
        )
    );
    covariance.validate_targets(&length, Some(&time))?;
    assert_eq!(
        covariance.validate_targets(&time, Some(&length)),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    assert_eq!(
        covariance.validate_targets(&length, Some(&length)),
        Err(PairingError::TargetMismatch {
            side: PairingSide::Second,
        })
    );
    assert_eq!(
        covariance.validate_targets(&length, None),
        Err(PairingError::TargetMismatch {
            side: PairingSide::Second,
        })
    );
    Ok(())
}

/// Relative uncertainty needs a named nonzero difference reference, never a point center.
/// ´test:integration:pairing-relative-reference-refusals´
#[test]
fn pairing_relative_reference_refusals() -> TestResult {
    let temperature = point("temperature", TEMPERATURE, (1, 1), (27315, 100))?;
    let delta = temperature.difference();
    let reference = NamedReference::new("temperature-scale", &delta, true)?;
    for mode in [
        UncertaintyMode::RelativeRadius,
        UncertaintyMode::RelativeStandard,
    ] {
        assert_eq!(
            PairingWitness::uncertainty(mode, &temperature, None, None),
            Err(PairingError::Units(UnitError::RelativeReference))
        );
        let relative = PairingWitness::uncertainty(mode, &temperature, None, Some(&reference))?;
        assert_eq!(relative.dimension(), &Dimension::zero());
        assert_eq!(relative.measure().scale(), &Rational::one());
        assert_eq!(
            relative.reference().ok_or("reference lost")?.name(),
            "temperature-scale"
        );
        relative.validate_reference(Some(&reference))?;
        assert_eq!(
            relative.validate_reference(None),
            Err(PairingError::ReferenceMismatch)
        );
        let renamed = NamedReference::new("different-scale", &delta, true)?;
        assert_eq!(
            relative.validate_reference(Some(&renamed)),
            Err(PairingError::ReferenceMismatch)
        );
    }
    let wrong = NamedReference::new("ratio", &Descriptor::ratio(), true)?;
    assert_eq!(
        PairingWitness::uncertainty(
            UncertaintyMode::RelativeRadius,
            &temperature,
            None,
            Some(&wrong),
        ),
        Err(PairingError::Units(UnitError::RelativeReference))
    );
    Ok(())
}

/// Equal dimensions do not convert a standard uncertainty into a deterministic bound.
/// ´test:integration:pairing-modes-and-target-counts-refuse-substitution´
#[test]
fn pairing_modes_and_target_counts_refuse_substitution() -> TestResult {
    let value = Descriptor::ratio();
    let registry = Registry::new(ExponentPolicy::Rational);
    let radius = PairingWitness::uncertainty(UncertaintyMode::Radius, &value, None, None)?;
    let standard = PairingWitness::uncertainty(UncertaintyMode::Standard, &value, None, None)?;
    let entry = standard
        .uncertainty_descriptor()
        .ok_or("missing standard")?;
    assert_eq!(
        radius.check_entry(&registry, entry),
        Err(PairingError::Units(UnitError::KindMismatch))
    );
    assert_eq!(
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &value, None, None),
        Err(PairingError::Units(UnitError::UncertaintyRelationship))
    );
    assert_eq!(
        PairingWitness::uncertainty(UncertaintyMode::Variance, &value, Some(&value), None),
        Err(PairingError::Units(UnitError::UncertaintyRelationship))
    );
    let reference = NamedReference::new("scale", &value, true)?;
    assert_eq!(
        PairingWitness::uncertainty(UncertaintyMode::Radius, &value, None, Some(&reference)),
        Err(PairingError::Units(UnitError::UncertaintyRelationship))
    );
    Ok(())
}

/// Formal derivative quotients derive coordinates without registering a physical multiplication.
/// ´test:integration:sensitivity-entry-requires-semantic-kind-rule´
#[test]
fn sensitivity_entry_requires_semantic_kind_rule() -> TestResult {
    let length = difference("length", LENGTH, (1, 100))?;
    let time = difference("time", TIME, (1, 1000))?;
    let speed = difference("speed", SPEED, (1, 1))?;
    let sensitivity = PairingWitness::sensitivity(&length, &time)?;
    assert_eq!(sensitivity.dimension(), &Dimension::integers(SPEED)?);
    assert_eq!(sensitivity.measure().scale(), &rational(10, 1)?);
    assert_eq!(sensitivity.measure().offset(), &Rational::zero());
    assert_eq!(sensitivity.uncertainty_descriptor(), None);
    let mut registry = Registry::new(ExponentPolicy::Rational);
    assert_eq!(
        sensitivity.check_entry(&registry, &speed),
        Err(PairingError::Units(UnitError::UnsupportedKindRule))
    );
    registry.register_binary(
        nrz_bindings::BinaryOp::Multiply,
        speed.kind().clone(),
        time.kind().clone(),
        length.kind().clone(),
    )?;
    let conversion = sensitivity.check_entry(&registry, &speed)?;
    assert_eq!(conversion.scale, rational(1, 10)?);
    assert_eq!(conversion.shift, Rational::zero());
    let wrong = difference("unrelated", SPEED, (1, 1))?;
    assert_eq!(
        sensitivity.check_entry(&registry, &wrong),
        Err(PairingError::Units(UnitError::UnsupportedKindRule))
    );
    Ok(())
}

/// Ratio dimensions do not erase angle semantics, discrete counts or relative references.
/// ´test:integration:sensitivity-dimensionless-kind-refusals´
#[test]
fn sensitivity_dimensionless_kind_refusals() -> TestResult {
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let ratio = Descriptor::ratio();
    let registry = Registry::new(ExponentPolicy::Rational);
    let sensitivity = PairingWitness::sensitivity(&angle, &ratio)?;
    sensitivity.check_entry(&registry, &angle)?;
    assert_eq!(
        sensitivity.check_entry(&registry, &ratio),
        Err(PairingError::Units(UnitError::KindMismatch))
    );
    let count = Descriptor::new(
        Kind::Count("items".into(), nrz_bindings::CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert_eq!(
        PairingWitness::sensitivity(&ratio, &count),
        Err(PairingError::Units(UnitError::CountDomain))
    );
    assert_eq!(
        PairingWitness::sensitivity(&count, &ratio),
        Err(PairingError::Units(UnitError::CountDomain))
    );
    let continuous = PairingWitness::uncertainty(UncertaintyMode::Standard, &count, None, None)?;
    PairingWitness::sensitivity(
        continuous
            .uncertainty_descriptor()
            .ok_or("missing standard")?,
        &ratio,
    )?;
    let reference = NamedReference::new("ratio-scale", &ratio, true)?;
    let relative = PairingWitness::uncertainty(
        UncertaintyMode::RelativeRadius,
        &ratio,
        None,
        Some(&reference),
    )?;
    assert_eq!(
        PairingWitness::sensitivity(
            relative
                .uncertainty_descriptor()
                .ok_or("missing relative")?,
            &ratio,
        ),
        Err(PairingError::Units(UnitError::RelativeReference))
    );
    Ok(())
}

/// Evaluation bounds target the covariance parameter and preserve both of its product factors.
/// ´test:integration:covariance-evaluation-radius-targets-parameter´
#[test]
fn covariance_evaluation_radius_targets_parameter() -> TestResult {
    let length = difference("length", LENGTH, (1, 100))?;
    let time = difference("time", TIME, (1, 1000))?;
    let covariance =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &length, Some(&time), None)?;
    let evaluation = EvaluationDescriptor::new(&covariance);
    assert_eq!(evaluation.kind(), EvaluationKind::Radius);
    assert_eq!(evaluation.role(), Role::Difference);
    assert_eq!(
        evaluation.dimension(),
        &Dimension::integers([1, 0, 1, 0, 0, 0, 0])?
    );
    assert_eq!(evaluation.measure().scale(), &rational(1, 100_000)?);
    assert_eq!(evaluation.measure().offset(), &Rational::zero());
    assert_eq!(evaluation.target(), &covariance);
    evaluation.validate_target(&covariance)?;
    let reversed =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &time, Some(&length), None)?;
    assert_eq!(
        evaluation.validate_target(&reversed),
        Err(PairingError::EvaluationTargetMismatch)
    );
    let physical = PairingWitness::uncertainty(UncertaintyMode::Radius, &length, None, None)?;
    assert_eq!(
        evaluation.validate_target(&physical),
        Err(PairingError::EvaluationTargetMismatch)
    );
    Ok(())
}

/// Relative parameter error is explicit and cannot weaken the scalar nested-kind refusal.
/// ´test:integration:relative-evaluation-keeps-named-reference´
#[test]
fn relative_evaluation_keeps_named_reference() -> TestResult {
    let value = Descriptor::ratio();
    let reference = NamedReference::new("scale", &value, true)?;
    let relative = PairingWitness::uncertainty(
        UncertaintyMode::RelativeRadius,
        &value,
        None,
        Some(&reference),
    )?;
    let evaluation = EvaluationDescriptor::new(&relative);
    assert_eq!(evaluation.dimension(), &Dimension::zero());
    assert_eq!(evaluation.target().reference(), Some(&reference));
    let scalar = relative
        .uncertainty_descriptor()
        .ok_or("missing scalar descriptor")?;
    let nested = Kind::Uncertainty(
        UncertaintyMode::Radius,
        Box::new(scalar.kind().clone()),
        None,
    );
    assert_eq!(
        Descriptor::new(nested, Measure::coherent(), Role::Difference),
        Err(UnitError::RelativeReference)
    );
    let renamed = NamedReference::new("other-scale", &value, true)?;
    let other = PairingWitness::uncertainty(
        UncertaintyMode::RelativeRadius,
        &value,
        None,
        Some(&renamed),
    )?;
    assert_eq!(
        evaluation.validate_target(&other),
        Err(PairingError::EvaluationTargetMismatch)
    );
    let standard = PairingWitness::uncertainty(
        UncertaintyMode::RelativeStandard,
        &value,
        None,
        Some(&reference),
    )?;
    assert_eq!(
        evaluation.validate_target(&standard),
        Err(PairingError::EvaluationTargetMismatch)
    );
    Ok(())
}

/// Named references cannot be empty, possibly zero or affine points.
/// ´test:integration:named-reference-construction-refusals´
#[test]
fn named_reference_construction_refusals() -> TestResult {
    let temperature = point("temperature", TEMPERATURE, (1, 1), (27315, 100))?;
    let delta = temperature.difference();
    for name in ["", " "] {
        assert_eq!(
            NamedReference::new(name, &delta, true),
            Err(UnitError::RelativeReference)
        );
    }
    assert_eq!(
        NamedReference::new("point", &temperature, true),
        Err(UnitError::RelativeReference)
    );
    assert_eq!(
        NamedReference::new("zero", &delta, false),
        Err(UnitError::RelativeReference)
    );
    Ok(())
}

/// Dimension overflow refuses both quotient and covariance derivation without wrapping exponents.
/// ´test:integration:pairing-exponent-exhaustion´
#[test]
fn pairing_exponent_exhaustion() -> TestResult {
    let maximum = difference("max", [i64::MAX, 0, 0, 0, 0, 0, 0], (1, 1))?;
    let reciprocal = difference("inverse", [-1, 0, 0, 0, 0, 0, 0], (1, 1))?;
    let length = difference("length", LENGTH, (1, 1))?;
    assert_eq!(
        PairingWitness::sensitivity(&maximum, &reciprocal),
        Err(PairingError::Units(UnitError::ExponentRange))
    );
    assert_eq!(
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &maximum, Some(&length), None,),
        Err(PairingError::Units(UnitError::ExponentRange))
    );
    Ok(())
}

/// A registry's exponent policy still applies when checking an actual companion entry.
/// ´test:integration:pairing-entry-check-retains-registry-policy´
#[test]
fn pairing_entry_check_retains_registry_policy() -> TestResult {
    let mut exponents = std::array::from_fn(|_| Rational::zero());
    let first = exponents.first_mut().ok_or("missing dimension component")?;
    *first = rational(1, 2)?;
    let fractional = Descriptor::new(
        Kind::Named("fractional".into(), Box::new(Dimension::new(exponents)?)),
        Measure::coherent(),
        Role::Difference,
    )?;
    let radius = PairingWitness::uncertainty(UncertaintyMode::Radius, &fractional, None, None)?;
    let scalar = radius.uncertainty_descriptor().ok_or("missing radius")?;
    radius.check_entry(&Registry::new(ExponentPolicy::Rational), scalar)?;
    assert_eq!(
        radius.check_entry(&Registry::new(ExponentPolicy::Integral), scalar),
        Err(PairingError::Units(UnitError::NonIntegralDimension))
    );
    Ok(())
}
