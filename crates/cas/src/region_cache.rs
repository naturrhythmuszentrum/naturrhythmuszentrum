// SPDX-License-Identifier: AGPL-3.0-only
//! Ledger-local exact partitions keyed by complete portable region syntax.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::number::Rational;
use crate::region::RegionCertificate;

/// Cache accounting is independent of the certificate's claimed truth.
const AT: Location = Location::new("region partition memo");

/// Only independently computed successful partitions enter this ledger-owned cache.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    /// Exact keys, policy ceilings and reconstructed values.
    entries: Vec<Entry>,
    /// Conservative retained allocation bound, also reserved by every staged-payload check.
    pub(crate) bytes: u64,
}

/// A value is reusable only under the same resource ceilings.
#[derive(Debug)]
struct Entry {
    /// Ordered, length-delimited portable roots and the optional exact cut.
    key: Vec<u8>,
    /// Ceilings used for the original reconstruction.
    limits: Limits,
    /// Reconstructed cuts and membership, never copied from an untrusted certificate.
    value: RegionCertificate,
    /// Bound for copying the retained value.
    bytes: u64,
}

/// Reuse a decision across receiving stores without using admission serials as identity.
pub(crate) fn partition_at(
    cas: &Cas,
    roots: &[LogicalId],
    boundary: Option<&Rational>,
    budget: &mut Budget,
) -> Result<RegionCertificate, Fault> {
    let key = structural_key(cas, roots, boundary, budget)?;
    if let Some(value) = lookup(&key, budget)? {
        crate::region_check::measure(&value, budget)?;
        return Ok(value);
    }
    let value = crate::region::partition_at(&cas.state, roots, boundary, budget)?;
    retain(key, &value, budget)?;
    Ok(value)
}

/// Ordinary decisions have an explicitly absent extra boundary cut in their key.
pub(crate) fn partition(
    cas: &Cas,
    roots: &[LogicalId],
    budget: &mut Budget,
) -> Result<RegionCertificate, Fault> {
    partition_at(cas, roots, None, budget)
}

/// Check connected openness using the same cached exact membership decision.
pub(crate) fn open_interval(cas: &Cas, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    crate::region::check_open(&partition(cas, &[root], budget)?)
}

/// Every region key includes its header, ambient coordinate, scope and full dependency syntax.
fn structural_key(
    cas: &Cas,
    roots: &[LogicalId],
    boundary: Option<&Rational>,
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::new(budget, AT);
    out.count(roots.len())?;
    let mut bytes = out.finish();
    for root in roots {
        cas.state.region(*root)?;
        let syntax = crate::wire_build::syntax_key(cas, *root, budget)?;
        let mut out = Encoder::new(budget, AT);
        out.raw(&bytes)?;
        out.count(syntax.len())?;
        out.raw(&syntax)?;
        bytes = out.finish();
    }
    let mut out = Encoder::new(budget, AT);
    out.raw(&bytes)?;
    out.byte(u8::from(boundary.is_some()))?;
    if let Some(boundary) = boundary {
        out.number(boundary)?;
    }
    Ok(out.finish())
}

/// Lookup and copying consume work and remain subject to all active storage ceilings.
fn lookup(key: &[u8], budget: &mut Budget) -> Result<Option<RegionCertificate>, Fault> {
    budget.check(BudgetKind::StagedPayloadBytes, 0, AT)?;
    let count = size(budget.region_partitions.entries.len(), AT)?;
    let work = size(key.len(), AT)?
        .checked_add(512)
        .and_then(|bytes| bytes.checked_mul(count))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
    budget.work(work, AT)?;
    let found = budget
        .region_partitions
        .entries
        .iter()
        .position(|entry| entry.key == key && entry.limits == *budget.limits());
    let Some(index) = found else {
        return Ok(None);
    };
    let bytes = budget
        .region_partitions
        .entries
        .get(index)
        .ok_or_else(invalid)?
        .bytes;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(Some(
        budget
            .region_partitions
            .entries
            .get(index)
            .ok_or_else(invalid)?
            .value
            .clone(),
    ))
}

/// Reserve cache storage before copying; failed or unsupported decisions are never inserted.
fn retain(key: Vec<u8>, value: &RegionCertificate, budget: &mut Budget) -> Result<(), Fault> {
    let bytes = footprint(&key, value, budget)?;
    let count = size(budget.region_partitions.entries.len(), AT)?
        .checked_add(1)
        .ok_or_else(invalid)?;
    budget.check(BudgetKind::Vertices, count, AT)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        bytes.checked_mul(2).ok_or_else(invalid)?,
        AT,
    )?;
    let work = count
        .checked_mul(1024)
        .and_then(|growth| growth.checked_add(bytes))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
    budget.work(work, AT)?;
    let retained = budget
        .region_partitions
        .bytes
        .checked_add(bytes)
        .ok_or_else(invalid)?;
    let limits = budget.limits().clone();
    budget
        .region_partitions
        .entries
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    budget.region_partitions.entries.push(Entry {
        key,
        limits,
        value: value.clone(),
        bytes,
    });
    budget.region_partitions.bytes = retained;
    Ok(())
}

/// Include vector storage, policy fields and conservative bigint allocation overhead.
fn footprint(key: &[u8], value: &RegionCertificate, budget: &mut Budget) -> Result<u64, Fault> {
    let mut bytes = size(key.len(), AT)?.checked_add(1024).ok_or_else(invalid)?;
    for cut in &value.cuts {
        let numeric = size(cut.structural_bytes(budget)?.len(), AT)?
            .checked_mul(4)
            .and_then(|amount| amount.checked_add(128))
            .ok_or_else(invalid)?;
        bytes = bytes.checked_add(numeric).ok_or_else(invalid)?;
    }
    for row in &value.membership {
        bytes = bytes
            .checked_add(size(row.len(), AT)?)
            .and_then(|amount| amount.checked_add(64))
            .ok_or_else(invalid)?;
    }
    Ok(bytes)
}

/// Internal index arithmetic cannot manufacture an accepted decision.
const fn invalid() -> Fault {
    Fault::capacity(BudgetKind::StagedPayloadBytes, AT)
}
