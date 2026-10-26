// SPDX-License-Identifier: AGPL-3.0-only
//! Exact floor-family references participate in ordinary statement replay.

use crate::decision::{IntegerCellRange, IntegerFamily};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Every endpoint is a graph dependency; absence has its own exact constructor.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement encoding, decoding and dependency publication share these private roles"
)]
pub(crate) fn encode(
    value: &IntegerFamily,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for handle in [
        &value.function,
        &value.coordinate,
        &value.index,
        &value.domain,
    ] {
        write(handle, out)?;
    }
    let flags = u8::from(value.lower.is_some()) | (u8::from(value.count.is_some()) << 1);
    out.byte(flags)?;
    if let Some(lower) = &value.lower {
        write(lower, out)?;
    }
    out.byte(u8::from(value.upper.is_some()))?;
    if let Some(upper) = &value.upper {
        write(upper, out)?;
    }
    if let Some(count) = &value.count {
        write(&count.coordinate, out)?;
        for bound in [&count.lower, &count.upper] {
            out.byte(u8::from(bound.is_some()))?;
            if let Some(number) = bound {
                out.number(number)?;
            }
        }
    }
    Ok(())
}

/// Unknown endpoint constructors refuse before mathematical checking.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement encoding, decoding and dependency publication share these private roles"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<IntegerFamily, Fault> {
    let function = object(reader, records)?;
    let coordinate = object(reader, records)?;
    let index = object(reader, records)?;
    let domain = object(reader, records)?;
    let flags = reader.byte()?;
    if flags > 3 {
        return Err(invalid());
    }
    let lower = if flags & 1 == 1 {
        Some(object(reader, records)?)
    } else {
        None
    };
    let upper = endpoint(reader, records)?;
    let count = if flags & 2 == 2 {
        Some(Box::new(IntegerCellRange {
            coordinate: object(reader, records)?,
            lower: bound(reader)?,
            upper: bound(reader)?,
        }))
    } else {
        None
    };
    Ok(IntegerFamily {
        function,
        coordinate,
        index,
        domain,
        lower,
        upper,
        count,
    })
}

/// Read an exact optional bound without an infinity value.
fn endpoint(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Option<Handle>, Fault> {
    match reader.byte()? {
        0 => Ok(None),
        1 => Ok(Some(object(reader, records)?)),
        _ => Err(invalid()),
    }
}

/// Count-domain endpoints are exact optional rationals, not machine infinities.
fn bound(reader: &mut Reader<'_, '_>) -> Result<Option<crate::number::Rational>, Fault> {
    if reader.boolean()? {
        Ok(Some(reader.rational()?))
    } else {
        Ok(None)
    }
}

/// Statement publication retains all six possible roles in stable order.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement encoding, decoding and dependency publication share these private roles"
)]
pub(crate) fn objects(value: &IntegerFamily) -> Vec<&Handle> {
    let mut result = vec![
        &value.function,
        &value.coordinate,
        &value.index,
        &value.domain,
    ];
    result.extend(value.lower.iter());
    result.extend(value.upper.iter());
    result.extend(value.count.iter().map(|count| &count.coordinate));
    result
}
