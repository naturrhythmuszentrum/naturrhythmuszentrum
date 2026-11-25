// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-order object calculus, with independently reconstructed integer units.

use super::check::{AT, coefficient, defined, equal, invalid, number, unsupported};
use super::{SeriesCertificate as C, SeriesDerivativeRow, SeriesIntegralRow, SeriesStatement as S};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;
use crate::proof_model::CheckedApplication;

/// Bound the integer embedding before constructing its rational components.
pub(super) fn integer(value: u32, budget: &mut Budget) -> Result<Rational, Fault> {
    let bits = u64::from(value.bit_width());
    budget.check(BudgetKind::ComponentBits, bits, AT)?;
    budget.check(BudgetKind::IntermediateBits, bits, AT)?;
    budget.work(16, AT)?;
    Rational::from_ratio(i64::from(value), 1, budget)
}

/// Reconstruct prescribed order changes and every arithmetic row without invoking production.
pub(super) fn check(
    cas: &Cas,
    statement: &S,
    certificate: &C,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    match (statement, certificate) {
        (S::Derive { input, output }, C::Derive { rows, .. }) => {
            if !premises.is_empty() {
                return Err(invalid());
            }
            derivative(cas, input, output, rows, budget)
        }
        (
            S::Integrate {
                input,
                constant,
                output,
            },
            C::Integrate { rows, .. },
        ) => {
            defined(
                cas,
                constant,
                cas.series_object(input, budget)?,
                premises,
                budget,
            )?;
            integral(cas, input, constant, output, rows, budget)
        }
        _ => Err(invalid()),
    }
}

/// An order-one input has unavailable precision, not a fabricated zero derivative.
fn derivative(
    cas: &Cas,
    input: &Handle,
    output: &Handle,
    rows: &[SeriesDerivativeRow],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let input = cas.series_object(input, budget)?;
    let output = cas.series_object(output, budget)?;
    let order = input
        .order
        .checked_sub(1)
        .filter(|n| *n > 0)
        .ok_or_else(unsupported)?;
    output.preflight(budget)?;
    if output.order != order || size(rows.len(), AT)? != u64::from(order) {
        return Err(invalid());
    }
    for (index, row) in rows.iter().enumerate() {
        budget.work(1, AT)?;
        let index = u32::try_from(index).map_err(|_| invalid())?;
        if row.index != index {
            return Err(invalid());
        }
        let next = index.checked_add(1).ok_or_else(invalid)?;
        let multiplier = integer(next, budget)?;
        let value = coefficient(cas, input, next, budget)?;
        equal(&row.input, value, budget)?;
        equal(&row.multiplier, &multiplier, budget)?;
        equal(&row.result, &multiplier.mul(value, budget)?, budget)?;
        equal(
            &row.result,
            coefficient(cas, output, index, budget)?,
            budget,
        )?;
    }
    Ok(())
}

/// Each positive integer and inverse is checked even when the original coefficient is zero.
fn integral(
    cas: &Cas,
    input: &Handle,
    constant: &Handle,
    output: &Handle,
    rows: &[SeriesIntegralRow],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let input = cas.series_object(input, budget)?;
    let output = cas.series_object(output, budget)?;
    let order = input.order.checked_add(1).ok_or_else(unsupported)?;
    output.preflight(budget)?;
    if output.order != order || size(rows.len(), AT)? != u64::from(input.order) {
        return Err(invalid());
    }
    equal(
        number(cas, constant, budget)?,
        coefficient(cas, output, 0, budget)?,
        budget,
    )?;
    for (index, row) in rows.iter().enumerate() {
        budget.work(1, AT)?;
        let index = u32::try_from(index).map_err(|_| invalid())?;
        if row.index != index {
            return Err(invalid());
        }
        let next = index.checked_add(1).ok_or_else(invalid)?;
        let divisor = integer(next, budget)?;
        equal(&row.integer, &divisor, budget)?;
        equal(
            &divisor.mul(&row.inverse, budget)?,
            &Rational::one(),
            budget,
        )?;
        let value = coefficient(cas, input, index, budget)?;
        equal(&row.input, value, budget)?;
        equal(&row.result, &value.mul(&row.inverse, budget)?, budget)?;
        equal(&row.result, coefficient(cas, output, next, budget)?, budget)?;
    }
    Ok(())
}

/// Produce a derivative under the enclosing prepaid vector and arithmetic workspace.
pub(super) fn derive(
    cas: &mut Cas,
    input: &Handle,
    order: u32,
    budget: &mut Budget,
) -> Result<(S, C), Fault> {
    use super::produce::{copied, entry, literal, output};
    let count = usize::try_from(order).map_err(|_| invalid())?;
    let mut rows = Vec::new();
    let mut values = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for index in 0..order {
        let multiplier = integer(index.checked_add(1).ok_or_else(invalid)?, budget)?;
        let value = entry(cas, input, index + 1, budget)?;
        let result = value.mul(&multiplier, budget)?;
        rows.push(SeriesDerivativeRow {
            index,
            input: copied(value, budget)?,
            multiplier,
            result: copied(&result, budget)?,
        });
        values.push(literal(cas, input, result, budget)?);
    }
    let output = output(cas, input, values, budget)?;
    Ok((
        S::Derive {
            input: input.clone(),
            output,
        },
        C::Derive { form: 0, rows },
    ))
}

/// Propose exact reciprocals; only the independent checker grants them unit authority.
pub(super) fn integrate(
    cas: &mut Cas,
    input: &Handle,
    constant: &Handle,
    order: u32,
    budget: &mut Budget,
) -> Result<(S, C), Fault> {
    use super::produce::{copied, entry, literal, output};
    let count = usize::try_from(order).map_err(|_| invalid())?;
    let mut rows = Vec::new();
    let mut values = Vec::new();
    rows.try_reserve_exact(count - 1)
        .map_err(|_| Fault::allocation(AT))?;
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let initial = copied(number(cas, constant, budget)?, budget)?;
    values.push(literal(cas, input, initial, budget)?);
    for index in 0..order - 1 {
        let integer = integer(index.checked_add(1).ok_or_else(invalid)?, budget)?;
        let inverse = Rational::one().div(&integer, budget)?;
        let value = entry(cas, input, index, budget)?;
        let result = value.mul(&inverse, budget)?;
        rows.push(SeriesIntegralRow {
            index,
            integer,
            inverse,
            input: copied(value, budget)?,
            result: copied(&result, budget)?,
        });
        values.push(literal(cas, input, result, budget)?);
    }
    let output = output(cas, input, values, budget)?;
    Ok((
        S::Integrate {
            input: input.clone(),
            constant: constant.clone(),
            output,
        },
        C::Integrate { form: 0, rows },
    ))
}
