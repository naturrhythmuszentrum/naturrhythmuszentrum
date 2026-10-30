// SPDX-License-Identifier: AGPL-3.0-only
//! Portable symbolic identities retain their original order and separate interval premises.

use super::{AT, DerivativeEvidence, DerivativeRule as Rule, DerivativeStep, Interval, invalid};
use crate::Fault;
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::wire_read::Reader;

/// Frame every closed identity explicitly before its full-box enclosure.
pub(in crate::enclosure) fn encode(
    records: &[DerivativeEvidence],
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    out.count(records.len())?;
    for record in records {
        out.unsigned(u64::from(record.record))?;
        out.unsigned(u64::from(record.first))?;
        out.name(&record.coordinate)?;
        out.count(record.steps.len())?;
        for step in &record.steps {
            out.byte(tag(step.rule))?;
            out.number(step.bound.lower())?;
            out.number(step.bound.upper())?;
        }
    }
    Ok(())
}

/// Decoding validates every interval but grants no derivative authority.
pub(in crate::enclosure) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<Vec<DerivativeEvidence>, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 1, 128)?;
    let mut result = Vec::new();
    result
        .try_reserve(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let record = reader.small()?;
        let first = reader.small()?;
        let coordinate = reader.name()?;
        let count = reader.count(BudgetKind::ProofNodes, 19, 128)?;
        let mut steps = Vec::new();
        steps
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            let rule = rule(reader.byte()?)?;
            let lo = reader.rational()?;
            let hi = reader.rational()?;
            steps.push(DerivativeStep {
                rule,
                bound: Interval::new(lo, hi, reader.budget)?,
            });
        }
        result.push(DerivativeEvidence {
            record,
            first,
            coordinate,
            steps,
        });
    }
    Ok(result)
}

/// The wire inventory is closed and versioned by the enclosing derivative schema.
const fn tag(rule: Rule) -> u8 {
    match rule {
        Rule::Constant => 0,
        Rule::Coordinate => 1,
        Rule::Parameter => 2,
        Rule::Negate => 3,
        Rule::Sum => 4,
        Rule::Difference => 5,
        Rule::Product => 6,
        Rule::Quotient => 7,
        Rule::Exponential => 8,
        Rule::Logarithm => 9,
        Rule::Sigmoid => 10,
        Rule::Tanh => 11,
        Rule::Power => 12,
        Rule::Root => 13,
    }
}

/// Unknown identities cannot enter a checked derivative transcript.
const fn rule(tag: u8) -> Result<Rule, Fault> {
    Ok(match tag {
        0 => Rule::Constant,
        1 => Rule::Coordinate,
        2 => Rule::Parameter,
        3 => Rule::Negate,
        4 => Rule::Sum,
        5 => Rule::Difference,
        6 => Rule::Product,
        7 => Rule::Quotient,
        8 => Rule::Exponential,
        9 => Rule::Logarithm,
        10 => Rule::Sigmoid,
        11 => Rule::Tanh,
        12 => Rule::Power,
        13 => Rule::Root,
        _ => return Err(invalid()),
    })
}
