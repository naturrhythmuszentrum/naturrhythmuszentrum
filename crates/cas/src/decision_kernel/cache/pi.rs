// SPDX-License-Identifier: AGPL-3.0-only
//! Checked Machin judgments keyed by their complete borrowed canonical byte streams.

use super::{accounting, records};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, size};
use crate::pi_bounds::PiBoundCertificate;
use crate::proof::CONSTANT_SCHEMAS;

/// Each retained certificate comparison has a separate small Work charge.
const AT: Location = Location::new("checked Machin certificate");

/// Owning the canonical fields keeps the complete byte key without a duplicate encoding buffer.
#[derive(Debug)]
pub(super) struct Entry {
    /// Only the independent finite certificate checker can establish these fields.
    certificate: PiBoundCertificate,
    /// Existing logical certificate ownership returned by the complete checker.
    bytes: u64,
}

/// Cite a byte-identical checked theorem or run the unchanged full finite certificate checker.
pub(crate) fn validate(
    cas: &Cas,
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
    retain(cas, certificate, bytes, budget)?;
    Ok(bytes)
}

/// Byte iterators inspect every semantic field, including declared bounds and hidden sum digits.
fn lookup(
    cas: &Cas,
    certificate: &PiBoundCertificate,
    budget: &mut Budget,
) -> Result<Option<u64>, Fault> {
    let cache = records(cas)?;
    let count = size(cache.pi.len(), AT)?;
    budget.work(certificate.key_bytes(budget)?.saturating_mul(count), AT)?;
    for entry in &cache.pi {
        if certificate
            .canonical_key()?
            .eq(entry.certificate.canonical_key()?)
        {
            return Ok(Some(entry.bytes));
        }
    }
    drop(cache);
    Ok(None)
}

/// The ordinary complete validation allowance covers retaining the already checked fields.
fn retain(
    cas: &Cas,
    certificate: &PiBoundCertificate,
    bytes: u64,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut cache = records(cas)?;
    let total = cache.bytes.saturating_add(bytes).saturating_add(256);
    budget.check(
        BudgetKind::PayloadBytes,
        cas.counts().payload_bytes.saturating_add(total),
        AT,
    )?;
    budget.check(
        BudgetKind::Vertices,
        size(cache.pi.len(), AT)?
            .saturating_add(
                size(cas.state.scalar_judgments.len(), AT)?
                    .saturating_add(size(cache.pending.len(), AT)?),
            )
            .saturating_add(1),
        AT,
    )?;
    accounting::retain(
        cas,
        total.saturating_add(cas.retained_judgment_bytes()),
        budget,
    )?;
    cache
        .pi
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    cache.pi.push(Entry {
        certificate: certificate.clone(),
        bytes,
    });
    cache.bytes = total;
    drop(cache);
    Ok(())
}

#[cfg(test)]
mod tests;
