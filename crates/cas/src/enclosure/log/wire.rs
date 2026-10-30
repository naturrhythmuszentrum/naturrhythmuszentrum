// SPDX-License-Identifier: AGPL-3.0-only
//! Closed finite analytic evidence framing; all decoded intervals use validation.

use super::{Interval, LogEndpoint, LogEvidence, LogSeries};
use crate::Fault;
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::wire_read::Reader;

/// Append the log family's evidence without changing earlier families' bytes.
pub(in crate::enclosure) fn encode(
    records: &[LogEvidence],
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    out.count(records.len())?;
    for record in records {
        out.unsigned(u64::from(record.record))?;
        for endpoint in [&record.lower, &record.upper] {
            out.byte(u8::from(endpoint.exponent.is_negative()))?;
            out.unsigned(endpoint.exponent.unsigned_abs())?;
            out.number(&endpoint.mantissa)?;
            out.number(&endpoint.argument)?;
            for series in [&endpoint.reduced, &endpoint.two] {
                out.unsigned(u64::from(series.terms))?;
                for value in [&series.sum, &series.power, &series.tail] {
                    out.number(value.lower())?;
                    out.number(value.upper())?;
                }
            }
        }
    }
    Ok(())
}

/// Decode only bounded canonical records; successful decoding confers no authority.
pub(in crate::enclosure) fn decode(reader: &mut Reader<'_, '_>) -> Result<Vec<LogEvidence>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 1, 4096)?;
    let mut result = Vec::new();
    result
        .try_reserve(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        result.push(LogEvidence {
            record: reader.small()?,
            lower: endpoint(reader)?,
            upper: endpoint(reader)?,
        });
    }
    Ok(result)
}

/// Signed exponents reject negative zero and magnitudes outside the checked representation.
fn endpoint(reader: &mut Reader<'_, '_>) -> Result<LogEndpoint, Fault> {
    let negative = reader.boolean()?;
    let magnitude = reader.unsigned()?;
    let exponent = i64::try_from(magnitude).map_err(|_| super::invalid())?;
    if negative && exponent == 0 {
        return Err(super::invalid());
    }
    Ok(LogEndpoint {
        exponent: if negative { -exponent } else { exponent },
        mantissa: reader.rational()?,
        argument: reader.rational()?,
        reduced: series(reader)?,
        two: series(reader)?,
    })
}

/// A finite recurrence carries three independently checked interval summaries.
fn series(reader: &mut Reader<'_, '_>) -> Result<LogSeries, Fault> {
    Ok(LogSeries {
        terms: reader.small()?,
        sum: interval(reader)?,
        power: interval(reader)?,
        tail: interval(reader)?,
    })
}

/// Preserve the core decoder's validated endpoint boundary.
fn interval(reader: &mut Reader<'_, '_>) -> Result<Interval, Fault> {
    let lower = reader.rational()?;
    let upper = reader.rational()?;
    Interval::new(lower, upper, reader.budget)
}
