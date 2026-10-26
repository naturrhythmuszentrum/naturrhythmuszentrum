// SPDX-License-Identifier: AGPL-3.0-only
//! Exact symbolic range conjunctions with explicit count-domain intersection.

use std::collections::BTreeMap;

use super::{AT, invalid};
use crate::budget::Budget;
use crate::decision::IntegerFamily;
use crate::decision_cut::{affine, evaluate::Value, family::conjunction::Atom, number_bytes};
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, size};
use crate::number::Rational;

/// Original affine operands remain the authority for every symbolic parameter endpoint.
type Values = BTreeMap<LogicalId, Value>;

/// Relation and projected input domain use the same exact endpoint and count constructors.
pub(super) fn expected(
    cas: &Cas,
    family: &IntegerFamily,
    dimensions: usize,
    values: &Values,
    relation: bool,
    budget: &mut Budget,
) -> Result<Vec<Atom>, Fault> {
    let q = basis(dimensions, 1, budget)?;
    let n = basis(dimensions, 2, budget)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(6)
        .map_err(|_| Fault::allocation(AT))?;
    if relation {
        insert(
            &mut result,
            affine::combine(&q, &n, true, budget)?,
            false,
            budget,
        )?;
        let mut upper = affine::combine(&n, &q, true, budget)?;
        *upper.first_mut().ok_or_else(invalid)? = Rational::one();
        insert(&mut result, upper, true, budget)?;
    }
    let subject = if relation { &n } else { &q };
    for (strict, handle) in [(false, &family.lower), (true, &family.upper)] {
        if let Some(handle) = handle {
            let endpoint = affine::get(values, cas.checked(handle)?)?;
            range(&mut result, subject, endpoint, strict, budget)?;
        }
    }
    if let Some(count) = &family.count {
        for (strict, endpoint) in [(false, &count.lower), (true, &count.upper)] {
            if let Some(number) = endpoint {
                let mut value = zeros(dimensions, budget)?;
                *value.first_mut().ok_or_else(invalid)? = number.add(&Rational::zero(), budget)?;
                range(&mut result, subject, &value, strict, budget)?;
            }
        }
    }
    Ok(result)
}

/// Reserve retained coefficient copies from actual inputs and one arithmetic workspace.
pub(super) fn reservation(
    cas: &Cas,
    family: &IntegerFamily,
    dimensions: usize,
    values: &Values,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let mut bytes = size(dimensions, AT)?
        .saturating_add(1)
        .saturating_mul(2_048)
        .saturating_add(1_024);
    for endpoint in [&family.lower, &family.upper].into_iter().flatten() {
        for coefficient in affine::get(values, cas.checked(endpoint)?)? {
            budget.work(1, AT)?;
            bytes = bytes.saturating_add(number_bytes(coefficient).saturating_mul(4));
        }
    }
    if let Some(count) = &family.count {
        for number in [&count.lower, &count.upper].into_iter().flatten() {
            budget.work(1, AT)?;
            bytes = bytes.saturating_add(number_bytes(number).saturating_mul(4));
        }
    }
    Ok(bytes.saturating_add(crate::decision_cut::temporary(dimensions, budget)?))
}

/// Inclusive lower and exclusive upper inequalities share the original coefficient basis.
fn range(
    result: &mut Vec<Atom>,
    subject: &[Rational],
    endpoint: &[Rational],
    strict: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let coefficients = if strict {
        affine::combine(endpoint, subject, true, budget)?
    } else {
        affine::combine(subject, endpoint, true, budget)?
    };
    insert(result, coefficients, strict, budget)
}

/// Equal repeated count and range bounds do not introduce a second mathematical obligation.
fn insert(
    result: &mut Vec<Atom>,
    coefficients: Vec<Rational>,
    strict: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for atom in result.iter() {
        budget.work(1, AT)?;
        let mut equal = atom.strict == strict && atom.coefficients.len() == coefficients.len();
        for (left, right) in atom.coefficients.iter().zip(&coefficients) {
            budget.work(number_bytes(left).saturating_add(number_bytes(right)), AT)?;
            equal &= left == right;
        }
        if equal {
            return Ok(());
        }
    }
    result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    result.push(Atom {
        strict,
        coefficients,
    });
    Ok(())
}

/// Prepaid vectors have one constant and one coefficient per explicit coordinate.
fn zeros(dimensions: usize, budget: &mut Budget) -> Result<Vec<Rational>, Fault> {
    let count = dimensions.checked_add(1).ok_or_else(invalid)?;
    budget.work(size(count, AT)?, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    result.resize_with(count, Rational::zero);
    Ok(result)
}

/// The q and n basis vectors have distinct declared roles.
fn basis(dimensions: usize, index: usize, budget: &mut Budget) -> Result<Vec<Rational>, Fault> {
    let mut result = zeros(dimensions, budget)?;
    *result.get_mut(index).ok_or_else(invalid)? = Rational::one();
    Ok(result)
}

/// Project already checked range atoms by renaming their bound index coefficient to q.
pub(super) fn project(atoms: &mut Vec<Atom>, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(size(atoms.len(), AT)?.saturating_mul(64), AT)?;
    if atoms.len() < 2 {
        return Err(invalid());
    }
    atoms.drain(..2);
    for atom in atoms {
        budget.work(128, AT)?;
        let Some([q, n]) = atom.coefficients.get_mut(1..3) else {
            return Err(invalid());
        };
        if !q.is_zero() {
            return Err(invalid());
        }
        std::mem::swap(q, n);
    }
    Ok(())
}
