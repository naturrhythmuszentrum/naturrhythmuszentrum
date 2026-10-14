// SPDX-License-Identifier: AGPL-3.0-only
//! Specialized certificate limits are checked before untrusted payload copies.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::{DistributionCertificate, DistributionStatement};
use crate::distribution_syntax::AT;
use crate::error::Fault;
use crate::graph::{Cas, size};
use crate::proof_model::{Certificate, Statement};

/// Bound every distribution statement and certificate vector before replay or publication.
pub(crate) fn measure(
    cas: &Cas,
    statement: &Statement,
    certificate: &Certificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if let Statement::Distribution(value) = statement {
        cas.checked(value.space())?;
        if let DistributionStatement::Derivative { order, .. } = value.as_ref() {
            budget.check(BudgetKind::Depth, u64::from(*order), AT)?;
        }
    }
    let Certificate::Distribution(value) = certificate else {
        return Ok(());
    };
    match value.as_ref() {
        DistributionCertificate::Regular(value) => {
            budget.check(BudgetKind::RegionNodes, size(value.regions.len(), AT)?, AT)?;
            budget.check(
                BudgetKind::RegionCells,
                size(value.polynomials.len(), AT)?,
                AT,
            )?;
            crate::region_check::measure(&value.partition, budget)?;
            for region in &value.regions {
                cas.state.region(cas.checked(region)?)?;
            }
            for coefficients in value.polynomials.iter().flatten() {
                crate::univariate::validate(coefficients, budget)?;
            }
        }
        DistributionCertificate::AlmostEverywhere {
            exceptions,
            partition,
        } => {
            points(exceptions, budget)?;
            crate::region_check::measure(partition, budget)?;
        }
        DistributionCertificate::Support(values) => points(values, budget)?,
    }
    Ok(())
}

/// Support and exception sets have independent count and rational component bounds.
fn points(values: &[crate::number::Rational], budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::BoundaryNodes, size(values.len(), AT)?, AT)?;
    for value in values {
        value.check(budget, AT)?;
    }
    Ok(())
}
