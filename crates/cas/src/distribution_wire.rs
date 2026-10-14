// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical distribution statements and finite certificate payloads.

use crate::distribution_model::{DistributionCertificate, DistributionStatement, RegularityKind};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;

/// Enumerate every typed mathematical dependency in stable semantic order.
pub(crate) fn objects(value: &DistributionStatement) -> Vec<&Handle> {
    match value {
        DistributionStatement::Regularity {
            function, space, ..
        } => vec![function, space],
        DistributionStatement::Embedding {
            function,
            embedding,
            space,
        } => vec![function, embedding, space],
        DistributionStatement::Equality { left, right, space } => vec![left, right, space],
        DistributionStatement::Derivative {
            distribution,
            derivative,
            space,
            ..
        } => vec![distribution, derivative, space],
        DistributionStatement::Pairing {
            pairing,
            value,
            space,
        } => vec![pairing, value, space],
        DistributionStatement::SupportAction {
            distribution,
            function,
            neighborhood,
            space,
        } => vec![distribution, function, neighborhood, space],
    }
}

/// Exact statement tags never depend on Rust enum layout.
pub(crate) fn statement(
    value: &DistributionStatement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(match value {
        DistributionStatement::Regularity { .. } => 0,
        DistributionStatement::Embedding { .. } => 1,
        DistributionStatement::Equality { .. } => 2,
        DistributionStatement::Derivative { .. } => 3,
        DistributionStatement::Pairing { .. } => 4,
        DistributionStatement::SupportAction { .. } => 5,
    })?;
    for object in objects(value) {
        write(object, out)?;
    }
    match value {
        DistributionStatement::Regularity { property, .. } => out.byte(match property {
            RegularityKind::LocallyIntegrable => 0,
            RegularityKind::PiecewisePolynomial => 1,
            RegularityKind::Smooth => 2,
        }),
        DistributionStatement::Derivative { order, .. } => out.unsigned(u64::from(*order)),
        _ => Ok(()),
    }
}

/// Every polynomial row and exceptional point is part of the replayed evidence.
pub(crate) fn certificate(
    value: &DistributionCertificate,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        DistributionCertificate::Regular(value) => {
            out.byte(0)?;
            out.count(value.regions.len())?;
            for region in &value.regions {
                write(region, out)?;
            }
            crate::statement::case_cells(&value.partition, out)?;
            out.count(value.polynomials.len())?;
            for coefficients in &value.polynomials {
                out.byte(u8::from(coefficients.is_some()))?;
                if let Some(coefficients) = coefficients {
                    numbers(coefficients, out)?;
                }
            }
            Ok(())
        }
        DistributionCertificate::AlmostEverywhere {
            exceptions,
            partition,
        } => {
            out.byte(1)?;
            numbers(exceptions, out)?;
            crate::statement::case_cells(partition, out)
        }
        DistributionCertificate::Support(points) => {
            out.byte(2)?;
            numbers(points, out)
        }
    }
}

/// Encode exact rational families without copying coefficient storage.
fn numbers(values: &[crate::number::Rational], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        out.number(value)?;
    }
    Ok(())
}
