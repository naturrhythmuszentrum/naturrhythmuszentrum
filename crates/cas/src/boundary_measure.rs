// SPDX-License-Identifier: AGPL-3.0-only
//! Bound untrusted boundary payloads before copying, encoding or doing exact arithmetic.

use crate::boundary_model::BoundaryCertificate;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::graph::{Cas, size};
use crate::proof_model::{Certificate, Statement};

/// Boundary preflight location.
const AT: Location = Location::new("boundary evidence preflight");

/// Direct application admission and portable replay enforce the same order ceiling.
pub(crate) fn statement(value: &Statement, budget: &Budget) -> Result<(), Fault> {
    let order = match value {
        Statement::CaseDerivative(value) => Some(value.order),
        Statement::DerivativeAt(value) => Some(value.order),
        Statement::DerivativeDoesNotExist(value) => Some(value.order),
        Statement::TraceValue(value) => Some(value.order),
        Statement::Jump { order, .. } => Some(*order),
        Statement::Jet { values, .. } => {
            let order = values
                .len()
                .checked_sub(1)
                .ok_or_else(crate::case_syntax::invalid)?;
            budget.check(BudgetKind::JetOrder, size(order, AT)?, AT)?;
            None
        }
        _ => None,
    };
    if let Some(order) = order {
        budget.check(BudgetKind::JetOrder, u64::from(order), AT)?;
    }
    Ok(())
}

/// Certificate references are checked even when a hostile proposal changes its statement family.
pub(crate) fn certificate(
    cas: &Cas,
    certificate: &Certificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Certificate::Boundary(value) = certificate else {
        return Ok(());
    };
    match value.as_ref() {
        BoundaryCertificate::Partition { regions, partition } => {
            budget.check(BudgetKind::RegionNodes, size(regions.len(), AT)?, AT)?;
            crate::region_check::measure(partition, budget)?;
            for region in regions {
                cas.state.region(cas.checked(region)?)?;
            }
        }
        BoundaryCertificate::Polynomial {
            polynomial,
            coefficients,
        } => {
            cas.state.expression(cas.checked(polynomial)?)?;
            crate::univariate::validate(coefficients, budget)?;
            let bytes = crate::univariate::payload(coefficients, budget)?;
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        }
        _ => {}
    }
    Ok(())
}
