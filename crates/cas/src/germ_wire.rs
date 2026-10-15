// SPDX-License-Identifier: AGPL-3.0-only
//! Portable germ approach, relation and finite remainder records.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::Side;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::germ_check::AT;
use crate::germ_model::{GermAt, GermCertificate, GermMode, GermValue};
use crate::graph::{Handle, size};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// The point is an ordinary typed dependency, with an explicit one- or two-sided approach.
pub(crate) fn approach(
    value: &GermAt,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.name(&value.coordinate)?;
    write(&value.point, out)?;
    out.byte(match value.side {
        None => 0,
        Some(Side::Below) => 1,
        Some(Side::Above) => 2,
    })
}

/// Encode the exact result mode and all retained coefficients, including trailing zero entries.
pub(crate) fn statement(
    value: &GermValue,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    write(&value.function, out)?;
    approach(&value.at, out, &mut write)?;
    match value.mode {
        GermMode::Limit => out.byte(0)?,
        GermMode::Series { order, analytic } => {
            out.byte(1)?;
            out.unsigned(u64::from(order))?;
            out.byte(u8::from(analytic))?;
        }
    }
    crate::split_wire::numbers(&value.coefficients, out)
}

/// Read the exact approach without inventing an incident neighborhood.
fn read_approach(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<GermAt, Fault> {
    Ok(GermAt {
        coordinate: reader.name()?,
        point: object(reader, records)?,
        side: match reader.byte()? {
            0 => None,
            1 => Some(Side::Below),
            2 => Some(Side::Above),
            _ => return Err(invalid()),
        },
    })
}

/// Decode the explicit analytic tag and requested order before coefficient allocation.
pub(crate) fn decode_statement(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<GermValue, Fault> {
    let function = object(reader, records)?;
    let at = read_approach(reader, records)?;
    let mode = match reader.byte()? {
        0 => GermMode::Limit,
        1 => {
            let order = reader.small()?;
            reader
                .budget
                .check(BudgetKind::JetOrder, u64::from(order), AT)?;
            let analytic = match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(invalid()),
            };
            GermMode::Series { order, analytic }
        }
        _ => return Err(invalid()),
    };
    Ok(GermValue {
        function,
        at,
        mode,
        coefficients: crate::split_wire::read_numbers(reader)?,
    })
}

/// The exact remainder is independent data consumed by the polynomial identity checker.
pub(crate) fn certificate(value: &GermCertificate, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(value.remainders.len())?;
    for coefficients in &value.remainders {
        crate::split_wire::numbers(coefficients, out)?;
    }
    Ok(())
}

/// At most two required sides can contribute remainder vectors.
pub(crate) fn decode_certificate(reader: &mut Reader<'_, '_>) -> Result<GermCertificate, Fault> {
    let count = reader.count(BudgetKind::CaseArms, 4, 64)?;
    if count > 2 {
        return Err(invalid());
    }
    let mut remainders = Vec::new();
    remainders
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        remainders.push(crate::split_wire::read_numbers(reader)?);
    }
    Ok(GermCertificate { remainders })
}

/// Bound untrusted remainder payload before copying or encoding it.
pub(crate) fn measure(value: &GermCertificate, budget: &mut Budget) -> Result<(), Fault> {
    if value.remainders.len() > 2 {
        return Err(invalid());
    }
    budget.check(BudgetKind::CaseArms, size(value.remainders.len(), AT)?, AT)?;
    let mut bytes = 0_u64;
    for coefficients in &value.remainders {
        crate::univariate::validate(coefficients, budget)?;
        bytes = bytes
            .checked_add(crate::univariate::payload(coefficients, budget)?)
            .ok_or_else(invalid)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}
