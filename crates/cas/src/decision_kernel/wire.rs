// SPDX-License-Identifier: AGPL-3.0-only
//! Additive versioned scalar-origin encoding; existing origins keep their bytes.

use super::model::{ScalarKernelCertificate, ScalarKernelLaw, ScalarKernelRecord};
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
    if version != 1 {
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
