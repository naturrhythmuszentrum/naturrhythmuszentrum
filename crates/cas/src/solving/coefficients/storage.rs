// SPDX-License-Identifier: AGPL-3.0-only
//! Checked scratch reservations and graph payload access.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Handle, State, size};
use crate::solving::{CoefficientDomainData, CoefficientValueObject, PolynomialData, SolveDraft};

/// Shared location for coefficient scratch allocations.
pub(crate) const AT: Location = Location::new("exact coefficients");

/// A malformed coefficient or incompatible algebra never becomes arithmetic evidence.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

/// Count every attempted coefficient operation, including recursively called operations.
pub(crate) fn begin(budget: &mut Budget) -> Result<(), Fault> {
    budget.solving(BudgetKind::CoefficientOps, 1, AT)
}

/// Checked reservation arithmetic uses the same named capacity fault as allocation ceilings.
pub(crate) fn add(left: u64, right: u64) -> Result<u64, Fault> {
    left.checked_add(right)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
}

/// Checked product used before allocating scratch vectors.
pub(crate) fn product(left: u64, right: u64) -> Result<u64, Fault> {
    left.checked_mul(right)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
}

/// Allocate a bounded vector before copying or computing any entry.
pub(crate) fn vector<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>, Fault> {
    let length = size(count, AT)?;
    let bytes = product(length, size(std::mem::size_of::<T>(), AT)?)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(length, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// Grow scratch storage only after checking its complete resulting capacity bound.
pub(crate) fn reserve<T>(
    value: &mut Vec<T>,
    additional: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = add(size(value.len(), AT)?, size(additional, AT)?)?;
    let bytes = product(count, size(std::mem::size_of::<T>(), AT)?)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(size(additional, AT)?, AT)?;
    value
        .try_reserve_exact(additional)
        .map_err(|_| Fault::allocation(AT))
}

/// Copy a fixed-width variable frame or exponent vector after reserving it.
pub(crate) fn copied<T: Clone>(value: &[T], budget: &mut Budget) -> Result<Vec<T>, Fault> {
    let mut result = vector(value.len(), budget)?;
    result.extend_from_slice(value);
    Ok(result)
}

/// Borrow a domain without inferring any field theorem for quotients.
pub(crate) fn domain<'a>(
    state: &'a State,
    handle: &Handle,
) -> Result<&'a CoefficientDomainData<Handle>, Fault> {
    match super::super::graph::object(state, handle.logical_id())? {
        SolveDraft::CoefficientDomain(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow an admitted canonical coefficient value.
pub(crate) fn value<'a>(
    state: &'a State,
    handle: &Handle,
) -> Result<&'a CoefficientValueObject<Handle>, Fault> {
    match super::super::graph::object(state, handle.logical_id())? {
        SolveDraft::CoefficientValue(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a polynomial in its ordered formal variable frame.
pub(crate) fn polynomial<'a>(
    state: &'a State,
    handle: &Handle,
) -> Result<&'a PolynomialData<Handle>, Fault> {
    match super::super::graph::object(state, handle.logical_id())? {
        SolveDraft::Polynomial(value) => Ok(value),
        _ => Err(invalid()),
    }
}
