// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational maps over complete structural distribution and scalar-action terms.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::DistributionStatement;
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use std::collections::BTreeMap;

/// A structural term and its coordinate-independent rational coefficient.
pub(crate) type Terms = BTreeMap<Vec<u8>, (Option<Handle>, Rational)>;

/// Reconstruct linear identities, including composition through supplied typed equalities.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if draft.certificate != Certificate::None {
        return Err(invalid());
    }
    let Statement::Distribution(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let (left, right, space) = match statement.as_ref() {
        DistributionStatement::Equality { left, right, space } => {
            crate::distribution_check::membership(cas, left.logical_id(), space, premises, budget)?;
            crate::distribution_check::membership(
                cas,
                right.logical_id(),
                space,
                premises,
                budget,
            )?;
            (left, right, space)
        }
        DistributionStatement::Pairing {
            pairing,
            value,
            space,
        } => {
            budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
            crate::distribution_action::well_formed(cas, pairing, space, premises, budget)?;
            let expanded = crate::distribution_action::linear_value(cas, pairing, space, budget)?;
            return equal_terms(cas, &expanded, value, premises, budget);
        }
        _ => return Err(invalid()),
    };
    for premise in premises {
        if let Statement::Distribution(value) = &premise.judgment.statement
            && value.space() != space
        {
            return Err(invalid());
        }
    }
    if coefficients(&terms(cas, left.logical_id(), premises, budget)?)
        != coefficients(&terms(cas, right.logical_id(), premises, budget)?)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Ignore local handles when comparing fully structural rational maps.
fn coefficients(values: &Terms) -> BTreeMap<&Vec<u8>, &Rational> {
    values
        .iter()
        .map(|(key, (_, coefficient))| (key, coefficient))
        .collect()
}

/// Collect full terms; no display spelling or point sampling determines equality.
pub(crate) fn terms(
    cas: &Cas,
    root: LogicalId,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Terms, Fault> {
    let mut result = BTreeMap::new();
    let mut pending = vec![(root, Rational::one(), 0_u64)];
    while let Some((id, coefficient, depth)) = pending.pop() {
        budget.check(BudgetKind::Depth, depth, AT)?;
        budget.work(1, AT)?;
        if let Some(replacement) = replacement(id, premises) {
            pending.push((replacement, coefficient, depth.saturating_add(1)));
            continue;
        }
        collect(
            cas,
            id,
            coefficient,
            depth,
            &mut pending,
            &mut result,
            budget,
        )?;
    }
    Ok(result)
}

/// Expand one linear constructor while retaining every other complete structural term.
fn collect(
    cas: &Cas,
    id: LogicalId,
    coefficient: Rational,
    depth: u64,
    pending: &mut Vec<(LogicalId, Rational, u64)>,
    result: &mut Terms,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let value = cas.state.expression(id)?;
    match &value.operator {
        Operator::DistributionZero => {}
        Operator::Rational(number) => accumulate(
            result,
            Vec::new(),
            None,
            coefficient.mul(number, budget)?,
            budget,
        )?,
        Operator::DistributionAdd | Operator::Add | Operator::Sub => {
            let [left, right] = value.operands.as_slice() else {
                return Err(invalid());
            };
            let sign = if value.operator == Operator::Sub {
                coefficient.negated(budget)?
            } else {
                coefficient.clone()
            };
            pending.push((*left, coefficient, depth.saturating_add(1)));
            pending.push((*right, sign, depth.saturating_add(1)));
        }
        Operator::Neg => pending.push((
            *value.operands.first().ok_or_else(invalid)?,
            coefficient.negated(budget)?,
            depth.saturating_add(1),
        )),
        Operator::DistributionScale | Operator::Mul
            if rational_factor(cas, &value.operands).is_some() =>
        {
            let (factor, term) = rational_factor(cas, &value.operands).ok_or_else(invalid)?;
            pending.push((
                term,
                coefficient.mul(factor, budget)?,
                depth.saturating_add(1),
            ));
        }
        _ => {
            let key = crate::wire_build::syntax_key(cas, id, budget)?;
            accumulate(result, key, Some(cas.handle(id)), coefficient, budget)?;
        }
    }
    Ok(())
}

/// Supplied equalities are used only in their exact typed context and structural endpoints.
fn replacement(id: LogicalId, premises: &[CheckedApplication]) -> Option<LogicalId> {
    premises.iter().find_map(|premise| {
        let Statement::Distribution(value) = &premise.judgment.statement else {
            return None;
        };
        let (left, right) = match value.as_ref() {
            DistributionStatement::Equality { left, right, .. } => (left, right),
            DistributionStatement::Pairing { pairing, value, .. } => (pairing, value),
            _ => return None,
        };
        (left.logical_id() == id && left != right).then_some(right.logical_id())
    })
}

/// Only rational literals act as scalar coefficients; arbitrary functions stay structural.
fn rational_factor<'a>(cas: &'a Cas, operands: &[LogicalId]) -> Option<(&'a Rational, LogicalId)> {
    let [first, second] = operands else {
        return None;
    };
    crate::distribution_check::rational(cas, *first).map_or_else(
        |_| {
            crate::distribution_check::rational(cas, *second)
                .ok()
                .map(|value| (value, *first))
        },
        |value| Some((value, *second)),
    )
}

/// Combine one coefficient, removing only exact rational zero.
pub(crate) fn accumulate(
    result: &mut Terms,
    key: Vec<u8>,
    term: Option<Handle>,
    coefficient: Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.check(
        BudgetKind::PolynomialTerms,
        size(result.len().saturating_add(1), AT)?,
        AT,
    )?;
    let retained = result.keys().try_fold(size(key.len(), AT)?, |total, key| {
        total
            .checked_add(size(key.len(), AT)?)
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
    })?;
    budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
    budget.work(retained, AT)?;
    let sum = match result.get(&key) {
        Some((_, previous)) => previous.add(&coefficient, budget)?,
        None => coefficient,
    };
    if sum.is_zero() {
        result.remove(&key);
    } else {
        result.insert(key, (term, sum));
    }
    Ok(())
}

/// Compare the full structural rational maps after applying exact earlier equalities.
pub(crate) fn equal_terms(
    cas: &Cas,
    left: &Handle,
    right: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if coefficients(&terms(cas, left.logical_id(), premises, budget)?)
        == coefficients(&terms(cas, right.logical_id(), premises, budget)?)
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Local coefficient-map behavior without a producer or shared fixture.
#[cfg(test)]
mod tests {
    use super::{Terms, accumulate};
    use crate::budget::{Budget, Limits};
    use crate::error::Fault;
    use crate::number::Rational;

    /// Exact cancellation removes only the identical structural term, preserving another key.
    /// ´test:unit:distribution-coefficient-map-cancellation´
    #[test]
    fn distribution_coefficient_map_cancellation() -> Result<(), Fault> {
        let mut budget = Budget::new(Limits::default())?;
        let mut terms = Terms::new();
        accumulate(&mut terms, vec![1], None, Rational::one(), &mut budget)?;
        accumulate(&mut terms, vec![2], None, Rational::one(), &mut budget)?;
        let negative = Rational::from_ratio(-1, 1, &mut budget)?;
        accumulate(&mut terms, vec![1], None, negative, &mut budget)?;
        assert_eq!(terms.keys().cloned().collect::<Vec<_>>(), vec![vec![2]]);
        assert_eq!(
            super::coefficients(&terms)
                .values()
                .map(|number| (**number).clone())
                .collect::<Vec<_>>(),
            vec![Rational::one()]
        );
        Ok(())
    }
}
