// SPDX-License-Identifier: AGPL-3.0-only
//! Finite structural support and cutoff-independent smooth-germ action.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::{DistributionCertificate, DistributionStatement};
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Reconstruct a finite containing support; a regular distribution has no implicit compact support.
pub(crate) fn support(
    cas: &Cas,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let mut pending = vec![root];
    let mut result: Vec<Rational> = Vec::new();
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let value = cas.state.expression(id)?;
        match value.operator {
            Operator::DistributionZero => {}
            Operator::Delta { .. } => {
                let point = crate::distribution_check::rational(
                    cas,
                    *value.operands.first().ok_or_else(invalid)?,
                )?;
                let mut index = 0;
                while let Some(previous) = result.get(index) {
                    if previous.compare(point, budget)? != std::cmp::Ordering::Less {
                        break;
                    }
                    index += 1;
                }
                if result.get(index) != Some(point) {
                    budget.specialized(
                        BudgetKind::BoundaryNodes,
                        size(result.len().saturating_add(1), AT)?,
                        false,
                        AT,
                    )?;
                    result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                    result.insert(index, point.clone());
                }
            }
            Operator::DistributionAdd | Operator::DistributionDerivative { .. } => {
                pending.extend(&value.operands);
            }
            Operator::DistributionScale | Operator::SmoothMultiply => {
                pending.push(*value.operands.get(1).ok_or_else(invalid)?);
            }
            _ => return Err(Fault::invalid(InvalidCode::PendingPremise, AT)),
        }
    }
    Ok(result)
}

/// Interior containment and smoothness are checked near every reconstructed support point.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    let DistributionStatement::SupportAction {
        distribution,
        function,
        neighborhood,
        space,
    } = value.as_ref()
    else {
        return Err(invalid());
    };
    budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
    crate::distribution_check::membership(cas, distribution.logical_id(), space, premises, budget)?;
    crate::distribution_check::measure_test(cas, function.logical_id(), budget)?;
    let points = support(cas, distribution.logical_id(), budget)?;
    if draft.certificate
        != Certificate::Distribution(Box::new(DistributionCertificate::Support(points.clone())))
    {
        return Err(invalid());
    }
    crate::case_check::subset(
        cas,
        neighborhood.logical_id(),
        cas.state.space(space.logical_id())?.domain,
        budget,
    )?;
    for point in &points {
        crate::distribution_check::interior(cas, neighborhood.logical_id(), point, budget)?;
    }
    // The executable germ fragment is polynomial, hence smooth on the entire neighborhood.
    crate::distribution_regular::polynomial(cas, function, space, budget)?;
    Ok(())
}

/// A near-support pairing consumes its exact checked germ and neighborhood theorem.
pub(crate) fn action_premise(
    distribution: LogicalId,
    function: LogicalId,
    space: &Handle,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    if premises.iter().any(|premise| {
        matches!(&premise.judgment.statement, Statement::Distribution(value)
            if matches!(value.as_ref(), DistributionStatement::SupportAction {
                distribution: input, function: germ, space: actual, ..
            } if input.logical_id() == distribution && germ.logical_id() == function && actual == space))
    }) { Ok(()) } else { Err(Fault::invalid(InvalidCode::PendingPremise, AT)) }
}
