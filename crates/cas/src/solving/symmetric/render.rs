// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit mapped polynomial expressions preserve the complete expansion in proof syntax.

use super::AT;
use super::algebra::Polynomial;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::{MapTarget, SolveDraft};

/// Explicit scalar interpretation, checked again by the coefficient replay schema.
#[derive(Debug)]
pub(crate) struct Rendering {
    /// Header of every variable and coefficient expression.
    pub(crate) header: Handle,
    /// Scalar interpretation of the coefficient algebra.
    pub(crate) field: Field,
    /// Graph-owned homomorphism, with any required existence and denominator premises.
    pub(crate) map: Handle,
    /// Source algebra; implicit coefficient casts are rejected.
    pub(crate) domain: Handle,
    /// The exact region of every emitted equality.
    pub(crate) region: Handle,
}

/// One bounded scalar admission into the caller's private candidate.
fn admit(cas: &mut Cas, draft: &Draft, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(std::slice::from_ref(draft), budget)?
        .into_iter()
        .next()
        .ok_or_else(invalid)
}

impl Rendering {
    /// Accept an explicit scalar-target map, never invent an embedding of a quotient into C.
    pub(crate) fn new(cas: &Cas, map: &Handle, budget: &mut Budget) -> Result<Self, Fault> {
        budget.work(1, AT)?;
        let SolveDraft::CoefficientMap(data) =
            crate::solving::graph::object(&cas.state, cas.checked(map)?)?
        else {
            return Err(invalid());
        };
        let MapTarget::ScalarAmbient { header, field } = &data.target else {
            return Err(invalid());
        };
        if !matches!(field, Field::Rational | Field::Real | Field::Complex) {
            return Err(invalid());
        }
        Ok(Self {
            header: header.clone(),
            field: field.clone(),
            map: map.clone(),
            domain: data.source.clone(),
            region: data.region.clone(),
        })
    }

    /// Scope and sorts are checked before any expression is proposed for replay.
    fn variables(
        &self,
        cas: &Cas,
        polynomial: &Polynomial,
        images: &[Handle],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if polynomial.domain != self.domain || polynomial.variables.len() != images.len() {
            return Err(invalid());
        }
        for (field, image) in polynomial.variables.iter().zip(images) {
            budget.work(1, AT)?;
            let expression = cas.state.expression(cas.checked(image)?)?;
            if (
                field,
                expression.header,
                &expression.sort,
                expression.scope.is_empty(),
                expression.space.is_none(),
            ) != (
                &self.field,
                self.header.logical_id(),
                &Sort::Value(AtomicSort::Scalar(self.field.clone())),
                true,
                true,
            ) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// Arithmetic remains visible as ordinary expression nodes for independent normalization.
    pub(crate) fn operation(
        &self,
        cas: &mut Cas,
        operator: Operator,
        operands: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut arguments = vector(operands.len(), budget)?;
        arguments.extend(operands.iter().cloned().map(Reference::Existing));
        admit(
            cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(self.field.clone())),
                operands: arguments,
                scope: Vec::new(),
                space: None,
                binding: None,
            }),
            budget,
        )
    }

    /// A coefficient literal retains its exact value and explicit scalar map.
    fn coefficient(
        &self,
        cas: &mut Cas,
        element: &Element,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if element.domain != self.domain {
            return Err(invalid());
        }
        let value = crate::solving::towers::materialize::value(cas, element, budget)?;
        admit(
            cas,
            &Draft::ExactCoefficient {
                header: Reference::Existing(self.header.clone()),
                value: Reference::Existing(value),
                map: Reference::Existing(self.map.clone()),
            },
            budget,
        )
    }

    /// Render a polynomial by simultaneous replacement of every formal slot.
    pub(crate) fn polynomial(
        &self,
        cas: &mut Cas,
        polynomial: &Polynomial,
        images: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.variables(cas, polynomial, images, budget)?;
        let mut result = None;
        for term in &polynomial.form.terms {
            let mut product = self.coefficient(cas, &term.coefficient, budget)?;
            for (variable, exponent) in &term.powers {
                let Variable::Slot(slot) = variable else {
                    return Err(invalid());
                };
                let image = images
                    .get(usize::try_from(*slot).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                let power = if *exponent == 1 {
                    image.clone()
                } else {
                    self.operation(
                        cas,
                        Operator::IntegerPower(i64::from(*exponent)),
                        std::slice::from_ref(image),
                        budget,
                    )?
                };
                product = self.operation(cas, Operator::Mul, &[product, power], budget)?;
            }
            result = Some(match result {
                Some(previous) => {
                    self.operation(cas, Operator::Add, &[previous, product], budget)?
                }
                None => product,
            });
        }
        if let Some(result) = result {
            return Ok(result);
        }
        let engine = Engine {
            state: &cas.state,
            domain: &self.domain,
        };
        let zero = engine
            .constant(false, budget)
            .map_err(|error| fault(&error))?;
        self.coefficient(cas, &zero, budget)
    }
}
