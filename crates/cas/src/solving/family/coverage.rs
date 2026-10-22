// SPDX-License-Identifier: AGPL-3.0-only
//! The family producer records repeated identical factors with their exact multiplicity.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::Certificate;
use crate::solving::radicals::{construction::Construction, roots::Values};
use crate::solving::towers::dense::Dense;
use crate::solving::{RootFactor, SolveCertificate};

/// Reuse complete product construction, then consolidate only identical original factor handles.
pub(crate) fn emit(
    cas: &mut Cas,
    construction: &mut Construction,
    problem: &Handle,
    polynomial: &Dense,
    values: &Values,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let set = crate::solving::radicals::proof::coverage(
        cas,
        construction,
        problem,
        polynomial,
        values,
        budget,
    )?;
    let step = construction.steps.last_mut().ok_or_else(super::invalid)?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err(super::invalid());
    };
    let SolveCertificate::ProductCompleteness(value) = certificate.as_mut() else {
        return Err(super::invalid());
    };
    let mut factors: Vec<RootFactor<Handle>> =
        crate::solving::coefficients::storage::vector(value.factors.len(), budget)?;
    for original in &value.factors {
        budget.work(
            crate::graph::size(factors.len().saturating_add(1), super::AT)?,
            super::AT,
        )?;
        if let Some(factor) = factors
            .iter_mut()
            .find(|factor| factor.root == original.root)
        {
            factor.multiplicity = factor
                .multiplicity
                .checked_add(original.multiplicity)
                .ok_or_else(super::invalid)?;
        } else {
            factors.push(original.clone());
        }
    }
    value.factors = factors;
    Ok(set)
}
