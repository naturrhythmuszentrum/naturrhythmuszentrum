// SPDX-License-Identifier: AGPL-3.0-only
//! Exact schema fixtures use literal definitions and only public interfaces.

use super::declarations::TestResult;
use crate::test_api::{Context, Declaration, Descriptor, Evidence, Kind, Measure, Role, Scale};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits},
};

/// Exact test literals are not obtained from production conversions.
pub(crate) fn rational(text: &str) -> TestResult<Rational> {
    Ok(Rational::parse(text, &mut Budget::new(Limits::default())?)?)
}

/// A difference schema with an independently specified exact measure scale.
pub(crate) fn difference(kind: Kind, scale: &str) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        kind,
        Measure::new(
            "reference",
            Scale::Exact(rational(scale)?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?)
}

/// A zero point coordinate with an independently specified coherent origin.
pub(crate) fn point(kind: Kind, origin: &str) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        kind,
        Measure::new("origin", Scale::Exact(Rational::one()), rational(origin)?)?,
        Role::Point,
    )?)
}

/// Declare a descriptor constant through the ordinary public context path.
pub(crate) fn constant(
    builder: &mut Context,
    key: &str,
    descriptor: &Descriptor,
    value: &str,
) -> TestResult {
    builder.declare(
        key.into(),
        Declaration::Constant {
            descriptor: descriptor.clone(),
            value: rational(value)?,
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}
