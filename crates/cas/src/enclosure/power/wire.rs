// SPDX-License-Identifier: AGPL-3.0-only
//! Route evidence is portable, bounded and admitted only through validated intervals.

use super::{AT, Interval, PowerEvidence, PowerRoute, invalid};
use crate::Fault;
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::wire_read::Reader;

/// Frame component laws explicitly before route-specific finite evidence.
pub(in crate::enclosure) fn encode(
    records: &[PowerEvidence],
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    out.count(records.len())?;
    for record in records {
        out.unsigned(u64::from(record.record))?;
        out.byte(match record.route {
            PowerRoute::Integer => 0,
            PowerRoute::Dyadic => 1,
            PowerRoute::LogExp => 2,
        })?;
        for index in schemas(record.route) {
            out.name(super::super::ENCLOSURE_SCHEMAS[*index].name)?;
            out.unsigned(1)?;
        }
        out.count(record.roots.len())?;
        for root in &record.roots {
            out.number(root.lower())?;
            out.number(root.upper())?;
        }
        if let Some(bound) = &record.logarithm {
            out.byte(1)?;
            out.number(bound.lower())?;
            out.number(bound.upper())?;
        } else {
            out.byte(0)?;
        }
    }
    Ok(())
}

/// Decoding never grants route identity or mathematical authority.
pub(in crate::enclosure) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<Vec<PowerEvidence>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 1, 256)?;
    let mut result = Vec::new();
    result
        .try_reserve(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let record = reader.small()?;
        let route = match reader.byte()? {
            0 => PowerRoute::Integer,
            1 => PowerRoute::Dyadic,
            2 => PowerRoute::LogExp,
            _ => return Err(invalid()),
        };
        for index in schemas(route) {
            if reader.name()? != super::super::ENCLOSURE_SCHEMAS[*index].name
                || reader.small()? != 1
            {
                return Err(invalid());
            }
        }
        let count = reader.count(BudgetKind::IntegerExponent, 18, 256)?;
        let mut roots = Vec::new();
        roots
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            roots.push(interval(reader)?);
        }
        let logarithm = match reader.byte()? {
            0 => None,
            1 => Some(interval(reader)?),
            _ => return Err(invalid()),
        };
        result.push(PowerEvidence {
            record,
            route,
            roots,
            logarithm,
        });
    }
    Ok(result)
}

/// Each route fixes its component schema identities and versions.
const fn schemas(route: PowerRoute) -> &'static [usize] {
    match route {
        PowerRoute::Integer => &[],
        PowerRoute::Dyadic => &[1],
        PowerRoute::LogExp => &[2, 3],
    }
}

/// Reversed endpoints refuse at the boundary, before the record is constructed.
fn interval(reader: &mut Reader<'_, '_>) -> Result<Interval, Fault> {
    let lo = reader.rational()?;
    let hi = reader.rational()?;
    Interval::new(lo, hi, reader.budget)
}
