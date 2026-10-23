// SPDX-License-Identifier: AGPL-3.0-only
//! Retain native original operands and actual component readings beside the closed coefficient check.

use super::super::{ReadingError, ReadingProjection, Representation, support};
use nrz_cas::syntax::{AtomicSort, Extent, Field, Operator, Sort};
use nrz_cas::{Cas, Handle, Rational, limits::Budget, readings::AffineMap};

/// Exact real and imaginary projections keep their complete original definitions and selected copies.
pub(super) fn components(
    cas: &mut Cas,
    operand: &Representation,
    budget: &mut Budget,
) -> Result<[ReadingProjection; 2], ReadingError> {
    Ok([
        operand.reading().project(cas, &[0], budget)?,
        operand.reading().project(cas, &[1], budget)?,
    ])
}

/// Preserve ordered native Complex multiplication and its ordered Real block operation.
pub(super) fn products(
    cas: &mut Cas,
    operands: &[Representation; 2],
    budget: &mut Budget,
) -> Result<[Handle; 2], ReadingError> {
    let context = operands[0].reading().family().context();
    let original = support::expression(
        cas,
        context,
        Operator::Mul,
        Sort::Value(AtomicSort::Scalar(Field::Complex)),
        &[
            operands[0].reading().node().clone(),
            operands[1].reading().node().clone(),
        ],
        budget,
    )?;
    let block = support::expression(
        cas,
        context,
        Operator::MatrixMultiply,
        matrix(),
        &[
            operands[0].reconstruction().clone(),
            operands[1].reconstruction().clone(),
        ],
        budget,
    )?;
    Ok([original, block])
}

/// Exact conjugation composition equals the complete transpose permutation on the original basis.
pub(super) fn conjugation(
    cas: &mut Cas,
    operand: &Representation,
    budget: &mut Budget,
) -> Result<super::PhasorConjugation, ReadingError> {
    let reading = operand.reading();
    let context = reading.family().context();
    let (_, _, operands, _) = support::inspect(cas, operand.reconstruction(), budget)?;
    let declaration = operands.first().ok_or(ReadingError::Definition)?;
    let conjugate = support::expression(
        cas,
        context,
        Operator::Conjugate,
        reading.sort().clone(),
        std::slice::from_ref(reading.node()),
        budget,
    )?;
    let reconstructed = cas.define_reading(declaration, &conjugate, operand.expansion(), budget)?;
    let transpose = support::expression(
        cas,
        context,
        Operator::Transpose,
        matrix(),
        std::slice::from_ref(reading.selected()),
        budget,
    )?;
    let conjugation = AffineMap::new(
        2,
        2,
        vec![
            Rational::one(),
            Rational::zero(),
            Rational::zero(),
            Rational::one().negated(budget)?,
        ],
        vec![Rational::zero(); 2],
        budget,
    )?;
    let coefficients = operand.expansion().compose(&conjugation, budget)?;
    let transposed = operand.expansion().project(&[0, 2, 1, 3], budget)?;
    if coefficients != transposed {
        return Err(ReadingError::Relation);
    }
    Ok(super::PhasorConjugation {
        operand: operand.clone(),
        endpoints: [reconstructed, transpose],
        coefficients,
    })
}

/// The block law has a fixed Real rectangle and logical column order.
const fn matrix() -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(2),
        columns: Extent::Literal(2),
    })
}

/// Every checked component equation retains native endpoints in logical column order.
pub(super) fn equations(
    cas: &mut Cas,
    context: &Handle,
    products: &[Handle; 2],
    operand: &Representation,
    budget: &mut Budget,
) -> Result<[[Handle; 2]; 4], ReadingError> {
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    let a = support::expression(
        cas,
        context,
        Operator::RealPart,
        real.clone(),
        std::slice::from_ref(&products[0]),
        budget,
    )?;
    let b = support::expression(
        cas,
        context,
        Operator::ImagPart,
        real.clone(),
        std::slice::from_ref(&products[0]),
        budget,
    )?;
    let negative = support::expression(
        cas,
        context,
        Operator::Neg,
        real,
        std::slice::from_ref(&b),
        budget,
    )?;
    let (_, _, operands, _) = support::inspect(cas, operand.reconstruction(), budget)?;
    let declaration = operands.first().ok_or(ReadingError::Definition)?;
    let defined = cas.define_reading(
        declaration,
        &products[1],
        &nrz_cas::readings::AffineMap::identity(4, budget)?,
        budget,
    )?;
    let mut equations = support::reserve(4, budget)?;
    for (index, component) in [a.clone(), b, negative, a].into_iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| ReadingError::Shape)?;
        let selected = cas.select_entries(
            &defined,
            &nrz_cas::readings::SelectionSpec::Entry(nrz_cas::readings::EntryIndex {
                row: index % 2,
                column: index / 2,
            }),
            budget,
        )?;
        equations.push([component, selected]);
    }
    equations.try_into().map_err(|_| ReadingError::Shape)
}
