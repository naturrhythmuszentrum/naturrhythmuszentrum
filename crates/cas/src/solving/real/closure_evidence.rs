// SPDX-License-Identifier: AGPL-3.0-only
//! Proof payload references supplement structural graph edges in the Real closure.

use super::AT;
use crate::budget::Budget;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::CheckedApplication;
use crate::solving::coefficients::storage::{reserve, vector};

/// The current certificate encoder visits the same semantic references used by portable replay.
fn certificate(
    application: &CheckedApplication,
    roots: &mut Vec<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = Encoder::new(budget, AT);
    crate::statement::certificate(&application.certificate, &mut out, |handle, out| {
        // Reserve the copied handle footprint before retaining its reference.
        out.raw(&[0; 64])?;
        roots.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        roots.push(handle.clone());
        Ok(())
    })
}

/// A checked premise retains its statement, certificate, substitutions, conditions and all earlier proof roots.
pub(super) fn roots(
    cas: &Cas,
    application: &CheckedApplication,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let objects = crate::statement::objects(&application.judgment.statement);
    let mut roots = vector(objects.len().saturating_add(1), budget)?;
    roots.push(application.judgment.context.clone());
    roots.extend(objects.into_iter().cloned());
    reserve(&mut roots, application.premises.len(), budget)?;
    roots.extend(application.premises.iter().map(|id| cas.handle(*id)));
    reserve(&mut roots, application.substitutions.len(), budget)?;
    roots.extend(
        application
            .substitutions
            .iter()
            .map(|value| value.replacement.clone()),
    );
    for guarded in application
        .conditions
        .requirements
        .iter()
        .chain(&application.judgment.residuals)
    {
        reserve(&mut roots, 3, budget)?;
        roots.extend([
            guarded.home.clone(),
            guarded.region.clone(),
            guarded.predicate.clone(),
        ]);
    }
    certificate(application, &mut roots, budget)?;
    Ok(roots)
}
