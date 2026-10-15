// SPDX-License-Identifier: AGPL-3.0-only
//! Literal dimension and scale fixtures independent of the pairing derivation.

use nrz_cas::{
    Fault, Rational,
    limits::{Budget, Limits},
};

use super::declarations::TestResult;
use crate::test_api::{Descriptor, Dimension, Kind, Measure, Role, Scale};

/// Construct a literal coefficient; expected values never call a pairing operation.
pub(crate) fn rational(numerator: i64, denominator: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(numerator, denominator, &mut Budget::new(Limits::default())?)
}

/// Explicit named schema with independent exponent, scale and origin literals.
fn descriptor(
    name: &str,
    exponents: [i64; 7],
    scale: (i64, u64),
    offset: (i64, u64),
    role: Role,
) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(exponents)?)),
        Measure::new(
            format!("{name}-measure"),
            Scale::Exact(rational(scale.0, scale.1)?),
            rational(offset.0, offset.1)?,
        )?,
        role,
    )?)
}

/// Length dimension in the fixed SI basis.
pub(crate) const LENGTH: [i64; 7] = [1, 0, 0, 0, 0, 0, 0];
/// Time dimension in the fixed SI basis.
pub(crate) const TIME: [i64; 7] = [0, 0, 1, 0, 0, 0, 0];
/// Temperature dimension in the fixed SI basis.
pub(crate) const TEMPERATURE: [i64; 7] = [0, 0, 0, 0, 1, 0, 0];
/// Voltage dimension in the fixed SI basis.
pub(crate) const VOLTAGE: [i64; 7] = [2, 1, -3, -1, 0, 0, 0];
/// Current dimension in the fixed SI basis.
pub(crate) const CURRENT: [i64; 7] = [0, 0, 0, 1, 0, 0, 0];
/// Energy and torque share dimensions but have distinct fixture kinds.
pub(crate) const ENERGY: [i64; 7] = [2, 1, -2, 0, 0, 0, 0];
/// Speed dimension in the fixed SI basis.
pub(crate) const SPEED: [i64; 7] = [1, 0, -1, 0, 0, 0, 0];

/// A named difference fixture, with an explicitly literal scale and no origin.
pub(crate) fn difference(
    name: &str,
    exponents: [i64; 7],
    scale: (i64, u64),
) -> TestResult<Descriptor> {
    descriptor(name, exponents, scale, (0, 1), Role::Difference)
}

/// A named affine fixture retaining an explicit literal origin.
pub(crate) fn point(
    name: &str,
    exponents: [i64; 7],
    scale: (i64, u64),
    offset: (i64, u64),
) -> TestResult<Descriptor> {
    descriptor(name, exponents, scale, offset, Role::Point)
}
