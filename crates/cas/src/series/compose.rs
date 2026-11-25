// SPDX-License-Identifier: AGPL-3.0-only
//! Zero-constant substitution in the identical rational increment coordinate.

use super::check::{AT, binary, coefficient, equal, invalid};
use super::{SeriesCertificate as C, SeriesPowerRow, SeriesProductRow, SeriesStatement as S};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;
use crate::proof_model::CheckedApplication;

/// Unavailable precision is distinct from a proved inadmissible inner constant.
pub(super) fn precision(
    outer: u32,
    inner: u32,
    requested: u32,
    budget: &mut Budget,
) -> Result<Fault, Fault> {
    budget.work(256, AT)?;
    budget.check(BudgetKind::OutputBytes, 256, AT)?;
    budget.with_retained_payload(256, AT, |_| {
        let mut fault = super::check::unsupported();
        fault.series_evidence = Some(std::sync::Arc::new(
            crate::error::SeriesRefusalEvidence::Precision([outer, inner, requested]),
        ));
        Ok(fault)
    })
}

/// Count all seed, power-pair and accumulation rows before allocating.
pub(super) fn entries(order: u32) -> Result<u64, Fault> {
    let n = u64::from(order);
    let pairs = n
        .checked_mul(n.checked_add(1).ok_or_else(invalid)?)
        .ok_or_else(invalid)?
        / 2;
    pairs
        .checked_mul(n.checked_sub(1).ok_or_else(invalid)?)
        .and_then(|v| v.checked_add(n.checked_mul(n)?))
        .and_then(|v| v.checked_add(n))
        .ok_or_else(invalid)
}

/// Reserve simultaneous powers and unreduced rational intermediates conservatively.
pub(super) fn workspace(order: u32, bits: u64) -> Result<u64, Fault> {
    let n = u64::from(order);
    entries(order)?
        .checked_mul(
            bits.checked_mul(n)
                .and_then(|v| v.checked_mul(n))
                .and_then(|v| v.checked_mul(64))
                .and_then(|v| v.checked_add(512))
                .ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)
}

/// Mathematical admission precedes inspection of a caller's claimed zero witness.
fn zero(cas: &Cas, inner: &Handle, budget: &mut Budget) -> Result<(), Fault> {
    let object = cas.series_object(inner, budget)?;
    let number = coefficient(cas, object, 0, budget)?;
    if number.compare(&Rational::zero(), budget)? != std::cmp::Ordering::Equal {
        let (a, b) = number.component_bits();
        let bytes = a
            .checked_add(b)
            .and_then(|v| v.checked_add(7))
            .map(|v| v / 8)
            // The additional attempt token owns two reference-count words.
            .and_then(|v| v.checked_add(256 + 2 * std::mem::size_of::<usize>() as u64))
            .ok_or_else(invalid)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        return budget.with_retained_payload(bytes, AT, |budget| {
            budget.work(bytes, AT)?;
            let value = super::produce::copied(number, budget)?;
            let [input, coefficient] =
                crate::error::SeriesDiagnosticIdentity::pair(inner, &object.coefficients[0]);
            let mut fault = Fault::invalid(InvalidCode::SeriesNonzeroConstant, AT);
            fault.series_evidence = Some(std::sync::Arc::new(
                crate::error::SeriesRefusalEvidence::Constant(
                    crate::error::SeriesConstantEvidence {
                        input,
                        coefficient,
                        index: 0,
                        value,
                    },
                ),
            ));
            Err(fault)
        });
    }
    Ok(())
}

/// Allocate only within the enclosing prepaid workspace.
fn vector<T>(count: u64) -> Result<Vec<T>, Fault> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(usize::try_from(count).map_err(|_| invalid())?)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(values)
}

