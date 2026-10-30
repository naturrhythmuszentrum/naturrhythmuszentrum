// SPDX-License-Identifier: AGPL-3.0-only
//! Additive versioned scalar-origin encoding; existing origins keep their bytes.

use super::model::{
    ScalarKernelCertificate, ScalarKernelFamily, ScalarKernelFamilyInput, ScalarKernelInput,
    ScalarKernelInputValue, ScalarKernelLaw, ScalarKernelReading, ScalarKernelRecord,
};
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::Handle;
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Unknown scalar grammar versions fail before any certificate allocation.
const AT: Location = Location::new("scalar kernel wire");

/// Encode exact original scope and ordered pure rational law records.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling decision wire dispatcher calls the scalar encoder"
)]
pub(crate) fn encode(
    value: &ScalarKernelCertificate,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.unsigned(u64::from(value.version))?;
    for handle in [&value.expression, &value.context, &value.input_family] {
        write(handle, out)?;
    }
    out.count(value.records.len())?;
    for record in &value.records {
        match &record.law {
            ScalarKernelLaw::Reading(value) => {
                out.byte(7)?;
                write(&value.definition, out)?;
                out.unsigned(u64::from(value.input))?;
                out.number(&value.scale)?;
                out.number(&value.offset)?;
            }
            ScalarKernelLaw::Family(value) => {
                out.byte(6)?;
                encode_family(value, out, write)?;
            }
            ScalarKernelLaw::Input(value) => {
                out.byte(5)?;
                encode_input(value, out)?;
            }
            ScalarKernelLaw::Rational(value) => {
                out.byte(0)?;
                out.number(value)?;
            }
            ScalarKernelLaw::Sum { left, right } => {
                out.byte(1)?;
                out.unsigned(u64::from(*left))?;
                out.unsigned(u64::from(*right))?;
            }
            ScalarKernelLaw::Product { left, right } => {
                out.byte(2)?;
                out.unsigned(u64::from(*left))?;
                out.unsigned(u64::from(*right))?;
            }
            ScalarKernelLaw::Reciprocal { input } => {
                out.byte(3)?;
                out.unsigned(u64::from(*input))?;
            }
            ScalarKernelLaw::SquareRoot { input } => {
                out.byte(4)?;
                out.unsigned(u64::from(*input))?;
            }
        }
        out.number(&record.lower)?;
        out.number(&record.upper)?;
    }
    Ok(())
}

/// Decode a bounded child-before-parent record inventory for independent semantic replay.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling decision wire dispatcher calls the scalar decoder"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    objects: &[Decoded],
) -> Result<ScalarKernelCertificate, Fault> {
    let version = reader.small()?;
    if !matches!(version, 1..=4) {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    let expression = object(reader, objects)?;
    let context = object(reader, objects)?;
    let input_family = object(reader, objects)?;
    let count = reader.count(BudgetKind::ConditionAtoms, 1, 640)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let law = match reader.byte()? {
            7 if version == 4 => ScalarKernelLaw::Reading(Box::new(ScalarKernelReading {
                definition: object(reader, objects)?,
                input: reader.small()?,
                scale: reader.rational()?,
                offset: reader.rational()?,
            })),
            6 if version >= 3 => ScalarKernelLaw::Family(Box::new(decode_family(reader, objects)?)),
            5 if version >= 2 => ScalarKernelLaw::Input(Box::new(decode_input(reader)?)),
            5..=7 => return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT)),
            0 => ScalarKernelLaw::Rational(reader.rational()?),
            1 => ScalarKernelLaw::Sum {
                left: reader.small()?,
                right: reader.small()?,
            },
            2 => ScalarKernelLaw::Product {
                left: reader.small()?,
                right: reader.small()?,
            },
            3 => ScalarKernelLaw::Reciprocal {
                input: reader.small()?,
            },
            4 => ScalarKernelLaw::SquareRoot {
                input: reader.small()?,
            },
            _ => return Err(invalid()),
        };
        records.push(ScalarKernelRecord {
            law,
            lower: reader.rational()?,
            upper: reader.rational()?,
        });
    }
    Ok(ScalarKernelCertificate {
        version,
        expression,
        context,
        input_family,
        records,
    })
}

/// Encode the exact native set and closed input source without a live authority claim.
fn encode_input(value: &ScalarKernelInput, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.center)?;
    out.number(&value.radius)?;
    match &value.value {
        ScalarKernelInputValue::Rational(value) => {
            out.byte(0)?;
            out.number(value)
        }
        ScalarKernelInputValue::PiMultiple {
            factor,
            certificate,
        } => {
            out.byte(1)?;
            out.number(factor)?;
            out.unsigned(u64::from(certificate.version))?;
            for count in certificate.terms {
                out.unsigned(u64::from(count))?;
            }
            for number in [
                &certificate.sums[0],
                &certificate.sums[1],
                &certificate.lower,
                &certificate.upper,
            ] {
                out.number(number)?;
            }
            Ok(())
        }
    }
}

/// Decode one bounded input source; nested origins are deliberately absent.
fn decode_input(reader: &mut Reader<'_, '_>) -> Result<ScalarKernelInput, Fault> {
    let center = reader.rational()?;
    let radius = reader.rational()?;
    let value = match reader.byte()? {
        0 => ScalarKernelInputValue::Rational(reader.rational()?),
        1 => ScalarKernelInputValue::PiMultiple {
            factor: reader.rational()?,
            certificate: Box::new(crate::pi_bounds::PiBoundCertificate {
                version: reader.small()?,
                terms: [reader.small()?, reader.small()?],
                sums: [reader.rational()?, reader.rational()?],
                lower: reader.rational()?,
                upper: reader.rational()?,
            }),
        },
        _ => return Err(invalid()),
    };
    Ok(ScalarKernelInput {
        center,
        radius,
        value,
    })
}

/// Add only the new source-box law; existing law encodings retain their exact bytes.
fn encode_family(
    value: &ScalarKernelFamilyInput,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for number in [
        &value.center,
        &value.radius,
        &value.family.lower,
        &value.family.upper,
    ] {
        out.number(number)?;
    }
    write(&value.family.source, out)?;
    for premise in &value.family.premises {
        write(&premise.context, out)?;
        out.unsigned(u64::from(premise.slot))?;
    }
    Ok(())
}

/// Pure source identity and qualified assumptions require independent checking after decoding.
fn decode_family(
    reader: &mut Reader<'_, '_>,
    objects: &[Decoded],
) -> Result<ScalarKernelFamilyInput, Fault> {
    let center = reader.rational()?;
    let radius = reader.rational()?;
    let lower = reader.rational()?;
    let upper = reader.rational()?;
    let source = object(reader, objects)?;
    let first = crate::domain::HypothesisRef {
        context: object(reader, objects)?,
        slot: reader.small()?,
    };
    let second = crate::domain::HypothesisRef {
        context: object(reader, objects)?,
        slot: reader.small()?,
    };
    Ok(ScalarKernelFamilyInput {
        center,
        radius,
        family: ScalarKernelFamily {
            source,
            lower,
            upper,
            premises: [first, second],
        },
    })
}
