// SPDX-License-Identifier: AGPL-3.0-only
//! Checked Machin judgments are graph-owned; pending payloads live only inside an attempt.

use super::{accounting, records};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, LogicalId, PendingJudgment, PiInventory, size};
use crate::pi_bounds::PiBoundCertificate;
use crate::proof::CONSTANT_SCHEMAS;
use std::sync::Arc;

/// Complete borrowed canonical comparisons preserve the existing citation Work boundary.
const AT: Location = Location::new("checked Machin certificate");

/// Cite authenticated graph evidence or independently reconstruct the complete finite theorem.
pub(crate) fn validate(
    cas: &Cas,
    original: LogicalId,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let unsupported = certificate.version != CONSTANT_SCHEMAS[0].version;
    if unsupported || cas.memo_epoch == u64::MAX {
        return certificate.validate(budget);
    }
    if let Some(bytes) = lookup(cas, certificate, budget)? {
        return Ok(bytes);
    }
    let bytes = certificate.validate(budget)?;
    retain(cas, original, certificate, bytes, budget)?;
    Ok(bytes)
}

/// No warm encoding buffer is allocated; each inspected complete canonical key is prepaid.
fn lookup(
    cas: &Cas,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Option<u64>, Fault> {
    let bytes = certificate.key_bytes(budget)?;
    if cas.state.pi_judgments.current(cas) {
        let count = size(cas.state.pi_judgments.len(), AT)?;
        budget.work(bytes.saturating_mul(u64::from(count.bit_width())), AT)?;
        if let Some((id, key)) = cas.state.pi_judgments.entry(certificate)?
            && let Some(bytes) = cas.pi_judgment(id)?
        {
            let inventory = cas.pi_inventory(id)?;
            if !Arc::ptr_eq(key, &inventory.certificate) {
                return Err(Fault::invalid(crate::error::InvalidCode::Reference, AT));
            }
            return Ok(Some(bytes));
        }
    }
    let cache = records(cas)?;
    let count = cache
        .pending
        .iter()
        .filter(|entry| {
            matches!(
                entry,
                PendingJudgment::NewPi(_) | PendingJudgment::RecheckedPi(_)
            )
        })
        .count();
    budget.work(bytes.saturating_mul(size(count, AT)?), AT)?;
    for entry in &cache.pending {
        let inventory = match entry {
            PendingJudgment::NewPi(inventory) => inventory,
            PendingJudgment::RecheckedPi(id) => cas.pi_inventory(*id)?,
            PendingJudgment::NewScalar(_) | PendingJudgment::RecheckedScalar(_) => continue,
        };
        if certificate
            .canonical_key()?
            .eq(inventory.certificate.canonical_key()?)
        {
            return Ok(Some(inventory.bytes - 256));
        }
    }
    drop(cache);
    Ok(None)
}

/// The unchanged full-check allowance covers moving complete fields into staged graph ownership.
fn retain(
    cas: &Cas,
    original: LogicalId,
    certificate: &PiBoundCertificate,
    bytes: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    cas.state.expression(original)?;
    let _validated = certificate.canonical_key()?;
    if let Some((id, key)) = cas.state.pi_judgments.entry(certificate)? {
        let inventory = cas.pi_inventory(id)?;
        if !Arc::ptr_eq(key, &inventory.certificate) {
            return Err(Fault::invalid(crate::error::InvalidCode::Reference, AT));
        }
        let mut cache = records(cas)?;
        cache
            .pending
            .try_reserve_exact(1)
            .map_err(|_| Fault::allocation(AT))?;
        cache.pending.push(PendingJudgment::RecheckedPi(id));
        drop(cache);
        return Ok(());
    }
    let mut cache = records(cas)?;
    let bytes = bytes.saturating_add(256);
    let total = cache.bytes.saturating_add(bytes);
    budget.check(
        BudgetKind::PayloadBytes,
        cas.counts().payload_bytes.saturating_add(total),
        AT,
    )?;
    budget.check(
        BudgetKind::Vertices,
        size(cas.counts().vertices, AT)?
            .saturating_add(size(cache.pending.len(), AT)?)
            .saturating_add(1),
        AT,
    )?;
    accounting::retain(
        cas,
        total.saturating_add(cas.retained_judgment_bytes()),
        budget,
    )?;
    cache
        .pending
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    cache.pending.push(PendingJudgment::NewPi(PiInventory {
        certificate: Arc::new(certificate.clone()),
        original,
        bytes,
    }));
    cache.bytes = total;
    drop(cache);
    Ok(())
}

#[cfg(test)]
mod tests;
