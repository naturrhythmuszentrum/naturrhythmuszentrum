// SPDX-License-Identifier: AGPL-3.0-only
//! Exact nonrealness of roots whose real quadratic specialization is strictly positive.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle};
use crate::model::Operator;
use crate::solving::CoefficientValueData;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::storage::{polynomial, value};

/// A positive leading and constant coefficient make aX²+c strictly positive on the real subfield.
fn quadratic(
    evaluation: &Evaluation<'_>,
    relation: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let relation = super::selected::relation(evaluation.cas, relation)?;
    if &relation.region != evaluation.region {
        return Err(invalid());
    }
    let stored = polynomial(&evaluation.cas.state, &relation.polynomial)?;
    let [leading, constant] = stored.terms.as_slice() else {
        return Ok(false);
    };
    if (leading.exponents.as_slice(), constant.exponents.as_slice())
        != ([2].as_slice(), [0].as_slice())
    {
        return Ok(false);
    }
    for handle in [&leading.coefficient, &constant.coefficient] {
        if !positive(evaluation, handle, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Equality to either sign of a proved nonreal quadratic root preserves nonrealness.
pub(crate) fn check(
    evaluation: &Evaluation<'_>,
    entry: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let actual = evaluation.expression(entry.logical_id(), true, budget)?;
    for term in &actual.numerator.terms {
        for (variable, _) in &term.powers {
            budget.work(1, AT)?;
            let Variable::Atom(id) = variable else {
                continue;
            };
            let expression = evaluation.cas.state.expression(*id)?;
            let (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) =
                (&expression.operator, &expression.attachment)
            else {
                continue;
            };
            let selection =
                super::selected::selection(evaluation.cas, &evaluation.cas.handle(*selected))?;
            let relation = selection
                .relations
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            if !quadratic(evaluation, relation, budget)? {
                continue;
            }
            let root = Value::variable(*variable, &evaluation.engine, budget)?;
            if super::product::decides(
                evaluation.zero(&actual.sub(&root, &evaluation.engine, budget)?, budget),
            )? || super::product::decides(
                evaluation.zero(&actual.add(&root, &evaluation.engine, budget)?, budget),
            )? {
                return Ok(());
            }
        }
    }
    Err(invalid())
}

/// Constant tower embeddings preserve the exact sign of a rational real scalar.
fn positive(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut current = handle;
    loop {
        budget.work(1, AT)?;
        let coefficient = value(&evaluation.cas.state, current)?;
        match &coefficient.value {
            CoefficientValueData::Rational { value } => {
                return Ok(value.compare(&crate::number::Rational::zero(), budget)?
                    == std::cmp::Ordering::Greater);
            }
            CoefficientValueData::Tower { coefficients } if coefficients.len() == 1 => {
                current = coefficients.first().ok_or_else(invalid)?;
            }
            _ => return Ok(false),
        }
    }
}
