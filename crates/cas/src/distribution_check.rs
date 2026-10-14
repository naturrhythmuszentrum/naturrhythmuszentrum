// SPDX-License-Identifier: AGPL-3.0-only
//! Closed distribution kernel dispatch and reconstructed membership requirements.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::DistributionStatement;
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{Operator, Sort};
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::rule::Primitive;
use std::collections::BTreeSet;

/// All nine primitives retain complete typed conditions and exact contexts.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    crate::distribution_syntax::statement(cas, value, budget)?;
    if !draft.substitutions.is_empty()
        || premises.iter().any(|p| {
            p.judgment.context != draft.context
                || p.judgment
                    .space
                    .as_ref()
                    .is_some_and(|space| space != value.space())
        })
    {
        return Err(invalid());
    }
    for premise in premises {
        crate::proof_view::validate_judgment(cas, &premise.judgment, budget)?;
    }
    let domain = cas.state.space(cas.checked(value.space())?)?.domain;
    let within = cas.state.context(cas.checked(&draft.context)?)?.admissible;
    crate::case_check::subset(cas, domain, within, budget)?;
    match draft.rule {
        Primitive::DistRegular => crate::distribution_regular::check(cas, draft, premises, budget)?,
        Primitive::DistAe => crate::distribution_ae::check(cas, draft, premises, budget)?,
        Primitive::DistLinear => crate::distribution_linear::check(cas, draft, premises, budget)?,
        Primitive::DistDerivative => {
            crate::distribution_derivative::check(cas, draft, premises, budget)?;
        }
        Primitive::DistDeltaPair => {
            crate::distribution_action::check(cas, draft, premises, budget)?;
        }
        Primitive::DistSmooth => {
            crate::distribution_multiply::smooth(cas, draft, premises, budget)?;
        }
        Primitive::DistDeltaMultiply => {
            crate::distribution_multiply::delta(cas, draft, premises, budget)?;
        }
        Primitive::DistSupportAction => {
            crate::distribution_support::check(cas, draft, premises, budget)?;
        }
        Primitive::DistJump => crate::distribution_jump::check(cas, draft, premises, budget)?,
        _ => return Err(invalid()),
    }
    let mut result = ScopedConditions::default();
    for premise in premises {
        crate::statement::merge(cas, &mut result, &premise.conditions, budget)?;
    }
    Ok(result)
}

/// Previously checked endpoint membership cannot be inferred from an unproved syntax node.
fn known_member(id: LogicalId, space: &Handle, premises: &[CheckedApplication]) -> bool {
    premises.iter().any(|premise| {
        let Statement::Distribution(value) = &premise.judgment.statement else {
            return false;
        };
        if value.space() != space {
            return false;
        }
        match value.as_ref() {
            DistributionStatement::Embedding { embedding, .. } => embedding.logical_id() == id,
            DistributionStatement::Equality { left, right, .. } => {
                left.logical_id() == id || right.logical_id() == id
            }
            DistributionStatement::Derivative {
                distribution,
                derivative,
                ..
            } => distribution.logical_id() == id || derivative.logical_id() == id,
            DistributionStatement::SupportAction { distribution, .. } => {
                distribution.logical_id() == id
            }
            _ => false,
        }
    })
}

/// Reconstruct membership through the complete distribution tree, retaining embedding premises.
pub(crate) fn membership(
    cas: &Cas,
    root: LogicalId,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        if !seen.insert(id) {
            continue;
        }
        let value = cas.state.expression(id)?.clone();
        if value.sort != Sort::Distribution || value.space != Some(cas.checked(space)?) {
            return Err(invalid());
        }
        if known_member(id, space, premises) {
            continue;
        }
        match value.operator {
            Operator::Free(_) | Operator::DistributionZero => {}
            Operator::RegularEmbed => {
                let function = cas.handle(*value.operands.first().ok_or_else(invalid)?);
                crate::distribution_regular::evidence(&function, space, premises)?;
            }
            Operator::Delta { order } => {
                budget.specialized(BudgetKind::DeltaOrder, u64::from(order), false, AT)?;
                let point = rational(cas, *value.operands.first().ok_or_else(invalid)?)?;
                interior(
                    cas,
                    cas.state.space(space.logical_id())?.domain,
                    point,
                    budget,
                )?;
            }
            Operator::DistributionAdd | Operator::DistributionDerivative { .. } => {
                pending.extend(value.operands);
            }
            Operator::DistributionScale | Operator::SmoothMultiply => {
                if value.operator == Operator::SmoothMultiply {
                    crate::distribution_regular::polynomial(
                        cas,
                        &cas.handle(*value.operands.first().ok_or_else(invalid)?),
                        space,
                        budget,
                    )?;
                }
                pending.push(*value.operands.get(1).ok_or_else(invalid)?);
            }
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// Delta locations are exact interior points, including in bounded intervals.
pub(crate) fn interior(
    cas: &Cas,
    region: LogicalId,
    point: &crate::number::Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let cells = crate::region_cache::partition_at(cas, &[region], Some(point), budget)?;
    let index = cells
        .cuts
        .iter()
        .position(|cut| cut == point)
        .ok_or_else(invalid)?;
    let row = cells.membership.first().ok_or_else(invalid)?;
    let middle = index
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(invalid)?;
    if !(middle - 1..=middle + 1).all(|position| row.get(position) == Some(&true)) {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
    }
    Ok(())
}

/// Borrow a literal coefficient or location without numerical evaluation.
pub(crate) fn rational(cas: &Cas, id: LogicalId) -> Result<&crate::number::Rational, Fault> {
    let Operator::Rational(value) = &cas.state.expression(id)?.operator else {
        return Err(Fault::invalid(InvalidCode::PendingPremise, AT));
    };
    Ok(value)
}

/// Check compact smooth test construction and meter every reached test/germ expression.
pub(crate) fn test(
    cas: &Cas,
    root: LogicalId,
    space: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    measure_test(cas, root, budget)?;
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let value = cas.state.expression(id)?;
        if value.sort != Sort::Test || value.space != Some(space.logical_id()) {
            return Err(invalid());
        }
        match value.operator {
            Operator::Free(_) => {}
            Operator::TestDerivative { .. } => pending.extend(&value.operands),
            Operator::TestMultiply => {
                crate::distribution_regular::polynomial(
                    cas,
                    &cas.handle(*value.operands.first().ok_or_else(invalid)?),
                    space,
                    budget,
                )?;
                pending.push(*value.operands.get(1).ok_or_else(invalid)?);
            }
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// The specialized test-node bound counts the complete expression DAG, not a producer estimate.
pub(crate) fn measure_test(cas: &Cas, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    let nodes = crate::case_syntax::order(cas, &[root], budget)?;
    budget.specialized(BudgetKind::TestNodes, size(nodes.len(), AT)?, false, AT)
}
