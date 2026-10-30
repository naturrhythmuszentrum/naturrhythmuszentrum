// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical selected Integer propositions and original enclosure certificates.

use crate::budget::{Budget, BudgetKind};
use crate::decision::{EnclosureOrigin, IntegerSelection, RationalEnclosure};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, LogicalId, size};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::Reader;

/// Portable selected-index proof accounting.
const AT: Location = Location::new("integer selection wire");

/// The complete family and selected literal are statement identity, not certificate hints.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement dispatch shares the selection wire contract"
)]
pub(crate) fn encode(
    value: &IntegerSelection,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(27)?;
    crate::decision_family_wire::encode(&value.family, out, write)?;
    write(&value.value, out)
}

/// Decode the family before the selected result and recheck both during proof replay.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement decoding shares the selection wire contract"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<IntegerSelection, Fault> {
    Ok(IntegerSelection {
        family: crate::decision_family_wire::decode(reader, records)?,
        value: object(reader, records)?,
    })
}

/// Every original family operand and the selected Integer remains a graph dependency.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement publication shares the selection wire contract"
)]
pub(crate) fn objects(value: &IntegerSelection) -> Vec<&Handle> {
    let mut result = crate::decision_family_wire::objects(&value.family);
    result.push(&value.value);
    result
}

/// Version the enclosure consumer independently of the original closed enclosure encoding.
#[expect(
    clippy::redundant_pub_crate,
    reason = "certificate encoding shares the selection wire contract"
)]
pub(crate) fn certificate(
    value: &RationalEnclosure,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(17)?;
    out.unsigned(1)?;
    crate::decision_wire::encode_enclosure(value, out, write)
}

/// Unknown certificate versions refuse before any selection checker is invoked.
#[expect(
    clippy::redundant_pub_crate,
    reason = "certificate decoding shares the selection wire contract"
)]
pub(crate) fn decode_certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<RationalEnclosure, Fault> {
    if reader.unsigned()? != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    crate::decision_wire::decode_enclosure(reader, records)
}

/// Authenticate embedded handles and prepay copied certificate data before semantic checking.
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof preflight shares the selection wire contract"
)]
pub(crate) fn measure(
    cas: &Cas,
    value: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = Encoder::new(budget, AT);
    certificate(value, &mut out, &mut |handle, out| {
        cas.checked(handle)?;
        crate::statement::handle(handle, out)
    })?;
    let bytes = size(out.finish().len(), AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}

/// The original target and origin expression survive dependency collection and fresh-store replay.
#[expect(
    clippy::redundant_pub_crate,
    reason = "proof publication shares the selection wire contract"
)]
pub(crate) fn dependencies(
    cas: &Cas,
    value: &RationalEnclosure,
    edges: &mut Vec<(LogicalId, Edge)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(4, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(edges.len().saturating_add(4), AT)?.saturating_mul(32),
        AT,
    )?;
    edges
        .try_reserve_exact(4)
        .map_err(|_| Fault::allocation(AT))?;
    for handle in [&value.target, &value.context, &value.input_family] {
        edges.push((cas.checked(handle)?, Edge::DependsOn));
    }
    if let EnclosureOrigin::ExactRational { expression, .. }
    | EnclosureOrigin::PiBound { expression, .. } = &value.origin
    {
        edges.push((cas.checked(expression)?, Edge::DependsOn));
    } else if let EnclosureOrigin::ScalarKernel(certificate) = &value.origin {
        edges.push((cas.checked(&certificate.expression)?, Edge::DependsOn));
    }
    Ok(())
}
