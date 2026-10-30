// SPDX-License-Identifier: AGPL-3.0-only
//! Conditional source identities and qualified assumptions survive every scalar proof consumer.

use super::super::model::ScalarKernelLaw;
use crate::budget::{Budget, BudgetKind};
use crate::decision::{EnclosureOrigin, RationalEnclosure};
use crate::graph::{Cas, Edge, LogicalId, size};
use crate::proof_model::ScopedConditions;
use crate::{Fault, Location};

/// New conditional dependencies have their own charged owner.
const AT: Location = Location::new("scalar kernel family retention");

/// Closed versions retain their existing empty assumption collection and accounting.
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar proof consumers retain every qualified source assumption"
)]
pub(crate) fn retain(
    cas: &Cas,
    contract: &RationalEnclosure,
    conditions: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let EnclosureOrigin::ScalarKernel(certificate) = &contract.origin else {
        return Ok(());
    };
    if certificate.version < 3 {
        return Ok(());
    }
    for record in &certificate.records {
        budget.work(1, AT)?;
        if let ScalarKernelLaw::Family(input) = &record.law {
            let count = size(conditions.hypotheses.len().saturating_add(2), AT)?;
            budget.check(BudgetKind::ConditionAtoms, count, AT)?;
            budget.check(BudgetKind::StagedPayloadBytes, count.saturating_mul(64), AT)?;
            budget.work(128, AT)?;
            conditions
                .hypotheses
                .try_reserve(2)
                .map_err(|_| Fault::allocation(AT))?;
            conditions
                .hypotheses
                .extend_from_slice(&input.family.premises);
        }
    }
    crate::statement::canonical_hypotheses(cas, &mut conditions.hypotheses, budget)
}

/// Complete source and assumption context handles enter the portable dependency closure.
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar proof publication retains complete source identities"
)]
pub(crate) fn dependencies(
    cas: &Cas,
    contract: &RationalEnclosure,
    edges: &mut Vec<(LogicalId, Edge)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let EnclosureOrigin::ScalarKernel(certificate) = &contract.origin else {
        return Ok(());
    };
    if certificate.version < 3 {
        return Ok(());
    }
    for record in &certificate.records {
        budget.work(1, AT)?;
        if let ScalarKernelLaw::Reading(value) = &record.law {
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(edges.len().saturating_add(1), AT)?.saturating_mul(32),
                AT,
            )?;
            budget.work(1, AT)?;
            edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            edges.push((cas.checked(&value.definition)?, Edge::DependsOn));
        }
        if let ScalarKernelLaw::Family(input) = &record.law {
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(edges.len().saturating_add(3), AT)?.saturating_mul(32),
                AT,
            )?;
            budget.work(3, AT)?;
            edges.try_reserve(3).map_err(|_| Fault::allocation(AT))?;
            for handle in [
                &input.family.source,
                &input.family.premises[0].context,
                &input.family.premises[1].context,
            ] {
                edges.push((cas.checked(handle)?, Edge::DependsOn));
            }
        }
    }
    Ok(())
}
