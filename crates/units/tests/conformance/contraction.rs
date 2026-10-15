// SPDX-License-Identifier: AGPL-3.0-only
//! Formal contraction laws check modes and both exact target schemas.

use nrz_units::{
    Descriptor, Dimension, PairingError, PairingKind, PairingSide, PairingWitness, UncertaintyMode,
};

use crate::testing::declarations::TestResult;
use crate::testing::pairing::{
    CURRENT, ENERGY, LENGTH, TEMPERATURE, TIME, VOLTAGE, difference, point, rational,
};

/// Two sensitivity sides contract a cross-covariance into the paired output product.
/// ´test:integration:paired-sensitivity-covariance-contraction´
#[test]
fn paired_sensitivity_covariance_contraction() -> TestResult {
    let length = difference("length", LENGTH, (1, 100))?;
    let time = difference("time", TIME, (1, 1000))?;
    let voltage = difference("voltage", VOLTAGE, (1, 1000))?;
    let current = difference("current", CURRENT, (2, 1))?;
    let covariance =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, &length, Some(&time), None)?;
    let left = PairingWitness::sensitivity(&voltage, &length)?;
    let right = PairingWitness::sensitivity(&current, &time)?;
    let result = left.contract_covariance(&covariance, &right)?;
    assert_eq!(
        result.kind(),
        PairingKind::Uncertainty(UncertaintyMode::Covariance)
    );
    assert_eq!(
        result.dimension(),
        &Dimension::integers([2, 1, -3, 0, 0, 0, 0])?
    );
    assert_eq!(result.measure().scale(), &rational(1, 500)?);
    assert_eq!(result.target(), &voltage);
    assert_eq!(result.other(), Some(&current));
    let variance = PairingWitness::uncertainty(UncertaintyMode::Variance, &length, None, None)?;
    let repeated = left.contract_covariance(&variance, &left)?;
    assert_eq!(
        repeated.dimension(),
        &Dimension::integers([4, 2, -6, -2, 0, 0, 0])?
    );
    assert_eq!(repeated.measure().scale(), &rational(1, 1_000_000)?);
    Ok(())
}

/// Equal dimensions on the wrong input kind still fail on the identified contraction side.
/// ´test:integration:covariance-contraction-rejects-each-wrong-kind-side´
#[test]
fn covariance_contraction_rejects_each_wrong_kind_side() -> TestResult {
    let left_value = difference("energy", ENERGY, (1, 1))?;
    let right_value = difference("torque", ENERGY, (1, 1))?;
    let covariance = PairingWitness::uncertainty(
        UncertaintyMode::Covariance,
        &left_value,
        Some(&right_value),
        None,
    )?;
    let left = PairingWitness::sensitivity(&Descriptor::ratio(), &left_value)?;
    let right = PairingWitness::sensitivity(&Descriptor::ratio(), &right_value)?;
    left.contract_covariance(&covariance, &right)?;
    assert_eq!(
        right.contract_covariance(&covariance, &right),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    assert_eq!(
        left.contract_covariance(&covariance, &left),
        Err(PairingError::TargetMismatch {
            side: PairingSide::Second,
        })
    );
    let reversed = PairingWitness::uncertainty(
        UncertaintyMode::Covariance,
        &right_value,
        Some(&left_value),
        None,
    )?;
    assert_eq!(
        left.contract_covariance(&reversed, &right),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    Ok(())
}

/// A coordinate or origin change requires an explicit new pairing before contraction.
/// ´test:integration:contraction-target-coordinates-are-exact´
#[test]
fn contraction_target_coordinates_are_exact() -> TestResult {
    let centimetres = difference("length", LENGTH, (1, 100))?;
    let metres = difference("length", LENGTH, (1, 1))?;
    let variance = PairingWitness::uncertainty(UncertaintyMode::Variance, &metres, None, None)?;
    let old = PairingWitness::sensitivity(&Descriptor::ratio(), &centimetres)?;
    let new = PairingWitness::sensitivity(&Descriptor::ratio(), &metres)?;
    assert_eq!(
        old.contract_covariance(&variance, &new),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    assert_eq!(
        new.contract_covariance(&variance, &old),
        Err(PairingError::TargetMismatch {
            side: PairingSide::Second,
        })
    );
    new.contract_covariance(&variance, &new)?;
    let celsius = point("temperature", TEMPERATURE, (1, 1), (27315, 100))?;
    let kelvin = point("temperature", TEMPERATURE, (1, 1), (0, 1))?;
    let derivative = PairingWitness::sensitivity(&celsius, &metres)?;
    assert_eq!(derivative.measure().offset(), &nrz_cas::Rational::zero());
    assert_eq!(
        derivative.validate_targets(&kelvin, Some(&metres)),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    Ok(())
}

/// Statistical and deterministic modes never substitute for one another during contraction.
/// ´test:integration:contraction-mode-refusals´
#[test]
fn contraction_mode_refusals() -> TestResult {
    let value = Descriptor::ratio();
    let radius = PairingWitness::uncertainty(UncertaintyMode::Radius, &value, None, None)?;
    let standard = PairingWitness::uncertainty(UncertaintyMode::Standard, &value, None, None)?;
    let variance = PairingWitness::uncertainty(UncertaintyMode::Variance, &value, None, None)?;
    let derivative = PairingWitness::sensitivity(&value, &value)?;
    let output = derivative.contract_radius(&radius)?;
    assert_eq!(
        output.kind(),
        PairingKind::Uncertainty(UncertaintyMode::Radius)
    );
    assert_eq!(
        derivative.contract_radius(&standard),
        Err(PairingError::ExpectedRadius)
    );
    assert_eq!(
        derivative.contract_covariance(&radius, &derivative),
        Err(PairingError::ExpectedCovariance)
    );
    assert_eq!(
        radius.contract_covariance(&variance, &derivative),
        Err(PairingError::ExpectedSensitivity)
    );
    assert_eq!(
        derivative.contract_covariance(&variance, &radius),
        Err(PairingError::ExpectedSensitivity)
    );
    let reference = nrz_units::NamedReference::new("scale", &value, true)?;
    let relative = PairingWitness::uncertainty(
        UncertaintyMode::RelativeRadius,
        &value,
        None,
        Some(&reference),
    )?;
    assert_eq!(
        derivative.contract_radius(&relative),
        Err(PairingError::ExpectedRadius)
    );
    let other = difference("angle-like", [0; 7], (1, 1))?;
    let unrelated = PairingWitness::uncertainty(UncertaintyMode::Radius, &other, None, None)?;
    assert_eq!(
        derivative.contract_radius(&unrelated),
        Err(PairingError::TargetMismatch {
            side: PairingSide::First,
        })
    );
    Ok(())
}
