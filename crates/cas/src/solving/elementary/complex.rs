// SPDX-License-Identifier: AGPL-3.0-only
//! Exact real and imaginary expression induction for standard complex conjugation.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, LogicalId};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::{CoefficientValueData, SolveDraft};

/// Formal real components preserve independent root choices as independent variables.
pub(crate) struct Parts {
    /// Exact real component.
    pub(crate) real: Value,
    /// Exact imaginary component.
    pub(crate) imaginary: Value,
}

impl Parts {
    /// Rational real and imaginary constants embed through the standard fixed subfield.
    fn rational(
        evaluation: &Evaluation<'_>,
        real: &crate::number::Rational,
        imaginary: &crate::number::Rational,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let coefficient = |value, budget: &mut Budget| {
            let value = evaluation
                .engine
                .rational(value, budget)
                .map_err(|e| fault(&e))?;
            Value::polynomial(
                Form::constant(value, &evaluation.engine, budget).map_err(|e| fault(&e))?,
                &evaluation.engine,
                budget,
            )
        };
        Ok(Self {
            real: coefficient(real, budget)?,
            imaginary: coefficient(imaginary, budget)?,
        })
    }

    /// Componentwise addition and subtraction are exact over the underlying real polynomial ring.
    fn sum(
        &self,
        other: &Self,
        subtract: bool,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if subtract {
            Ok(Self {
                real: self.real.sub(&other.real, &evaluation.engine, budget)?,
                imaginary: self
                    .imaginary
                    .sub(&other.imaginary, &evaluation.engine, budget)?,
            })
        } else {
            Ok(Self {
                real: self.real.add(&other.real, &evaluation.engine, budget)?,
                imaginary: self
                    .imaginary
                    .add(&other.imaginary, &evaluation.engine, budget)?,
            })
        }
    }

