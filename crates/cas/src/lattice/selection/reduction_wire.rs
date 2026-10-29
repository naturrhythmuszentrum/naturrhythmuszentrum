// SPDX-License-Identifier: AGPL-3.0-only
//! One bounded integral transport wraps either existing original-target certificate grammar.

use super::{LatticeEnclosure, LatticeReduction};
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::number::Rational;
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Encode exactly one reduction wrapper and its original enclosure.
pub(super) fn encode(
    value: &LatticeReduction,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    if matches!(value.enclosure, LatticeEnclosure::Reduced(_)) {
        return Err(invalid());
    }
    out.byte(18)?;
    out.unsigned(1)?;
    out.byte(2)?;
    for matrix in [&value.transform, &value.inverse] {
        out.count(matrix.len())?;
        for entry in matrix {
            out.number(entry)?;
        }
    }
    out.count(value.values.len())?;
    for entry in &value.values {
        write(entry, out)?;
    }
    super::wire::certificate(&value.enclosure, out, write)
}

/// Reconstruct bounded matrices and indices before a nonnested base enclosure.
pub(super) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<LatticeReduction, Fault> {
    let transform = matrix(reader)?;
    let inverse = matrix(reader)?;
    let count = reader.count(BudgetKind::MatrixEntries, 4, 64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        values.push(object(reader, records)?);
    }
    let enclosure = super::wire::decode_base_certificate(reader, records)?;
    Ok(LatticeReduction {
        transform,
        inverse,
        values,
        enclosure,
    })
}

/// Bound matrix entry count and exact rational payload before allocation.
fn matrix(reader: &mut Reader<'_, '_>) -> Result<Vec<Rational>, Fault> {
    let count = reader.count(BudgetKind::MatrixEntries, 2, 128)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        entries.push(reader.rational()?);
    }
    Ok(entries)
}
