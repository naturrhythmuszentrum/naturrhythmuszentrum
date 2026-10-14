// SPDX-License-Identifier: AGPL-3.0-only
//! Fallible typed constructors shared by distribution proposals.

use crate::budget::Budget;
use crate::distribution_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::number::Rational;

/// Construct one expression with the exact caller-selected header and space.
pub(crate) fn node(
    cas: &mut Cas,
    header: &Handle,
    space: &Handle,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let sort = match operator {
        Operator::TestDerivative { .. } | Operator::TestMultiply => Sort::Test,
        ref operator
            if crate::distribution_syntax::scalar_action(operator)
                || crate::distribution_syntax::action_arithmetic(operator) =>
        {
            Sort::Value(AtomicSort::Scalar(Field::Real))
        }
        _ => Sort::Distribution,
    };
    crate::case_syntax::admit(
        cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            space: Some(Reference::Existing(space.clone())),
            scope: Vec::new(),
            sort,
            operator,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
            binding: None,
        }),
        budget,
    )
}

/// Pointwise rational coefficients carry no distribution space.
pub(crate) fn rational(
    cas: &mut Cas,
    header: &Handle,
    value: &Rational,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    crate::case_syntax::admit(
        cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            space: None,
            scope: Vec::new(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operator: Operator::Rational(value.clone()),
            operands: Vec::new(),
            binding: None,
        }),
        budget,
    )
}

/// Materialize a structural coefficient map in its portable key order.
pub(crate) fn linear(
    cas: &mut Cas,
    header: &Handle,
    space: &Handle,
    terms: &crate::distribution_linear::Terms,
    scalar: bool,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut result = None;
    for (term, coefficient) in terms.values() {
        let value = match term {
            None if scalar => rational(cas, header, coefficient, budget)?,
            Some(term) if *coefficient == Rational::one() => term.clone(),
            Some(term) => {
                let factor = rational(cas, header, coefficient, budget)?;
                let operator = if scalar {
                    Operator::Mul
                } else {
                    Operator::DistributionScale
                };
                node(
                    cas,
                    header,
                    space,
                    operator,
                    &[factor, term.clone()],
                    budget,
                )?
            }
            None => return Err(invalid()),
        };
        result = Some(match result {
            None => value,
            Some(previous) => node(
                cas,
                header,
                space,
                if scalar {
                    Operator::Add
                } else {
                    Operator::DistributionAdd
                },
                &[previous, value],
                budget,
            )?,
        });
    }
    match result {
        Some(value) => Ok(value),
        None if scalar => rational(cas, header, &Rational::zero(), budget),
        None => node(cas, header, space, Operator::DistributionZero, &[], budget),
    }
}
