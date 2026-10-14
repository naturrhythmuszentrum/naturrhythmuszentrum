// SPDX-License-Identifier: AGPL-3.0-only
//! Finite ordered rewrite proposals, checked independently before becoming search edges.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::model::Operator;
use crate::producer::{Plan, integer, step, term};
use crate::proof_model::Relation;
use crate::rule::Primitive;

/// A deterministic match cursor entry; no arbitrary terms or variables are synthesized.
#[derive(Clone, Debug)]
pub(crate) struct Action {
    /// The finite generator family in lexical rule order.
    pub(crate) primitive: Primitive,
    /// Ordered operand position of the proposed rewrite.
    path: Vec<u32>,
    /// The actual source leaf at that path.
    leaf: LogicalId,
    /// An explicit equality target, only after structural expansion proposals are resolved.
    target: Option<LogicalId>,
    /// An older proof frozen before this search began.
    pub(crate) known: Option<crate::known::Known>,
}

impl Action {
    /// Reject shape mismatches before paying for a complete transaction copy.
    pub(crate) fn plausible(&self, cas: &Cas) -> Result<bool, Fault> {
        if self.known.is_some() {
            return Ok(true);
        }
        let expression = cas.state.expression(self.leaf)?;
        match self.primitive {
            Primitive::Power => {
                let Operator::IntegerPower(exponent) = expression.operator else {
                    return Ok(false);
                };
                if exponent <= 1 {
                    return Ok(true);
                }
                let base = expression.operands.first().copied().ok_or_else(invalid)?;
                Ok(matches!(
                    cas.state.expression(base)?.operator,
                    Operator::Add | Operator::Sub | Operator::Mul
                ))
            }
            Primitive::Distribute | Primitive::Jets => {
                if expression.operator != Operator::Mul {
                    return Ok(false);
                }
                let position = usize::from(self.primitive == Primitive::Distribute);
                let child = expression
                    .operands
                    .get(position)
                    .copied()
                    .ok_or_else(invalid)?;
                Ok(cas.state.expression(child)?.operator == Operator::Add)
            }
            Primitive::RationalIdentity => Ok(true),
            _ => Ok(false),
        }
    }
}

/// Rewriting location.
const AT: Location = Location::new("identity generation");

