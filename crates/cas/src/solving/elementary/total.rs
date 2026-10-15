// SPDX-License-Identifier: AGPL-3.0-only
//! Definedness needed before a scalar admits a total classical partition.

use super::proof::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::{SolveDraft, SolveStatement};

/// Every strict operation has a totality check; an arbitrary root predicate is insufficient.
pub(crate) fn check(
    cas: &Cas,
    expression: LogicalId,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(expression);
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let value = cas.state.expression(id)?;
        match (&value.operator, &value.attachment) {
            (
                Operator::Rational(_)
                | Operator::Free(_)
                | Operator::Bound { .. }
                | Operator::Neg
                | Operator::Add
                | Operator::Sub
                | Operator::Mul,
                None,
            ) => {}
            (Operator::IntegerPower(exponent), None) if *exponent > 0 => {}
            (Operator::RootValue { .. }, Some(ExpressionAttachment::SelectedRoot(selection))) => {
                existence(*selection, region, premises, budget)?;
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) => {
                let SolveDraft::CoefficientMap(mapping) =
                    crate::solving::graph::object(&cas.state, *map)?
                else {
                    return Err(invalid());
                };
                if mapping.region != *region {
                    return Err(invalid());
                }
                crate::solving::coefficients::proof::identity_map(cas, mapping, premises, budget)?;
                crate::solving::coefficients::proof::defined(
                    cas,
                    &cas.handle(*value),
                    region,
                    premises,
                    budget,
                )?;
            }
            _ => {
                explicit(cas, id, region, premises, budget)?;
            }
        }
        reserve(&mut pending, value.operands.len(), budget)?;
        pending.extend(&value.operands);
    }
    Ok(())
}

/// Only an exact domain or definedness predicate can justify unsupported partial syntax.
fn explicit(
    cas: &Cas,
    id: LogicalId,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in premises {
        budget.work(1, AT)?;
        match &premise.judgment.statement {
            Statement::DefinedDomain { function, domain }
                if (function.logical_id(), domain) == (id, region) =>
            {
                return Ok(());
            }
            Statement::PredicateFact {
                region: domain,
                predicate,
            } if domain == region => {
                if let (crate::model::PredicateKind::Defined, [argument]) =
                    super::evidence::predicate(cas, predicate.logical_id())?
                    && *argument == id
                {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
    Err(invalid())
}

/// Root existence stays attached to the complete selection and exact coefficient region.
fn existence(
    selection: LogicalId,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::RootExistence(value) = statement.as_ref()
            && (value.relation.logical_id(), &value.region) == (selection, region)
        {
            return Ok(());
        }
    }
    Err(invalid())
}
