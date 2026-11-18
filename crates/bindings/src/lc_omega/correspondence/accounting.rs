// SPDX-License-Identifier: AGPL-3.0-only
//! Disjoint bindings allocations; CAS retains its existing logical graph tariffs.

use super::{AT, Result};
use crate::AdmissionError;
use nrz_cas::{
    Cas, Rational,
    limits::{Budget, BudgetKind},
};

/// Inline store plus its two initial Arc allocations, from graph/identity and graph/intern.
pub(super) const STORE: u64 = size_of::<Cas>() as u64 + 384;

/// Checked container bytes including its vector header.
pub(super) fn vector<T>(capacity: usize) -> Result<u64> {
    capacity
        .checked_mul(size_of::<T>())
        .and_then(|n| n.checked_add(size_of::<Vec<T>>()))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(AdmissionError::ResourceLimit)
}

/// Existing LC rational storage envelope, excluding the inline slot already in its container.
pub(super) fn rational(value: &Rational) -> u64 {
    let (n, d) = value.component_bits();
    128_u64
        .saturating_add(n.div_ceil(8))
        .saturating_add(d.div_ceil(8))
        .saturating_sub(size_of::<Rational>() as u64)
}

/// Every fallible reserve prepays allocation and cleanup, including failed attempts.
pub(super) fn allocate<T>(capacity: usize, budget: &mut Budget) -> Result<Vec<T>> {
    budget.check(BudgetKind::StagedPayloadBytes, vector::<T>(capacity)?, AT)?;
    edges(2, budget)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| AdmissionError::ResourceLimit)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        vector::<T>(values.capacity())?,
        AT,
    )?;
    Ok(values)
}

/// Literal allocation and owning-edge bookkeeping belongs to one phase partition.
pub(super) fn edges(count: u64, budget: &mut Budget) -> Result<()> {
    budget.with_work_phase("LC omega correspondence", "edges", |budget| {
        budget.work(count, AT).map_err(Into::into)
    })
}

/// Scalar syntax visit, index comparison or copied name byte.
pub(super) fn expression(count: u64, budget: &mut Budget) -> Result<()> {
    budget.with_work_phase("LC omega correspondence", "expression", |budget| {
        budget.work(count, AT).map_err(Into::into)
    })
}

/// Original declaration identity and ordered expected-request visits.
pub(super) fn original(count: u64, budget: &mut Budget) -> Result<()> {
    budget.with_work_phase("LC omega correspondence", "original", |budget| {
        budget.work(count, AT).map_err(Into::into)
    })
}
