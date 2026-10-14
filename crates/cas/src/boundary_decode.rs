// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded independent decoding of boundary statements and certificates.

use crate::boundary_model::{
    BoundaryCertificate, BoundaryPartition, CaseDerivative, NonexistenceReason, PointDerivative,
    PointNonexistence, TraceValue,
};
use crate::budget::BudgetKind;
use crate::domain_model::Side;
use crate::error::{Fault, Location};
use crate::proof_model::Statement;
use crate::wire_decode::{Decoded, object, objects};
use crate::wire_read::{Reader, invalid};

/// Decode positive derivative orders before constructing a statement.
fn order(reader: &mut Reader<'_, '_>) -> Result<u32, Fault> {
    let value = reader.small()?;
    if value == 0 {
        return Err(invalid());
    }
    reader.budget.check(
        BudgetKind::JetOrder,
        u64::from(value),
        Location::new("derivative order"),
    )?;
    Ok(value)
}

/// Decode exact tags independently of Rust's enum layout.
fn side(reader: &mut Reader<'_, '_>) -> Result<Side, Fault> {
    match reader.byte()? {
        0 => Ok(Side::Below),
        1 => Ok(Side::Above),
        _ => Err(invalid()),
    }
}

/// Read the complete partition or region derivative before later role validation.
pub(crate) fn statement(
    tag: u8,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Statement, Fault> {
    Ok(match tag {
        17 => Statement::BoundaryPartition(Box::new(BoundaryPartition {
            function: object(reader, records)?,
            domain: object(reader, records)?,
            coordinate: reader.name()?,
            boundaries: objects(reader, records, BudgetKind::BoundaryNodes)?,
            cells: objects(reader, records, BudgetKind::RegionCells)?,
        })),
        18 => Statement::CaseDerivative(Box::new(CaseDerivative {
            function: object(reader, records)?,
            derivative: object(reader, records)?,
            coordinate: reader.name()?,
            order: order(reader)?,
            domain: object(reader, records)?,
        })),
        _ => return point_statement(tag, reader, records),
    })
}

/// Read point derivative, nonexistence and oriented trace records.
fn point_statement(
    tag: u8,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Statement, Fault> {
    Ok(match tag {
        19 => Statement::DerivativeAt(Box::new(PointDerivative {
            function: object(reader, records)?,
            boundary: object(reader, records)?,
            coordinate: reader.name()?,
            order: order(reader)?,
            value: object(reader, records)?,
        })),
        20 => Statement::DerivativeDoesNotExist(Box::new(PointNonexistence {
            function: object(reader, records)?,
            boundary: object(reader, records)?,
            coordinate: reader.name()?,
            order: order(reader)?,
            reason: match reader.byte()? {
                0 => NonexistenceReason::Discontinuous(Side::Below),
                1 => NonexistenceReason::Discontinuous(Side::Above),
                2 => NonexistenceReason::UnequalSlopes,
                _ => return Err(invalid()),
            },
        })),
        21 => Statement::TraceValue(Box::new(TraceValue {
            function: object(reader, records)?,
            boundary: object(reader, records)?,
            side: side(reader)?,
            order: reader.small()?,
            value: object(reader, records)?,
        })),
        _ => return Err(invalid()),
    })
}

/// Every vector is bounded before allocation; a mode tag never proves anything.
pub(crate) fn certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<BoundaryCertificate, Fault> {
    Ok(match reader.byte()? {
        0 => BoundaryCertificate::Partition {
            regions: objects(reader, records, BudgetKind::RegionNodes)?,
            partition: crate::wire_statement::region(reader)?,
        },
        1 => polynomial(reader, records)?,
        2 => BoundaryCertificate::Projection,
        3 => BoundaryCertificate::Jump,
        4 => BoundaryCertificate::Interior,
        5 => BoundaryCertificate::Point,
        6 => BoundaryCertificate::Assemble,
        _ => return Err(invalid()),
    })
}

/// Degree and rational component bounds apply even to a malicious tiny stream.
fn polynomial(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<BoundaryCertificate, Fault> {
    let polynomial = object(reader, records)?;
    let count = reader.count(BudgetKind::StagedPayloadBytes, 9, 128)?;
    let degree = u64::try_from(count.saturating_sub(1)).map_err(|_| invalid())?;
    reader.budget.check(
        BudgetKind::PolynomialDegree,
        degree,
        Location::new("jet coefficients"),
    )?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(Location::new("jet coefficients")))?;
    for _ in 0..count {
        coefficients.push(reader.rational()?);
    }
    Ok(BoundaryCertificate::Polynomial {
        polynomial,
        coefficients,
    })
}
