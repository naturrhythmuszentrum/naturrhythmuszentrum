// SPDX-License-Identifier: AGPL-3.0-only
//! Exact floor-family references participate in ordinary statement replay.

use crate::decision::IntegerFamily;
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
    for endpoint in [&value.lower, &value.upper] {
        out.byte(u8::from(endpoint.is_some()))?;
        if let Some(handle) = endpoint {
            write(handle, out)?;
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
    Ok(IntegerFamily {
        function: object(reader, records)?,
        coordinate: object(reader, records)?,
        index: object(reader, records)?,
        domain: object(reader, records)?,
        lower: endpoint(reader, records)?,
        upper: endpoint(reader, records)?,
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
    result
}