/// Reconstruct powers independently; certificate values never seed the recurrence.
#[expect(
    clippy::too_many_lines,
    reason = "The independent complete power and accumulation schedule remains visible together"
)]
pub(super) fn check(
    cas: &Cas,
    statement: &S,
    certificate: &C,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let S::Compose {
        left,
        right,
        output,
        order,
        default_order,
    } = statement
    else {
        return Err(invalid());
    };
    let C::Compose {
        order: claimed,
        default_order: default,
        zero: witness,
        seed,
        powers,
        sums,
        ..
    } = certificate
    else {
        return Err(invalid());
    };
    let outer_order = cas.series_object(left, budget)?.order;
    let inner_order = cas.series_object(right, budget)?.order;
    if *order == 0 || *order > outer_order.min(inner_order) {
        return Err(precision(outer_order, inner_order, *order, budget)?);
    }
    let (outer, inner, result) = binary(
        cas,
        [left, right, output],
        *order,
        *default_order,
        premises,
        budget,
    )?;
    zero(cas, right, budget)?;
    equal(witness, coefficient(cas, inner, 0, budget)?, budget)?;
    equal(witness, &Rational::zero(), budget)?;
    let n = u64::from(*order);
    let count = entries(*order)?;
    budget.check(BudgetKind::JetEntries, count, AT)?;
    if order != claimed
        || default_order != default
        || size(seed.len(), AT)? != n
        || size(sums.len(), AT)? != n * n
        || size(powers.len(), AT)? != count - n * n - n
    {
        return Err(invalid());
    }
    let mut rebuilt = vector(n * n)?;
    for (index, value) in seed.iter().enumerate() {
        let expected = if index == 0 {
            Rational::one()
        } else {
            Rational::zero()
        };
        equal(value, &expected, budget)?;
        rebuilt.push(expected);
    }
    let width = usize::try_from(*order).map_err(|_| invalid())?;
    let mut rows = powers.iter();
    for exponent in 1..*order {
        for degree in 0..*order {
            let mut sum = Rational::zero();
            for i in 0..=degree {
                budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
                let supplied = rows.next().ok_or_else(invalid)?;
                let row = &supplied.row;
                if supplied.exponent != exponent
                    || (row.degree, row.left_index, row.right_index) != (degree, i, degree - i)
                {
                    return Err(invalid());
                }
                let a = rebuilt
                    .get(
                        (usize::try_from(exponent).map_err(|_| invalid())? - 1) * width
                            + usize::try_from(i).map_err(|_| invalid())?,
                    )
                    .ok_or_else(invalid)?;
                let b = coefficient(cas, inner, degree - i, budget)?;
                equal(&row.left, a, budget)?;
                equal(&row.right, b, budget)?;
                let product = a.mul(b, budget)?;
                equal(&row.product, &product, budget)?;
                sum = sum.add(&product, budget)?;
                equal(&row.sum, &sum, budget)?;
            }
            rebuilt.push(sum);
        }
    }
    let mut rows = sums.iter();
    for degree in 0..*order {
        let mut sum = Rational::zero();
        for exponent in 0..*order {
            budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
            let row = rows.next().ok_or_else(invalid)?;
            if (row.degree, row.left_index, row.right_index) != (degree, exponent, degree) {
                return Err(invalid());
            }
            let a = coefficient(cas, outer, exponent, budget)?;
            let b = rebuilt
                .get(
                    usize::try_from(exponent).map_err(|_| invalid())? * width
                        + usize::try_from(degree).map_err(|_| invalid())?,
                )
                .ok_or_else(invalid)?;
            equal(&row.left, a, budget)?;
            equal(&row.right, b, budget)?;
            let product = a.mul(b, budget)?;
            equal(&row.product, &product, budget)?;
            sum = sum.add(&product, budget)?;
            equal(&row.sum, &sum, budget)?;
        }
        equal(&sum, coefficient(cas, result, degree, budget)?, budget)?;
    }
    Ok(())
}

/// Candidate construction has no authority; publication always invokes independent checking.
pub(super) fn produce(
    cas: &mut Cas,
    left: &Handle,
    right: &Handle,
    order: u32,
    default_order: bool,
    budget: &mut Budget,
) -> Result<(S, C), Fault> {
    use super::produce::{copied, entry, literal, output};
    super::check::compatible(
        cas.series_object(left, budget)?,
        cas.series_object(right, budget)?,
        budget,
    )?;
    zero(cas, right, budget)?;
    let n = u64::from(order);
    let count = entries(order)?;
    let mut seed = vector(n)?;
    for index in 0..order {
        seed.push(if index == 0 {
            Rational::one()
        } else {
            Rational::zero()
        });
    }
    let mut table = vector(n * n)?;
    for value in &seed {
        table.push(copied(value, budget)?);
    }
    let width = usize::try_from(order).map_err(|_| invalid())?;
    let mut powers = vector(count - n * n - n)?;
    for exponent in 1..order {
        for degree in 0..order {
            let mut sum = Rational::zero();
            for index in 0..=degree {
                budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
                let a = table
                    .get(
                        (usize::try_from(exponent).map_err(|_| invalid())? - 1) * width
                            + usize::try_from(index).map_err(|_| invalid())?,
                    )
                    .ok_or_else(invalid)?;
                let b = entry(cas, right, degree - index, budget)?;
                let product = a.mul(b, budget)?;
                sum = sum.add(&product, budget)?;
                powers.push(SeriesPowerRow {
                    exponent,
                    row: SeriesProductRow {
                        degree,
                        left_index: index,
                        right_index: degree - index,
                        left: copied(a, budget)?,
                        right: copied(b, budget)?,
                        product,
                        sum: copied(&sum, budget)?,
                    },
                });
            }
            table.push(sum);
        }
    }
    let mut sums = vector(n * n)?;
    let mut values = vector(n)?;
    for degree in 0..order {
        let mut total = Rational::zero();
        for exponent in 0..order {
            budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
            let a = entry(cas, left, exponent, budget)?;
            let b = table
                .get(
                    usize::try_from(exponent).map_err(|_| invalid())? * width
                        + usize::try_from(degree).map_err(|_| invalid())?,
                )
                .ok_or_else(invalid)?;
            let product = a.mul(b, budget)?;
            total = total.add(&product, budget)?;
            sums.push(SeriesProductRow {
                degree,
                left_index: exponent,
                right_index: degree,
                left: copied(a, budget)?,
                right: copied(b, budget)?,
                product,
                sum: copied(&total, budget)?,
            });
        }
        values.push(literal(cas, left, total, budget)?);
    }
    let output = output(cas, left, values, budget)?;
    Ok((
        S::Compose {
            left: left.clone(),
            right: right.clone(),
            output,
            order,
            default_order,
        },
        C::Compose {
            form: 0,
            order,
            default_order,
            zero: Rational::zero(),
            seed,
            powers,
            sums,
        },
    ))
}