/// Enumerate finite lexical rule/position matches, including failed structural matches.
pub(crate) fn actions(
    cas: &Cas,
    root: LogicalId,
    target: Option<LogicalId>,
    known: &[crate::known::Known],
    expanded: bool,
    budget: &mut Budget,
) -> Result<Vec<Action>, Fault> {
    let mut paths = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    let mut expansion = false;
    while let Some((id, path)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, size(path.len(), AT)?, AT)?;
        let expression = cas.state.expression(id)?;
        expansion |= expandable(cas, id)?;
        let count = size(paths.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, AT))?;
        budget.check(BudgetKind::PeakExpressionSize, count, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count
                .checked_mul(512)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            AT,
        )?;
        for (position, child) in expression.operands.iter().enumerate().rev() {
            let mut child_path = path.clone();
            child_path
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            child_path
                .push(u32::try_from(position).map_err(|_| Fault::capacity(BudgetKind::Depth, AT))?);
            pending.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            pending.push((*child, child_path));
        }
        paths.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        paths.push((id, path));
    }
    let mut result = Vec::new();
    for primitive in [Primitive::Distribute, Primitive::Power, Primitive::Jets] {
        result
            .try_reserve(paths.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (leaf, path) in &paths {
            budget.work(
                size(path.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            result.push(Action {
                primitive,
                path: path.clone(),
                leaf: *leaf,
                target: None,
                known: None,
            });
        }
    }
    if !expansion || expanded {
        result.try_reserve(2).map_err(|_| Fault::allocation(AT))?;
        result.push(Action {
            primitive: Primitive::RationalIdentity,
            path: Vec::new(),
            leaf: root,
            target: None,
            known: None,
        });
        if let Some(target) = target {
            result.push(Action {
                primitive: Primitive::RationalIdentity,
                path: Vec::new(),
                leaf: root,
                target: Some(target),
                known: None,
            });
        }
    }
    insert_known(&mut result, root, known, budget)?;
    Ok(result)
}

/// Merge older root matches into the already ordered finite generator schedule.
fn insert_known(
    actions: &mut Vec<Action>,
    root: LogicalId,
    known: &[crate::known::Known],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for value in known {
        budget.work(1, AT)?;
        if value.left != root {
            continue;
        }
        let count = size(actions.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::MatchAttempts, AT))?;
        budget.work(
            count
                .checked_mul(256)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let proposal = Action {
            primitive: value.primitive,
            path: Vec::new(),
            leaf: root,
            target: None,
            known: Some(*value),
        };
        let index =
            actions.partition_point(|existing| action_order(existing) <= action_order(&proposal));
        actions.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        actions.insert(index, proposal);
    }
    Ok(())
}

/// Exact rule, ordered path and cost/proof order; generated forms precede reused ties.
fn action_order(action: &Action) -> (&'static str, &[u32], Option<(u64, LogicalId)>) {
    (
        action.primitive.specification().key,
        &action.path,
        action.known.map(|known| (known.cost, known.proof)),
    )
}

/// A coefficient shortcut waits for explicit nontrivial algebraic expansion opportunities.
fn expandable(cas: &Cas, id: LogicalId) -> Result<bool, Fault> {
    let expression = cas.state.expression(id)?;
    if let Operator::IntegerPower(exponent) = expression.operator {
        let base = expression.operands.first().copied().ok_or_else(invalid)?;
        return Ok(exponent >= 2
            && matches!(
                cas.state.expression(base)?.operator,
                Operator::Add | Operator::Sub | Operator::Mul
            ));
    }
    if expression.operator == Operator::Mul {
        for child in &expression.operands {
            if matches!(cas.state.expression(*child)?.operator, Operator::Add) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Generate one proposed edge, including congruence and a path composition.
pub(crate) fn propose(
    cas: &mut Cas,
    root: LogicalId,
    original: LogicalId,
    previous: &Plan,
    action: &Action,
    budget: &mut Budget,
) -> Result<Option<Plan>, Fault> {
    if let Some(known) = action.known {
        budget.work(
            size(previous.steps.len(), AT)?
                .checked_mul(1024)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let mut plan = previous.clone();
        let prior = plan
            .steps
            .len()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(invalid)?;
        let imported = crate::known::append(&mut plan, cas, known.proof, budget)?;
        let output = cas.checked(&plan.get(imported)?.right)?;
        plan.push(
            step(
                cas,
                Primitive::Transitivity,
                Relation::EqualOnDomain,
                original,
                output,
                vec![prior, imported],
            ),
            budget,
        )?;
        return Ok(Some(plan));
    }
    let Some(replacement) = replacement(cas, action, budget)? else {
        return Ok(None);
    };
    if action.leaf == replacement {
        return Ok(None);
    }
    budget.work(
        size(previous.steps.len(), AT)?
            .checked_mul(1024)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )?;
    let mut plan = previous.clone();
    let prior = plan
        .steps
        .len()
        .checked_sub(1)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(invalid)?;
    let inner = plan.push(
        step(
            cas,
            action.primitive,
            Relation::EqualOnDomain,
            action.leaf,
            replacement,
            Vec::new(),
        ),
        budget,
    )?;
    let output = replace(cas, root, &action.path, replacement, budget)?;
    let edge = if action.path.is_empty() {
        inner
    } else {
        let mut congruence = step(
            cas,
            Primitive::Congruence,
            Relation::EqualOnDomain,
            root,
            output,
            vec![inner],
        );
        congruence.path.clone_from(&action.path);
        plan.push(congruence, budget)?
    };
    plan.push(
        step(
            cas,
            Primitive::Transitivity,
            Relation::EqualOnDomain,
            original,
            output,
            vec![prior, edge],
        ),
        budget,
    )?;
    Ok(Some(plan))
}

/// Construct one local formula without granting it any truth status.
fn replacement(
    cas: &mut Cas,
    action: &Action,
    budget: &mut Budget,
) -> Result<Option<LogicalId>, Fault> {
    let expression = cas.state.expression(action.leaf)?;
    let operator = expression.operator.clone();
    let operands = expression.operands.clone();
    let child = |position| operands.get(position).copied().ok_or_else(invalid);
    match action.primitive {
        Primitive::Power => {
            let Operator::IntegerPower(exponent) = operator else {
                return Ok(None);
            };
            budget.check(BudgetKind::IntegerExponent, exponent.unsigned_abs(), AT)?;
            let base = child(0)?;
            let result = match exponent {
                0 => integer(cas, action.leaf, 1, budget)?,
                1 => base,
                2.. => {
                    let previous = exponent.checked_sub(1).ok_or_else(invalid)?;
                    let power = term(
                        cas,
                        action.leaf,
                        Operator::IntegerPower(previous),
                        &[base],
                        budget,
                    )?;
                    term(cas, action.leaf, Operator::Mul, &[base, power], budget)?
                }
                _ => {
                    let exponent = exponent
                        .checked_neg()
                        .ok_or_else(|| Fault::capacity(BudgetKind::IntegerExponent, AT))?;
                    let power = term(
                        cas,
                        action.leaf,
                        Operator::IntegerPower(exponent),
                        &[base],
                        budget,
                    )?;
                    let one = integer(cas, action.leaf, 1, budget)?;
                    term(cas, action.leaf, Operator::Div, &[one, power], budget)?
                }
            };
            Ok(Some(result))
        }
        Primitive::Distribute | Primitive::Jets => {
            if operator != Operator::Mul {
                return Ok(None);
            }
            let (factor, sum) = if action.primitive == Primitive::Distribute {
                (child(0)?, child(1)?)
            } else {
                (child(1)?, child(0)?)
            };
            distribute(cas, action.leaf, factor, sum, budget)
        }
        Primitive::RationalIdentity => {
            if let Some(target) = action.target {
                if !crate::polynomial::equal(&cas.state, action.leaf, target, true, budget)? {
                    return Ok(None);
                }
                Ok(Some(target))
            } else {
                Ok(Some(crate::polynomial::normalize(
                    cas,
                    action.leaf,
                    budget,
                )?))
            }
        }
        _ => Err(invalid()),
    }
}

/// Left-distribution is checked by coefficient identity; right-distribution has its field schema.
fn distribute(
    cas: &mut Cas,
    model: LogicalId,
    factor: LogicalId,
    sum: LogicalId,
    budget: &mut Budget,
) -> Result<Option<LogicalId>, Fault> {
    let expression = cas.state.expression(sum)?;
    if expression.operator != Operator::Add {
        return Ok(None);
    }
    let first = expression.operands.first().copied().ok_or_else(invalid)?;
    let second = expression.operands.get(1).copied().ok_or_else(invalid)?;
    let left = term(cas, model, Operator::Mul, &[factor, first], budget)?;
    let right = term(cas, model, Operator::Mul, &[factor, second], budget)?;
    Ok(Some(term(
        cas,
        model,
        Operator::Add,
        &[left, right],
        budget,
    )?))
}

/// Rebuild only an ordered ancestor chain; every replacement remains closed scalar syntax.
fn replace(
    cas: &mut Cas,
    mut root: LogicalId,
    path: &[u32],
    replacement: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut ancestors = Vec::new();
    ancestors
        .try_reserve_exact(path.len())
        .map_err(|_| Fault::allocation(AT))?;
    for position in path {
        let expression = cas.state.expression(root)?;
        let index = usize::try_from(*position).map_err(|_| invalid())?;
        let child = expression
            .operands
            .get(index)
            .copied()
            .ok_or_else(invalid)?;
        ancestors.push((root, index));
        root = child;
    }
    let mut output = replacement;
    for (parent, position) in ancestors.into_iter().rev() {
        let expression = cas.state.expression(parent)?;
        let operator = expression.operator.clone();
        let mut operands = expression.operands.clone();
        *operands.get_mut(position).ok_or_else(invalid)? = output;
        output = term(cas, parent, operator, &operands, budget)?;
    }
    Ok(output)
}

/// Malformed internal references remain explicit failures.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}
