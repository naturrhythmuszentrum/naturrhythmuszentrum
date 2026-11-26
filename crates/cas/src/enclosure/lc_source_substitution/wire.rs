// SPDX-License-Identifier: AGPL-3.0-only
//! Portable original syntax; receiving stores rebuild it from their own expected request.

use super::{AT, Request, Tag};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::wire_read::Reader;
use crate::{Cas, Fault, InvalidCode};

/// A proposal that disagrees with receiving original syntax is invalid.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Build a canonical statement from the receiver's own original syntax.
pub(super) fn scope(cas: &Cas, request: &Request, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let count = request.handles().count();
    budget.check(BudgetKind::MatrixEntries, count as u64, AT)?;
    let mut live = (count as u64).saturating_mul(size_of::<Vec<u8>>() as u64);
    budget.check(BudgetKind::StagedPayloadBytes, live, AT)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for handle in request.handles() {
        let key = budget.with_retained_payload(live, AT, |budget| {
            crate::wire_build::syntax_key(cas, cas.checked(handle, budget)?, budget)
        })?;
        live = live.saturating_add(key.capacity() as u64);
        budget.check(BudgetKind::StagedPayloadBytes, live, AT)?;
        keys.push(key);
    }
    budget.with_retained_payload(live, AT, |budget| {
        let mut out = Encoder::witness(budget, AT);
        out.raw(b"NRZLCS01")?;
        out.unsigned(u64::from(request.revision))?;
        out.byte(match request.tag {
            Tag::Box => 0,
            Tag::MappedCover => 1,
        })?;
        out.count(request.sources.len())?;
        out.count(request.constraints.len())?;
        out.count(request.substitution.len())?;
        for key in &keys {
            out.count(key.len())?;
            out.raw(key)?;
        }
        Ok(out.finish())
    })
}

/// Encode the complete source statement and registered root transcript.
pub(super) fn encode(scope: &[u8], root: &[u8], budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(budget, AT);
    out.raw(b"NRZLSS01")?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.count(root.len())?;
    out.raw(root)?;
    Ok(out.finish())
}

/// Decode only bounded spans; the caller independently checks both original scopes.
pub(super) fn decode<'a>(
    bytes: &'a [u8],
    expected_scope: &[u8],
    budget: &mut Budget,
) -> Result<&'a [u8], Fault> {
    let mut reader = Reader::new(bytes, budget)?;
    if reader.take(8)? != b"NRZLSS01" {
        return Err(invalid());
    }
    let scope_len = reader.index()? as usize;
    if reader.take(scope_len)? != expected_scope {
        return Err(invalid());
    }
    let root_len = reader.index()? as usize;
    let root = reader.take(root_len)?;
    reader.finished()?;
    Ok(root)
}
