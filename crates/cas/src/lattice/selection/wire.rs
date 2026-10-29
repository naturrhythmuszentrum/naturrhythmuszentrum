// SPDX-License-Identifier: AGPL-3.0-only
//! Portable exact tuple propositions and both original-target enclosure grammars.

use super::{LatticeEnclosure, LatticeSelection};
use crate::budget::{Budget, BudgetKind};
use crate::decision::{EnclosureOrigin, RationalEnclosure};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Edge, Handle, LogicalId, size};
use crate::lattice::{
    LatticeBasisQuestion, LatticeBoxEnclosure, LatticeCoordinateEnclosure, RationalBoxEnclosure,
};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::Reader;

/// Preserve the entire original question and ordered Integer tuple in statement identity.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement encoding shares the lattice selection grammar"
)]
pub(crate) fn encode(
    value: &LatticeSelection,
    tag: u8,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(tag)?;
    crate::lattice::cover::wire::encode(&value.cover, out, write)?;
    write(&value.target, out)?;
    out.count(value.values.len())?;
    for selected in &value.values {
        write(selected, out)?;
    }
    Ok(())
}

/// Bound tuple size before reconstructing ordered exact roles.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement decoding shares the lattice selection grammar"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<LatticeSelection, Fault> {
    let Decoded::Statement(judgment) = crate::wire_decode::reference(reader, records)? else {
        return Err(crate::wire_read::invalid());
    };
    let crate::proof_model::Statement::LatticeCover(cover) = &judgment.statement else {
        return Err(crate::wire_read::invalid());
    };
    if judgment.context != cover.index.basis.context
        || judgment.layer != crate::domain_model::Layer::Logic
        || judgment.space.is_some()
        || !judgment.residuals.is_empty()
    {
        return Err(crate::wire_read::invalid());
    }
    let bytes = crate::lattice::cover::check::payload(cover, reader.budget)?;
    reader.allocation(bytes)?;
    let cover = *cover.clone();
    let target = object(reader, records)?;
    let count = reader.count(BudgetKind::MatrixEntries, 4, 64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        values.push(object(reader, records)?);
    }
    Ok(LatticeSelection {
        cover,
        target,
        values,
    })
}

/// The original target and complete tuple remain ordinary graph dependencies.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement publication shares exact lattice selection roles"
)]
pub(crate) fn objects(value: &LatticeSelection) -> Vec<&Handle> {
    let mut result = crate::lattice::cover::wire::objects(&value.cover);
    result.push(&value.target);
    result.extend(&value.values);
    result
}

/// Retain every scalar origin through the existing independently versioned enclosure grammar.
fn encode_box(
    value: &RationalBoxEnclosure,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for handle in [&value.target, &value.context, &value.input_family] {
        write(handle, out)?;
    }
    out.count(value.coordinates.len())?;
    for coordinate in &value.coordinates {
        crate::decision_wire::encode_enclosure(coordinate, out, write)?;
    }
    Ok(())
}

/// An explicit vector is reconstructed before its original scalar certificates.
fn decode_box(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<RationalBoxEnclosure, Fault> {
    let target = object(reader, records)?;
    let context = object(reader, records)?;
    let input_family = object(reader, records)?;
    let coordinates = decode_scalars(reader, records)?;
    Ok(RationalBoxEnclosure {
        target,
        context,
        input_family,
        coordinates,
    })
}

/// Each bounded scalar inventory is ordered, finite and fully replayable.
fn decode_scalars(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Vec<RationalEnclosure>, Fault> {
    let count = reader.count(BudgetKind::MatrixEntries, 4, 512)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        values.push(crate::decision_wire::decode_enclosure(reader, records)?);
    }
    Ok(values)
}

/// Both supported shapes carry the exact original basis and inverse identities.
#[expect(
    clippy::redundant_pub_crate,
    reason = "certificate encoding shares original lattice box evidence"
)]
pub(crate) fn certificate(
    value: &LatticeEnclosure,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(18)?;
    out.unsigned(1)?;
    let (tag, basis, inverse) = match value {
        LatticeEnclosure::Original(value) => (0, &value.basis, &value.inverse),
        LatticeEnclosure::Coordinates(value) => (1, &value.basis, &value.inverse),
    };
    out.byte(tag)?;
    for handle in [&basis.context, &basis.basis, &basis.offset, inverse] {
        write(handle, out)?;
    }
    match value {
        LatticeEnclosure::Original(value) => {
            encode_box(&value.original, out, write)?;
            out.count(value.coefficients.len())?;
            for coefficient in &value.coefficients {
                crate::decision_wire::encode_enclosure(coefficient, out, write)?;
            }
            out.count(value.bounds.len())?;
            for (lower, upper) in &value.bounds {
                out.number(lower)?;
                out.number(upper)?;
            }
        }
        LatticeEnclosure::Coordinates(value) => {
            write(&value.target, out)?;
            encode_box(&value.coordinates, out, write)?;
        }
    }
    Ok(())
}

