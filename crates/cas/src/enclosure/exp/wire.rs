// SPDX-License-Identifier: AGPL-3.0-only
//! Portable finite exponential records, decoded through validated intervals.

use super::{ExpEndpoint, ExpEvidence, ExpSeries, Interval};
use crate::Fault;
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::wire_read::Reader;

/// Frame the exponential family's bounded recurrence summaries.
pub(in crate::enclosure) fn encode(
    records: &[ExpEvidence],
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    out.count(records.len())?;
    for record in records {
        out.unsigned(u64::from(record.record))?;
        for endpoint in [&record.lower, &record.upper] {
            out.unsigned(u64::from(endpoint.squarings))?;
            out.number(&endpoint.argument)?;
            out.unsigned(u64::from(endpoint.series.degree))?;
            for value in [
                &endpoint.series.sum,
                &endpoint.series.omitted,
                &endpoint.series.tail,
            ] {
                out.number(value.lower())?;
                out.number(value.upper())?;
            }
        }
    }
    Ok(())
}

/// Bounded decoding proposes evidence and grants no mathematical authority.
pub(in crate::enclosure) fn decode(reader: &mut Reader<'_, '_>) -> Result<Vec<ExpEvidence>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 1, 2048)?;
    let mut result = Vec::new();
    result
        .try_reserve(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        result.push(ExpEvidence {
            record: reader.small()?,
            lower: endpoint(reader)?,
            upper: endpoint(reader)?,
        });
    }
    Ok(result)
}

/// Original sign belongs to the source expression, never an unauthenticated wire flag.
fn endpoint(reader: &mut Reader<'_, '_>) -> Result<ExpEndpoint, Fault> {
    Ok(ExpEndpoint {
        squarings: reader.small()?,
        argument: reader.rational()?,
        series: ExpSeries {
            degree: reader.small()?,
            sum: interval(reader)?,
            omitted: interval(reader)?,
            tail: interval(reader)?,
        },
    })
}

/// Refuse reversed intervals before admitting any decoded record.
fn interval(reader: &mut Reader<'_, '_>) -> Result<Interval, Fault> {
    let lower = reader.rational()?;
    let upper = reader.rational()?;
    Interval::new(lower, upper, reader.budget)
}
