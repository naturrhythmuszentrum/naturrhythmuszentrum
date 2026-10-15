// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical finite-calculus statements and independently decoded endpoint evidence.

use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::number::Rational;
use crate::split_geometry::AT;
use crate::split_model::{
    SplitCertificate, SplitKind, SplitPiece, SplitPoint, SplitRange, SplitValue,
};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Stable tags do not depend on Rust enum ordering.
pub(crate) const fn kind(value: SplitKind) -> u8 {
    match value {
        SplitKind::Integral => 0,
        SplitKind::Sum => 1,
    }
}

/// Encode exact intent, shared with request identity.
pub(crate) fn range(value: &SplitRange, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.name(&value.coordinate)?;
    out.number(&value.lower)?;
    out.number(&value.upper)
}

/// Encode complete function, closed domain, orientation and value.
pub(crate) fn statement(
    value: &SplitValue,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(kind(value.kind))?;
    write(&value.function, out)?;
    write(&value.domain, out)?;
    range(&value.range, out)?;
    out.number(&value.value)
}

/// Read a statement without computing or trusting its asserted result.
pub(crate) fn decode_statement(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<SplitValue, Fault> {
    Ok(SplitValue {
        kind: match reader.byte()? {
            0 => SplitKind::Integral,
            1 => SplitKind::Sum,
            _ => return Err(invalid()),
        },
        function: object(reader, records)?,
        domain: object(reader, records)?,
        range: SplitRange {
            coordinate: reader.name()?,
            lower: reader.rational()?,
            upper: reader.rational()?,
        },
        value: reader.rational()?,
    })
}

/// Encode every local coefficient and both endpoint evaluations.
pub(crate) fn certificate(value: &SplitCertificate, out: &mut Encoder<'_>) -> Result<(), Fault> {
    crate::statement::case_cells(&value.partition, out)?;
    out.count(value.pieces.len())?;
    for piece in &value.pieces {
        out.unsigned(u64::from(piece.cell))?;
        out.number(&piece.lower)?;
        out.number(&piece.upper)?;
        numbers(&piece.polynomial, out)?;
        numbers(&piece.primitive, out)?;
        out.number(&piece.start)?;
        out.number(&piece.end)?;
    }
    out.count(value.points.len())?;
    for point in &value.points {
        out.number(&point.point)?;
        out.byte(u8::from(point.value.is_some()))?;
        if let Some(number) = &point.value {
            out.number(number)?;
        }
    }
    Ok(())
}

/// Coefficients are canonical exact rationals, including interior zeros.
pub(crate) fn numbers(values: &[Rational], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        out.number(value)?;
    }
    Ok(())
}

/// Bound vectors before allocation and reject invalid option tags immediately.
pub(crate) fn decode_certificate(reader: &mut Reader<'_, '_>) -> Result<SplitCertificate, Fault> {
    let partition = crate::wire_statement::region(reader)?;
    let count = reader.count(BudgetKind::CaseArms, 1, 256)?;
    let mut pieces = Vec::new();
    pieces
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        pieces.push(SplitPiece {
            cell: reader.small()?,
            lower: reader.rational()?,
            upper: reader.rational()?,
            polynomial: read_numbers(reader)?,
            primitive: read_numbers(reader)?,
            start: reader.rational()?,
            end: reader.rational()?,
        });
    }
    let count = reader.count(BudgetKind::BoundaryNodes, 1, 128)?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let point = reader.rational()?;
        let value = match reader.byte()? {
            0 => None,
            1 => Some(reader.rational()?),
            _ => return Err(invalid()),
        };
        points.push(SplitPoint { point, value });
    }
    Ok(SplitCertificate {
        partition,
        pieces,
        points,
    })
}

/// Decode canonical polynomial vectors with the degree bound before allocation.
pub(crate) fn read_numbers(reader: &mut Reader<'_, '_>) -> Result<Vec<Rational>, Fault> {
    let count = reader.count(BudgetKind::StagedPayloadBytes, 9, 128)?;
    reader.budget.check(
        BudgetKind::PolynomialDegree,
        crate::graph::size(count.saturating_sub(1), AT)?,
        AT,
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        result.push(reader.rational()?);
    }
    Ok(result)
}