/// Unknown shape and certificate revisions refuse before the tuple checker.
#[expect(
    clippy::redundant_pub_crate,
    reason = "certificate decoding shares original lattice box evidence"
)]
pub(crate) fn decode_certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<LatticeEnclosure, Fault> {
    if reader.unsigned()? != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, super::AT));
    }
    let tag = reader.byte()?;
    let basis = LatticeBasisQuestion {
        context: object(reader, records)?,
        basis: object(reader, records)?,
        offset: object(reader, records)?,
    };
    let inverse = object(reader, records)?;
    match tag {
        0 => {
            let original = decode_box(reader, records)?;
            let coefficients = decode_scalars(reader, records)?;
            let count = reader.count(BudgetKind::MatrixEntries, 2, 256)?;
            let mut bounds = Vec::new();
            bounds
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(super::AT))?;
            for _ in 0..count {
                bounds.push((reader.rational()?, reader.rational()?));
            }
            Ok(LatticeEnclosure::Original(Box::new(LatticeBoxEnclosure {
                basis,
                original,
                inverse,
                coefficients,
                bounds,
            })))
        }
        1 => {
            let target = object(reader, records)?;
            let coordinates = decode_box(reader, records)?;
            Ok(LatticeEnclosure::Coordinates(Box::new(
                LatticeCoordinateEnclosure {
                    basis,
                    target,
                    inverse,
                    coordinates,
                },
            )))
        }
        _ => Err(Fault::invalid(
            InvalidCode::UnsupportedCertificate,
            super::AT,
        )),
    }
}

/// Prepay complete proposition and certificate copies before producer allocation.
pub(super) fn payload(
    cas: &Cas,
    expected: &LatticeSelection,
    value: &LatticeEnclosure,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let mut out = Encoder::new(budget, super::AT);
    let mut write = |handle: &Handle, out: &mut Encoder<'_>| {
        cas.checked(handle)?;
        crate::statement::handle(handle, out)
    };
    encode(expected, 29, &mut out, &mut write)?;
    certificate(value, &mut out, &mut write)?;
    let bytes = size(out.finish().len(), super::AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, super::AT)?;
    budget.work(bytes, super::AT)?;
    Ok(bytes)
}

/// Authenticate all embedded handles and bound certificate ownership during proof preflight.
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof preflight shares the lattice certificate grammar"
)]
pub(crate) fn measure(
    cas: &Cas,
    value: &LatticeEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = Encoder::new(budget, super::AT);
    certificate(value, &mut out, &mut |handle, out| {
        cas.checked(handle)?;
        crate::statement::handle(handle, out)
    })?;
    let bytes = size(out.finish().len(), super::AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, super::AT)?;
    budget.work(bytes, super::AT)
}

/// Publication records every original target, coefficient and origin as an ordinary dependency.
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof publication shares complete lattice certificate dependencies"
)]
pub(crate) fn dependencies(
    cas: &Cas,
    value: &LatticeEnclosure,
    edges: &mut Vec<(LogicalId, Edge)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = match value {
        LatticeEnclosure::Original(value) => value
            .original
            .coordinates
            .len()
            .saturating_add(value.coefficients.len())
            .saturating_mul(4)
            .saturating_add(7),
        LatticeEnclosure::Coordinates(value) => value
            .coordinates
            .coordinates
            .len()
            .saturating_mul(4)
            .saturating_add(8),
    };
    let retained = size(edges.len().saturating_add(count), super::AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, retained, super::AT)?;
    budget.work(size(count, super::AT)?, super::AT)?;
    edges
        .try_reserve(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    budget.with_retained_payload(retained, super::AT, |budget| {
        let mut out = Encoder::new(budget, super::AT);
        certificate(value, &mut out, &mut |handle, _out| {
            edges.push((cas.checked(handle)?, Edge::DependsOn));
            Ok(())
        })
    })
}

/// Constant citations are reconstructed from the actual retained scalar origins.
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof citations share the retained lattice Machin dependencies"
)]
pub(crate) fn uses_pi(value: &LatticeEnclosure, budget: &mut Budget) -> Result<bool, Fault> {
    let (original, coefficients) = match value {
        LatticeEnclosure::Original(value) => (&value.original, value.coefficients.as_slice()),
        LatticeEnclosure::Coordinates(value) => (&value.coordinates, &[] as &[RationalEnclosure]),
    };
    let mut found = false;
    for scalar in original.coordinates.iter().chain(coefficients) {
        budget.work(1, super::AT)?;
        found |= matches!(scalar.origin, EnclosureOrigin::PiBound { .. });
    }
    Ok(found)
}
