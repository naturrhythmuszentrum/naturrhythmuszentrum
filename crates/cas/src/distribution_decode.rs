// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded decoding of typed distribution statements and certificates.

use crate::budget::BudgetKind;
use crate::distribution_model::{
    DistributionCertificate, DistributionStatement, RegularCertificate, RegularityKind,
};
use crate::error::Fault;
use crate::wire_decode::{Decoded, object, objects};
use crate::wire_read::{Reader, invalid};

/// Read all references before the independent mathematical role checks.
pub(crate) fn statement(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<DistributionStatement, Fault> {
    Ok(match reader.byte()? {
        6 => DistributionStatement::Pullback(Box::new(crate::pullback_wire::decode_statement(
            reader, records,
        )?)),
        7 => DistributionStatement::PrimitiveUnique {
            left: object(reader, records)?,
            right: object(reader, records)?,
            space: object(reader, records)?,
        },
        0 => DistributionStatement::Regularity {
            function: object(reader, records)?,
            space: object(reader, records)?,
            property: match reader.byte()? {
                0 => RegularityKind::LocallyIntegrable,
                1 => RegularityKind::PiecewisePolynomial,
                2 => RegularityKind::Smooth,
                _ => return Err(invalid()),
            },
        },
        1 => DistributionStatement::Embedding {
            function: object(reader, records)?,
            embedding: object(reader, records)?,
            space: object(reader, records)?,
        },
        2 => DistributionStatement::Equality {
            left: object(reader, records)?,
            right: object(reader, records)?,
            space: object(reader, records)?,
        },
        3 => DistributionStatement::Derivative {
            distribution: object(reader, records)?,
            derivative: object(reader, records)?,
            space: object(reader, records)?,
            order: reader.small()?,
        },
        4 => DistributionStatement::Pairing {
            pairing: object(reader, records)?,
            value: object(reader, records)?,
            space: object(reader, records)?,
        },
        5 => DistributionStatement::SupportAction {
            distribution: object(reader, records)?,
            function: object(reader, records)?,
            neighborhood: object(reader, records)?,
            space: object(reader, records)?,
        },
        _ => return Err(invalid()),
    })
}

/// Closed certificate tags select bounded data, never a trusted assertion.
pub(crate) fn certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<DistributionCertificate, Fault> {
    Ok(match reader.byte()? {
        0 => DistributionCertificate::Regular(Box::new(regular(reader, records)?)),
        1 => DistributionCertificate::AlmostEverywhere {
            exceptions: numbers(reader, BudgetKind::BoundaryNodes)?,
            partition: crate::wire_statement::region(reader)?,
        },
        2 => DistributionCertificate::Support(numbers(reader, BudgetKind::BoundaryNodes)?),
        _ => return Err(invalid()),
    })
}

/// Bound cells and coefficients before reserving caller-controlled vectors.
fn regular(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<RegularCertificate, Fault> {
    let regions = objects(reader, records, BudgetKind::RegionNodes)?;
    let partition = crate::wire_statement::region(reader)?;
    let count = reader.count(BudgetKind::RegionCells, 1, 32)?;
    let mut polynomials = Vec::new();
    polynomials
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(crate::distribution_syntax::AT))?;
    for _ in 0..count {
        polynomials.push(if reader.boolean()? {
            Some(numbers(reader, BudgetKind::PolynomialTerms)?)
        } else {
            None
        });
    }
    Ok(RegularCertificate {
        regions,
        partition,
        polynomials,
    })
}

/// Rational lengths are charged before any coefficient allocation or arithmetic.
fn numbers(
    reader: &mut Reader<'_, '_>,
    kind: BudgetKind,
) -> Result<Vec<crate::number::Rational>, Fault> {
    let count = reader.count(kind, 9, 128)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(crate::distribution_syntax::AT))?;
    for _ in 0..count {
        result.push(reader.rational()?);
    }
    Ok(result)
}
