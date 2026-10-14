// SPDX-License-Identifier: AGPL-3.0-only
//! Signed derivative action, linearity, and checked first-order delta increments.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::DistributionStatement;
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Distribution derivatives never project to classical scalar derivative statements.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if draft.certificate != Certificate::None {
        return Err(invalid());
    }
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    match value.as_ref() {
        DistributionStatement::Derivative {
            distribution,
            derivative,
            space,
            order,
        } => {
            if from_equality(cas, distribution, derivative, space, *order, premises)? {
                return Ok(());
            }
            if *order > 1 {
                return iterate(distribution, derivative, space, *order, premises);
            }
            crate::distribution_check::membership(
                cas,
                distribution.logical_id(),
                space,
                premises,
                budget,
            )?;
            let expected = expand(cas, distribution, space, premises, budget)?;
            crate::distribution_linear::equal_terms(cas, &expected, derivative, premises, budget)
        }
        DistributionStatement::Equality { left, right, space } => {
            equation(cas, left, right, space, premises)
        }
        DistributionStatement::Pairing {
            pairing,
            value,
            space,
        } => action(cas, pairing, value, space, premises, budget),
        _ => Err(invalid()),
    }
}

/// A derivative equation is merely the syntax-level view of an exact derivative theorem.
fn equation(
    cas: &Cas,
    left: &Handle,
    right: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let formal = cas.state.expression(left.logical_id())?;
    let Operator::DistributionDerivative { order } = formal.operator else {
        return Err(invalid());
    };
    let input = *formal.operands.first().ok_or_else(invalid)?;
    if premises.iter().any(|premise| matches!(&premise.judgment.statement,
        Statement::Distribution(value) if matches!(value.as_ref(),
            DistributionStatement::Derivative { distribution, derivative, space: actual, order: n }
            if distribution.logical_id() == input && derivative == right && actual == space && *n == order))) {
        Ok(())
    } else { Err(invalid()) }
}

/// A checked equality for formal derivative syntax transports its entire typed theorem.
fn from_equality(
    cas: &Cas,
    input: &Handle,
    output: &Handle,
    space: &Handle,
    order: u32,
    premises: &[CheckedApplication],
) -> Result<bool, Fault> {
    for premise in premises {
        let Statement::Distribution(value) = &premise.judgment.statement else {
            continue;
        };
        let DistributionStatement::Equality {
            left,
            right,
            space: actual,
        } = value.as_ref()
        else {
            continue;
        };
        if right != output || actual != space {
            continue;
        }
        let value = cas.state.expression(left.logical_id())?;
        if value.operator == (Operator::DistributionDerivative { order })
            && value.operands == [input.logical_id()]
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Higher orders consume two actual derivative theorems, rather than branch-jet padding.
fn iterate(
    input: &Handle,
    output: &Handle,
    space: &Handle,
    order: u32,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let [prefix, next] = premises else {
        return Err(invalid());
    };
    let (Statement::Distribution(prefix), Statement::Distribution(next)) =
        (&prefix.judgment.statement, &next.judgment.statement)
    else {
        return Err(invalid());
    };
    let DistributionStatement::Derivative {
        distribution,
        derivative,
        space: first_space,
        order: first_order,
    } = prefix.as_ref()
    else {
        return Err(invalid());
    };
    if distribution != input || first_space != space || *first_order != order - 1 {
        return Err(invalid());
    }
    if *next.as_ref()
        != (DistributionStatement::Derivative {
            distribution: derivative.clone(),
            derivative: output.clone(),
            space: space.clone(),
            order: 1,
        })
    {
        return Err(invalid());
    }
    Ok(())
}

/// Differentiate each structural term with either the delta definition or an explicit premise.
pub(crate) fn expand(
    cas: &mut Cas,
    input: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let header = cas.expression_header(input)?;
    let terms = crate::distribution_linear::terms(cas, input.logical_id(), &[], budget)?;
    let mut result = crate::distribution_terms::node(
        cas,
        &header,
        space,
        Operator::DistributionZero,
        &[],
        budget,
    )?;
    for (term, coefficient) in terms.values() {
        let term = term.as_ref().ok_or_else(invalid)?;
        let derivative = term_derivative(cas, term, space, premises, budget)?;
        let factor = crate::distribution_terms::rational(cas, &header, coefficient, budget)?;
        let scaled = crate::distribution_terms::node(
            cas,
            &header,
            space,
            Operator::DistributionScale,
            &[factor, derivative],
            budget,
        )?;
        result = crate::distribution_terms::node(
            cas,
            &header,
            space,
            Operator::DistributionAdd,
            &[result, scaled],
            budget,
        )?;
    }
    Ok(result)
}

/// Delta order is checked before construction; all other atoms require a derivative theorem.
fn term_derivative(
    cas: &mut Cas,
    term: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let expression = cas.state.expression(term.logical_id())?.clone();
    if let Operator::Delta { order } = expression.operator {
        let order = order.checked_add(1).ok_or_else(invalid)?;
        budget.specialized(BudgetKind::DeltaOrder, u64::from(order), false, AT)?;
        let location = cas.handle(*expression.operands.first().ok_or_else(invalid)?);
        let header = cas.handle(expression.header);
        return crate::distribution_terms::node(
            cas,
            &header,
            space,
            Operator::Delta { order },
            &[location],
            budget,
        );
    }
    for premise in premises {
        if let Statement::Distribution(value) = &premise.judgment.statement
            && let DistributionStatement::Derivative {
                distribution,
                derivative,
                space: actual,
                order: 1,
            } = value.as_ref()
            && distribution == term
            && actual == space
        {
            return Ok(derivative.clone());
        }
    }
    Err(crate::error::Fault::invalid(
        crate::error::InvalidCode::PendingPremise,
        AT,
    ))
}

/// The defining first derivative action moves one derivative to the test with a minus sign.
fn action(
    cas: &mut Cas,
    pairing: &Handle,
    value: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
    crate::distribution_action::well_formed(cas, pairing, space, premises, budget)?;
    let pair = cas.state.expression(pairing.logical_id())?.clone();
    if pair.operator != Operator::Pair {
        return Err(invalid());
    }
    let [distribution, test] = pair.operands.as_slice() else {
        return Err(invalid());
    };
    let source = cas.state.expression(*distribution)?.clone();
    if source.operator != (Operator::DistributionDerivative { order: 1 }) {
        return Err(invalid());
    }
    let input = cas.handle(*source.operands.first().ok_or_else(invalid)?);
    let header = cas.handle(pair.header);
    let test = cas.handle(*test);
    let differentiated = crate::distribution_terms::node(
        cas,
        &header,
        space,
        Operator::TestDerivative { order: 1 },
        &[test],
        budget,
    )?;
    let action = crate::distribution_terms::node(
        cas,
        &header,
        space,
        Operator::Pair,
        &[input, differentiated],
        budget,
    )?;
    let negative =
        crate::distribution_terms::node(cas, &header, space, Operator::Neg, &[action], budget)?;
    crate::distribution_linear::equal_terms(cas, &negative, value, premises, budget)
}
