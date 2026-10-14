// SPDX-License-Identifier: AGPL-3.0-only
//! Reached-region requirements with explicit empty-region and pending semantics.

use crate::budget::Budget;
use crate::case_syntax::{self, invalid};
use crate::domain_model::{Layer, RegionExpr};
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, LogicalId};
use crate::model::PredicateKind;
use crate::proof_model::{CheckedApplication, GuardedObligation, ScopedConditions, Statement};

/// Whether exact region syntax or its checked rational partition proves emptiness.
pub(crate) fn empty(cas: &Cas, region: LogicalId, budget: &mut Budget) -> Result<bool, Fault> {
    if matches!(cas.state.region(region)?.expression, RegionExpr::Empty) {
        return Ok(true);
    }
    let cells = crate::region::partition(&cas.state, &[region], budget)?;
    Ok(cells
        .membership
        .first()
        .ok_or_else(invalid)?
        .iter()
        .all(|value| !value))
}

/// Identify supported uncertainty without confusing exhaustion or false claims with pending.
pub(crate) const fn unknown(fault: &Fault) -> bool {
    matches!(
        fault.kind,
        FaultKind::Invalid(
            InvalidCode::UnsupportedFragment
                | InvalidCode::PolynomialFragment
                | InvalidCode::CoefficientField
        )
    )
}

/// Retain a value's Defined requirement precisely on the region where it is reached.
pub(crate) fn defined(
    cas: &mut Cas,
    context: LogicalId,
    region: LogicalId,
    term: LogicalId,
    premises: &[CheckedApplication],
    result: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let predicate = case_syntax::predicate(cas, term, PredicateKind::Defined, budget)?;
    require(cas, context, region, predicate, premises, result, budget)?;
    let mut pending = vec![term];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.condition(1, Location::new("case branch requirements"))?;
        if !seen.insert(id) {
            continue;
        }
        let value = cas.state.expression(id)?.clone();
        let denominator = match value.operator {
            crate::model::Operator::Div => value.operands.get(1).copied(),
            crate::model::Operator::IntegerPower(power) if power <= 0 => {
                value.operands.first().copied()
            }
            _ => None,
        };
        if let Some(denominator) = denominator {
            let predicate =
                case_syntax::predicate(cas, denominator, PredicateKind::NonZero, budget)?;
            require(cas, context, region, predicate, premises, result, budget)?;
        }
        pending
            .try_reserve(value.operands.len())
            .map_err(|_| Fault::allocation(Location::new("case branch requirements")))?;
        pending.extend(value.operands);
    }
    Ok(())
}

/// Discharge only exact evidence; unknown predicates remain full guarded tuples.
pub(crate) fn require(
    cas: &mut Cas,
    context: LogicalId,
    region: LogicalId,
    predicate: LogicalId,
    premises: &[CheckedApplication],
    result: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (_, scope) = crate::descriptor::metadata(&cas.state, predicate)?;
    let obligation = GuardedObligation {
        home: cas.handle(context),
        scope: scope.clone(),
        layer: Layer::Pointwise,
        space: None,
        region: cas.handle(region),
        predicate: cas.handle(predicate),
    };
    let at = Location::new("case guarded requirements");
    budget.check(
        crate::budget::BudgetKind::ConditionAtoms,
        crate::graph::size(result.requirements.len().saturating_add(1), at)?,
        at,
    )?;
    result
        .requirements
        .try_reserve(1)
        .map_err(|_| Fault::allocation(at))?;
    result.requirements.push(obligation.clone());
    match empty(cas, region, budget) {
        Ok(true) => return Ok(()),
        Err(fault) if !unknown(&fault) => return Err(fault),
        _ => {}
    }
    for premise in premises {
        if premise.judgment.context == obligation.home
            && premise.judgment.statement
                == (Statement::PredicateFact {
                    region: obligation.region.clone(),
                    predicate: obligation.predicate.clone(),
                })
        {
            crate::statement::merge(cas, result, &premise.conditions, budget)?;
            return Ok(());
        }
    }
    let truth = case_syntax::guarded(cas, region, predicate, budget)?;
    match crate::region::partition(&cas.state, &[region, truth], budget) {
        Ok(cells) => {
            let reached = cells.membership.first().ok_or_else(invalid)?;
            let holds = cells.membership.get(1).ok_or_else(invalid)?;
            if reached
                .iter()
                .zip(holds)
                .all(|(reached, holds)| !reached || *holds)
            {
                return Ok(());
            }
            Err(Fault::invalid(
                InvalidCode::RefutedDomain,
                Location::new("reached case requirement"),
            ))
        }
        Err(fault) if unknown(&fault) => {
            result
                .residuals
                .try_reserve(1)
                .map_err(|_| Fault::allocation(at))?;
            result.residuals.push(obligation);
            Ok(())
        }
        Err(fault) => Err(fault),
    }
}
