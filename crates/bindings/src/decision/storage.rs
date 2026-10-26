// SPDX-License-Identifier: AGPL-3.0-only
//! Conservative retained metadata and exact comparison charges on the caller ledger.

use super::DecisionError;
use nrz_cas::{
    ExactConstant, Location, Rational,
    limits::{Budget, BudgetKind},
};

/// Shared accounting location for storage-neutral decision checks.
pub(crate) const AT: Location = Location::new("bindings Integer decision");

/// Exact limb storage includes both rational components and fixed metadata.
pub(crate) fn rational_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_add(64)
}

/// A retained normalized graph bounds its structural comparison key conservatively.
pub(crate) fn constant_bytes(value: &ExactConstant) -> u64 {
    value.as_rational().map_or_else(
        || {
            value.graph().map_or(128, |(cas, _)| {
                cas.counts().payload_bytes.saturating_add(128)
            })
        },
        rational_bytes,
    )
}

/// Bound an owned result before allocation; shared declaration graphs are charged by their owners.
pub(crate) fn retained(bytes: u64, budget: &Budget) -> Result<(), DecisionError> {
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    Ok(())
}

/// Retain the established conservative native-witness envelope without exporting a second copy.
pub(crate) fn witness_bytes(value: &nrz_cas::proof::VerifiedWitness) -> Result<u64, DecisionError> {
    u64::try_from(value.exported_byte_len())
        .map_err(|_| DecisionError::Capacity)?
        .checked_mul(8)
        .and_then(|size| size.checked_add(1024))
        .ok_or(DecisionError::Capacity)
}
