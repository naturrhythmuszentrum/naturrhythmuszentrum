// SPDX-License-Identifier: AGPL-3.0-only
//! Versioned closed certificates retain exact source identities for independent replay.

use crate::budget::{Budget, BudgetKind};
use crate::decision::{
    CaseCut, CasePartition, CoordinateInterval, DecisionIntroduction, EnclosureOrigin,
    IntegerCellRange, RationalEnclosure,
};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::wire_decode::{Decoded, object, objects};
use crate::wire_read::{Reader, invalid};

/// Portable certificate accounting location.
const AT: Location = Location::new("decision certificate");

/// Version and every proof-relevant identity are part of interning and portable bytes.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof and wire dispatchers share this checker"
)]
pub(crate) fn encode(
    value: &DecisionIntroduction,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.unsigned(1)?;
    write(&value.partition.function, out)?;
    out.count(value.partition.coordinates.len())?;
    for coordinate in &value.partition.coordinates {
        write(coordinate, out)?;
    }
    integer(value.partition.integer.as_ref(), out, write)?;
    write(&value.cut.function, out)?;
    write(&value.cut.association, out)?;
    out.count(value.cut.bounds.len())?;
    for bound in &value.cut.bounds {
        write(&bound.coordinate, out)?;
        out.number(&bound.lower)?;
        out.number(&bound.upper)?;
    }
    integer(value.cut.integer.as_ref(), out, write)?;
    out.count(value.enclosures.len())?;
    for enclosure in &value.enclosures {
        encode_enclosure(enclosure, out, write)?;
    }
    Ok(())
}

/// Encode the optional finite integer cell range.
fn integer(
    range: Option<&IntegerCellRange>,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(u8::from(range.is_some()))?;
    if let Some(range) = range {
        write(&range.coordinate, out)?;
        for endpoint in [&range.lower, &range.upper] {
            out.byte(u8::from(endpoint.is_some()))?;
            if let Some(endpoint) = endpoint {
                out.number(endpoint)?;
            }
        }
    }
    Ok(())
}

/// Retain the original target, family, bounds and origin.
#[expect(
    clippy::redundant_pub_crate,
    reason = "finite and Integer selection certificates share original enclosure syntax"
)]
pub(crate) fn encode_enclosure(
    enclosure: &RationalEnclosure,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for handle in [
        &enclosure.target,
        &enclosure.context,
        &enclosure.input_family,
    ] {
        write(handle, out)?;
    }
    out.number(&enclosure.lower)?;
    out.number(&enclosure.upper)?;
    match &enclosure.origin {
        EnclosureOrigin::ExactRational { expression, value } => {
            out.byte(0)?;
            write(expression, out)?;
            out.number(value)
        }
        EnclosureOrigin::PiBound {
            expression,
            certificate,
        } => {
            out.byte(4)?;
            write(expression, out)?;
            out.unsigned(u64::from(certificate.version))?;
            for count in certificate.terms {
                out.unsigned(u64::from(count))?;
            }
            for value in [
                &certificate.sums[0],
                &certificate.sums[1],
                &certificate.lower,
                &certificate.upper,
            ] {
                out.number(value)?;
            }
            Ok(())
        }
        EnclosureOrigin::ScalarKernel(certificate) => {
            out.byte(5)?;
            crate::decision_kernel::encode(certificate, out, write)
        }
        EnclosureOrigin::MatrixKernel => out.byte(2),
        EnclosureOrigin::Unverified => out.byte(3),
    }
}

/// Account for copies and authenticate all embedded handles before interpretation.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof and wire dispatchers share this checker"
)]
pub(crate) fn measure(
    cas: &Cas,
    value: &DecisionIntroduction,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = Encoder::new(budget, AT);
    encode(value, &mut out, &mut |handle, out| {
        cas.checked(handle)?;
        crate::statement::handle(handle, out)
    })?;
    let bytes = size(out.finish().len(), AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}

/// Decode only the supported semantic certificate version.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof and wire dispatchers share this checker"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<DecisionIntroduction, Fault> {
    if reader.unsigned()? != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    let partition = CasePartition {
        function: object(reader, records)?,
        coordinates: objects(reader, records, BudgetKind::ConditionAtoms)?,
        integer: decode_integer(reader, records)?,
    };
    let function = object(reader, records)?;
    let association = object(reader, records)?;
    let count = reader.count(BudgetKind::ConditionAtoms, 6, 384)?;
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        bounds.push(CoordinateInterval {
            coordinate: object(reader, records)?,
            lower: reader.rational()?,
            upper: reader.rational()?,
        });
    }
    let cut = CaseCut {
        function,
        association,
        bounds,
        integer: decode_integer(reader, records)?,
    };
    let count = reader.count(BudgetKind::ConditionAtoms, 15, 800)?;
    let mut enclosures = Vec::new();
    enclosures
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        enclosures.push(decode_enclosure(reader, records)?);
    }
    Ok(DecisionIntroduction {
        partition,
        cut,
        enclosures,
    })
}

