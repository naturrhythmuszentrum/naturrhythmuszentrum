// SPDX-License-Identifier: AGPL-3.0-only
//! Original declarations and typed references precede every zero coefficient elimination.

use super::{CoordinateNormalization, ReadingError, ReciprocalMapDefinition, support};
use crate::Role;
use nrz_cas::{
    Cas, ExactConstant, Handle, Rational, Reference,
    limits::Budget,
    readings::ExactAffineMap,
    syntax::{AtomicSort, Draft, Field, Operator, Sort},
};

/// All formal inputs are scalar declarations in the exact reference context.
pub(super) fn check(
    cas: &Cas,
    frame: &CoordinateNormalization,
    proposed: &ReciprocalMapDefinition,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    let count = frame.entries().len();
    let [rows, columns] = proposed.shape;
    if count == 0 || rows == 0 || columns == 0 {
        return Err(ReadingError::Shape);
    }
    let outputs = rows.checked_mul(columns).ok_or(ReadingError::Shape)?;
    if proposed.inputs.len() != count
        || usize::try_from(proposed.map.inputs()).map_err(|_| ReadingError::Shape)? != count
        || proposed.map.outputs() != outputs
        || proposed.original.inputs() != columns
        || proposed.original.outputs() != rows
    {
        return Err(ReadingError::Shape);
    }
    for (index, (node, entry)) in proposed.inputs.iter().zip(frame.entries()).enumerate() {
        let (sort, operator, _, header) = support::inspect(cas, node, budget)?;
        if sort != real()
            || !matches!(operator, Operator::Free(_))
            || header != cas.context_header(frame.context())?
        {
            return Err(ReadingError::Definition);
        }
        if entry.descriptor().role() != Role::Difference {
            return Err(ReadingError::Role);
        }
        for previous in proposed.inputs.iter().take(index) {
            budget.work(1, support::AT)?;
            if previous == node {
                return Err(ReadingError::Definition);
            }
        }
    }
    Ok(())
}

/// Construct reciprocal temporaries once and preserve them beside the explicit result image.
pub(super) fn image(
    cas: &mut Cas,
    frame: &CoordinateNormalization,
    proposed: &ReciprocalMapDefinition,
    budget: &mut Budget,
) -> Result<(Vec<Handle>, ExactAffineMap), ReadingError> {
    let context = frame.context();
    let mut reciprocals = support::reserve(proposed.inputs.len(), budget)?;
    for (input, entry) in proposed.inputs.iter().zip(frame.entries()) {
        let numerator = constant(cas, context, entry.magnitude(), budget)?;
        reciprocals.push(term(
            cas,
            context,
            Operator::Div,
            &[numerator, input.clone()],
            budget,
        )?);
    }
    let [rows, columns] = proposed.shape;
    let mut coefficients = support::reserve(proposed.map.offsets().len(), budget)?;
    if rows != 0 && columns != 0 {
        for row in 0..rows {
            for column in 0..columns {
                let index = usize::try_from(u64::from(column) * u64::from(rows) + u64::from(row))
                    .map_err(|_| ReadingError::Shape)?;
                coefficients.push(contraction(
                    cas,
                    context,
                    proposed,
                    &reciprocals,
                    index,
                    budget,
                )?);
            }
        }
    }
    let zero = term(
        cas,
        context,
        Operator::Rational(Rational::zero()),
        &[],
        budget,
    )?;
    let mut offsets = support::reserve(
        usize::try_from(rows).map_err(|_| ReadingError::Shape)?,
        budget,
    )?;
    offsets.resize(
        usize::try_from(rows).map_err(|_| ReadingError::Shape)?,
        zero,
    );
    let image = cas.exact_reading_map(context, (columns, rows), &coefficients, &offsets, budget)?;
    Ok((reciprocals, image))
}

/// The map's rows name logical column-ordered coefficient entries, never native matrix storage order.
fn contraction(
    cas: &mut Cas,
    context: &Handle,
    proposed: &ReciprocalMapDefinition,
    reciprocals: &[Handle],
    index: usize,
    budget: &mut Budget,
) -> Result<Handle, ReadingError> {
    let mut sum = term(
        cas,
        context,
        Operator::Rational(proposed.map.offsets()[index].clone()),
        &[],
        budget,
    )?;
    for (column, input) in reciprocals.iter().enumerate() {
        budget.work(1, support::AT)?;
        let coefficient = &proposed.map.coefficients()[index * reciprocals.len() + column];
        if !coefficient.is_zero() {
            let factor = term(
                cas,
                context,
                Operator::Rational(coefficient.clone()),
                &[],
                budget,
            )?;
            let product = term(
                cas,
                context,
                Operator::Mul,
                &[factor, input.clone()],
                budget,
            )?;
            sum = term(cas, context, Operator::Add, &[sum, product], budget)?;
        }
    }
    Ok(sum)
}

/// Exact source-free constants retain their native expressions, including unsupported numerical exports.
fn constant(
    cas: &mut Cas,
    context: &Handle,
    value: &ExactConstant,
    budget: &mut Budget,
) -> Result<Handle, ReadingError> {
    let header = cas.context_header(context)?;
    let drafts = value
        .expression_drafts(Reference::Existing(header), budget)?
        .into_iter()
        .map(Draft::Expression)
        .collect::<Vec<_>>();
    cas.admit(&drafts, budget)?
        .pop()
        .ok_or(ReadingError::Definition)
}

/// Every scalar term uses the original declaration frame and Real sort.
fn term(
    cas: &mut Cas,
    context: &Handle,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, ReadingError> {
    support::expression(cas, context, operator, real(), operands, budget)
}

/// Formal recipe coordinates are Real even when a later represented result has Complex meaning.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