    /// Multiplication uses i²=-1, without identifying any unrelated algebraic choices.
    fn mul(
        &self,
        other: &Self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let real = self
            .real
            .mul(&other.real, &evaluation.engine, budget)?
            .sub(
                &self
                    .imaginary
                    .mul(&other.imaginary, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        let imaginary = self
            .real
            .mul(&other.imaginary, &evaluation.engine, budget)?
            .add(
                &self
                    .imaginary
                    .mul(&other.real, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        Ok(Self { real, imaginary })
    }

    /// Conjugation negates exactly the imaginary component.
    pub(crate) fn conjugate(
        &self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        Ok(Self {
            real: self.real.copy(&evaluation.engine, budget)?,
            imaginary: self.imaginary.neg(&evaluation.engine, budget)?,
        })
    }

    /// Division requires the exact norm denominator to be nonzero before cancellation.
    fn div(
        &self,
        other: &Self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let norm = other
            .real
            .mul(&other.real, &evaluation.engine, budget)?
            .add(
                &other
                    .imaginary
                    .mul(&other.imaginary, &evaluation.engine, budget)?,
                &evaluation.engine,
                budget,
            )?;
        evaluation.nonzero(&norm, budget)?;
        let numerator = self.mul(&other.conjugate(evaluation, budget)?, evaluation, budget)?;
        Ok(Self {
            real: numerator.real.div(&norm, &evaluation.engine, budget)?,
            imaginary: numerator.imaginary.div(&norm, &evaluation.engine, budget)?,
        })
    }

    /// Pair equality is a finite exact identity, not a comparison of approximations.
    pub(crate) fn equal(
        &self,
        other: &Self,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        evaluation.zero(
            &self.real.sub(&other.real, &evaluation.engine, budget)?,
            budget,
        )?;
        evaluation.zero(
            &self
                .imaginary
                .sub(&other.imaginary, &evaluation.engine, budget)?,
            budget,
        )
    }

    /// Nonnegative powers are bounded repeated multiplication, independent of degree-specific formulas.
    fn power(
        &self,
        exponent: i64,
        evaluation: &Evaluation<'_>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if exponent <= 0 {
            return Err(invalid());
        }
        let mut remaining = u64::try_from(exponent).map_err(|_| invalid())?;
        budget.check(crate::budget::BudgetKind::IntegerExponent, remaining, AT)?;
        let mut result = Self {
            real: Value::integer(1, &evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        };
        let mut factor = Self {
            real: self.real.copy(&evaluation.engine, budget)?,
            imaginary: self.imaginary.copy(&evaluation.engine, budget)?,
        };
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&factor, evaluation, budget)?;
            }
            remaining >>= 1;
            if remaining > 0 {
                factor = factor.mul(&factor, evaluation, budget)?;
            }
        }
        Ok(result)
    }
}

/// Fresh component slots cannot coincide with any original declaration used in regional evidence.
fn atom(evaluation: &Evaluation<'_>, id: LogicalId, budget: &mut Budget) -> Result<Parts, Fault> {
    let expression = evaluation.cas.state.expression(id)?;
    let offset = u64::try_from(
        evaluation
            .cas
            .state
            .header(expression.header)?
            .environment
            .declarations
            .len(),
    )
    .map_err(|_| invalid())?;
    let first = id
        .serial
        .checked_mul(2)
        .and_then(|index| index.checked_add(offset))
        .ok_or_else(invalid)?;
    let real = Value::variable(
        Variable::Slot(u32::try_from(first).map_err(|_| invalid())?),
        &evaluation.engine,
        budget,
    )?;
    let imaginary = if expression.sort == Sort::Value(AtomicSort::Scalar(Field::Complex)) {
        Value::variable(
            Variable::Slot(
                u32::try_from(first.checked_add(1).ok_or_else(invalid)?).map_err(|_| invalid())?,
            ),
            &evaluation.engine,
            budget,
        )?
    } else {
        Value::integer(0, &evaluation.engine, budget)?
    };
    Ok(Parts { real, imaginary })
}

/// Literal Gaussian parts are canonical; general mapped extensions need their own certified images.
fn leaf(
    evaluation: &Evaluation<'_>,
    id: LogicalId,
    parameters: &[crate::graph::Handle],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    let expression = evaluation.cas.state.expression(id)?;
    match (&expression.operator, &expression.attachment) {
        (Operator::Rational(value), None) => {
            Parts::rational(evaluation, value, &crate::number::Rational::zero(), budget)
        }
        (Operator::Free(key), None) if real_parameter(evaluation, key, parameters, budget)? => {
            Ok(Parts {
                real: evaluation.expression(id, true, budget)?,
                imaginary: Value::integer(0, &evaluation.engine, budget)?,
            })
        }
        (Operator::Free(_) | Operator::Bound { .. }, None) => atom(evaluation, id, budget),
        (Operator::RootValue { .. }, Some(ExpressionAttachment::SelectedRoot(selected))) => {
            evaluation.existence(*selected, budget)?;
            atom(evaluation, id, budget)
        }
        (
            Operator::ExactCoefficient,
            Some(ExpressionAttachment::ExactCoefficient { value, map }),
        ) => {
            let SolveDraft::CoefficientMap(mapping) =
                crate::solving::graph::object(&evaluation.cas.state, *map)?
            else {
                return Err(invalid());
            };
            crate::solving::coefficients::proof::identity_map(
                evaluation.cas,
                mapping,
                evaluation.premises,
                budget,
            )?;
            if mapping.region != *evaluation.region {
                return Err(invalid());
            }
            let stored = crate::solving::coefficients::storage::value(
                &evaluation.cas.state,
                &evaluation.cas.handle(*value),
            )?;
            match &stored.value {
                CoefficientValueData::Rational { value } => {
                    Parts::rational(evaluation, value, &crate::number::Rational::zero(), budget)
                }
                CoefficientValueData::Gaussian { real, imaginary } => {
                    Parts::rational(evaluation, real, imaginary, budget)
                }
                _ => Err(invalid()),
            }
        }
        _ => Err(invalid()),
    }
}

/// All supported operations follow exact complex field laws and the named conjugation involution.
fn operation(
    evaluation: &Evaluation<'_>,
    id: LogicalId,
    values: &[Parts],
    parameters: &[crate::graph::Handle],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    match (&evaluation.cas.state.expression(id)?.operator, values) {
        (Operator::Add, [left, right]) => left.sum(right, false, evaluation, budget),
        (Operator::Sub, [left, right]) => left.sum(right, true, evaluation, budget),
        (Operator::Mul, [left, right]) => left.mul(right, evaluation, budget),
        (Operator::Div, [left, right]) => left.div(right, evaluation, budget),
        (Operator::Neg, [value]) => Ok(Parts {
            real: value.real.neg(&evaluation.engine, budget)?,
            imaginary: value.imaginary.neg(&evaluation.engine, budget)?,
        }),
        (Operator::Conjugate, [value]) => value.conjugate(evaluation, budget),
        (Operator::RealPart, [value]) => Ok(Parts {
            real: value.real.copy(&evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        }),
        (Operator::ImagPart, [value]) => Ok(Parts {
            real: value.imaginary.copy(&evaluation.engine, budget)?,
            imaginary: Value::integer(0, &evaluation.engine, budget)?,
        }),
        (Operator::IntegerPower(power), [value]) => value.power(*power, evaluation, budget),
        (_, []) => leaf(evaluation, id, parameters, budget),
        _ => Err(invalid()),
    }
}

/// Only a caller's explicit real-parameter transport supplies this fixed-subfield fact.
fn real_parameter(
    evaluation: &Evaluation<'_>,
    key: &str,
    parameters: &[crate::graph::Handle],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for handle in parameters {
        budget.work(1, AT)?;
        if matches!(&evaluation.cas.state.expression(handle.logical_id())?.operator, Operator::Free(name) if name == key)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Ordinary conjugation has no additional cross-header parameter assumptions.
pub(crate) fn expression(
    evaluation: &Evaluation<'_>,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    restricted(evaluation, root, &[], budget)
}

/// Iterative component normalization is bounded by ordinary expression depth and charged storage.
pub(crate) fn restricted(
    evaluation: &Evaluation<'_>,
    root: LogicalId,
    parameters: &[crate::graph::Handle],
    budget: &mut Budget,
) -> Result<Parts, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push((root, false));
    let mut values = vector(1, budget)?;
    while let Some((id, ready)) = pending.pop() {
        budget.work(1, AT)?;
        let expression = evaluation.cas.state.expression(id)?;
        if expression.header
            != evaluation
                .cas
                .state
                .region(evaluation.region.logical_id())?
                .header
        {
            return Err(invalid());
        }
        if !ready && !expression.operands.is_empty() {
            reserve(
                &mut pending,
                expression
                    .operands
                    .len()
                    .checked_add(1)
                    .ok_or_else(invalid)?,
                budget,
            )?;
            pending.push((id, true));
            pending.extend(
                expression
                    .operands
                    .iter()
                    .rev()
                    .map(|child| (*child, false)),
            );
        } else {
            let start = values
                .len()
                .checked_sub(expression.operands.len())
                .ok_or_else(invalid)?;
            let value = operation(
                evaluation,
                id,
                values.get(start..).ok_or_else(invalid)?,
                parameters,
                budget,
            )?;
            values.truncate(start);
            reserve(&mut values, 1, budget)?;
            values.push(value);
        }
    }
    if values.len() != 1 {
        return Err(invalid());
    }
    values.pop().ok_or_else(invalid)
}