/// Read both optional endpoints without inventing unbounded cells.
fn decode_integer(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Option<IntegerCellRange>, Fault> {
    if !reader.boolean()? {
        return Ok(None);
    }
    let coordinate = object(reader, records)?;
    let lower = if reader.boolean()? {
        Some(reader.rational()?)
    } else {
        None
    };
    let upper = if reader.boolean()? {
        Some(reader.rational()?)
    } else {
        None
    };
    Ok(Some(IntegerCellRange {
        coordinate,
        lower,
        upper,
    }))
}

/// Decode an original enclosure for independent semantic replay.
#[expect(
    clippy::redundant_pub_crate,
    reason = "finite and Integer selection certificates share original enclosure syntax"
)]
pub(crate) fn decode_enclosure(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<RationalEnclosure, Fault> {
    let target = object(reader, records)?;
    let context = object(reader, records)?;
    let input_family = object(reader, records)?;
    let lower = reader.rational()?;
    let upper = reader.rational()?;
    let origin = match reader.byte()? {
        0 => EnclosureOrigin::ExactRational {
            expression: object(reader, records)?,
            value: reader.rational()?,
        },
        1 => return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT)),
        5 => EnclosureOrigin::ScalarKernel(Box::new(crate::decision_kernel::decode(
            reader, records,
        )?)),
        2 => EnclosureOrigin::MatrixKernel,
        3 => EnclosureOrigin::Unverified,
        4 => EnclosureOrigin::PiBound {
            expression: object(reader, records)?,
            certificate: Box::new(decode_pi(reader)?),
        },
        _ => return Err(invalid()),
    };
    Ok(RationalEnclosure {
        target,
        context,
        input_family,
        lower,
        upper,
        origin,
    })
}

/// Encode the two closed strict-sign consequences with independent semantic versions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement encoder shares auxiliary decision certificates"
)]
pub(crate) fn sign(
    value: &crate::proof_model::Certificate,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    use crate::proof_model::Certificate;
    match value {
        Certificate::DecisionNonZero(reference) => {
            out.byte(15)?;
            out.unsigned(1)?;
            write(&reference.context, out)?;
            out.unsigned(u64::from(reference.slot))
        }
        Certificate::DecisionDischarge => {
            out.byte(16)?;
            out.unsigned(1)
        }
        _ => Err(invalid()),
    }
}

/// Refuse unknown consequence versions before reading their qualified references.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the wire statement decoder shares auxiliary decision certificates"
)]
pub(crate) fn decode_sign(
    tag: u8,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<crate::proof_model::Certificate, Fault> {
    use crate::proof_model::Certificate;
    if reader.unsigned()? != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    match tag {
        15 => Ok(Certificate::DecisionNonZero(
            crate::domain_model::HypothesisRef {
                context: object(reader, records)?,
                slot: reader.small()?,
            },
        )),
        16 => Ok(Certificate::DecisionDischarge),
        _ => Err(invalid()),
    }
}

/// A pi certificate has its own semantic revision within the additive origin tag.
fn decode_pi(reader: &mut Reader<'_, '_>) -> Result<crate::pi_bounds::PiBoundCertificate, Fault> {
    let version = reader.small()?;
    if version != crate::proof::CONSTANT_SCHEMAS[0].version {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    Ok(crate::pi_bounds::PiBoundCertificate {
        version,
        terms: [reader.small()?, reader.small()?],
        sums: [reader.rational()?, reader.rational()?],
        lower: reader.rational()?,
        upper: reader.rational()?,
    })
}
