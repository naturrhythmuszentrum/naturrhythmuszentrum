// SPDX-License-Identifier: AGPL-3.0-only
//! Exact owned scratch inventories include nested coefficients, capacities and scalar limbs.

use super::super::{proof::AT, value::Value};
use crate::budget::Budget;
use crate::error::Fault;
use crate::solving::coefficients::{forms::Form, model, polynomial::Polynomial, scalar::Scalar};

/// Count the allocated vector capacity before traversing its owned child payload.
fn allocation<T>(values: &Vec<T>) -> Result<u64, Fault> {
    Ok(crate::graph::size(values.capacity(), AT)?
        .saturating_mul(crate::graph::size(std::mem::size_of::<T>(), AT)?))
}

/// Four limb copies plus a fixed bigint header conservatively cover each reduced rational.
const fn rational((numerator, denominator): (u64, u64)) -> u64 {
    numerator
        .div_ceil(64)
        .saturating_add(denominator.div_ceil(64))
        .saturating_mul(32)
        .saturating_add(128)
}

/// Include rational limb bounds for both components of a Gaussian scalar.
fn scalar(value: &Scalar, budget: &mut Budget) -> Result<u64, Fault> {
    budget.work(1, AT)?;
    Ok(match value {
        Scalar::Rational(value) => rational(value.component_bits()),
        Scalar::Gaussian(value) => value
            .component_bits()
            .into_iter()
            .map(rational)
            .fold(0, u64::saturating_add),
    })
}

/// Count every retained term, exponent capacity and exact scalar payload.
fn polynomial(value: &Polynomial, budget: &mut Budget) -> Result<u64, Fault> {
    let mut bytes = allocation(&value.terms)?;
    for term in &value.terms {
        budget.work(1, AT)?;
        bytes = bytes
            .saturating_add(allocation(&term.powers)?)
            .saturating_add(scalar(&term.coefficient, budget)?);
    }
    Ok(bytes)
}

/// Walk only the actual coefficient representation, including nested tower entries.
fn coefficient(value: &model::Value, budget: &mut Budget) -> Result<u64, Fault> {
    budget.work(1, AT)?;
    match value {
        model::Value::Scalar(value) => scalar(value, budget),
        model::Value::Fraction(value) => Ok(polynomial(&value.numerator, budget)?
            .saturating_add(polynomial(&value.denominator, budget)?)),
        model::Value::Tower(values) => {
            let mut bytes = allocation(values)?;
            for value in values {
                bytes = bytes.saturating_add(coefficient(&value.value, budget)?);
            }
            Ok(bytes)
        }
    }
}

/// Count sparse formal keys and their independently owned coefficient payloads.
fn form(value: &Form, budget: &mut Budget) -> Result<u64, Fault> {
    let mut bytes = allocation(&value.terms)?;
    for term in &value.terms {
        budget.work(1, AT)?;
        bytes = bytes
            .saturating_add(allocation(&term.powers)?)
            .saturating_add(coefficient(&term.coefficient.value, budget)?);
    }
    Ok(bytes)
}

/// Include the owning value slot and every nested owned allocation without cloning it.
pub(super) fn value(value: &Value, budget: &mut Budget) -> Result<u64, Fault> {
    budget.work(1, AT)?;
    Ok(crate::graph::size(std::mem::size_of::<Value>(), AT)?
        .saturating_add(allocation(&value.maps)?)
        .saturating_add(form(&value.numerator, budget)?)
        .saturating_add(form(&value.denominator, budget)?))
}
