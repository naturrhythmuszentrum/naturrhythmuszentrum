// SPDX-License-Identifier: AGPL-3.0-only
//! Cover elimination of mechanically restricted and explicitly closed local conclusions.

use crate::budget::Budget;
use crate::case_model::CaseCertificate;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::rule::Primitive;

/// Verify every closed member, including singleton regions and all retained local conditions.
pub(crate) fn reconstruct(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<CaseCertificate, Fault> {
    let domain = crate::context_check::domain(&draft.conclusion).ok_or_else(invalid)?;
    if matches!(
        draft.conclusion,
        Statement::DefinedDomain { .. } | Statement::Elaboration { .. }
    ) {
        return Err(invalid());
    }
    let context = cas.state.context(cas.checked(&draft.context)?)?;
    crate::case_check::subset(cas, cas.checked(domain)?, context.admissible, budget)?;
    let cover = premises.first().ok_or_else(invalid)?;
    let Statement::Cover {
        domain: covered,
        members,
    } = &cover.judgment.statement
    else {
        return Err(invalid());
    };
    if covered != domain
        || cover.judgment.context != draft.context
        || premises.len() != members.len().saturating_add(1)
    {
        return Err(invalid());
    }
    let mut certificate = CaseCertificate {
        regions: members.clone(),
        ..CaseCertificate::default()
    };
    for (member, child) in members.iter().zip(premises.iter().skip(1)) {
        if child.rule != Primitive::ContextClose || child.judgment.context != draft.context {
            return Err(invalid());
        }
        let Certificate::Close { frame } = &child.certificate else {
            return Err(invalid());
        };
        let local = cas.state.context(cas.checked(frame)?)?;
        if local.parent != Some(cas.checked(&draft.context)?)
            || local.restriction != Some(cas.checked(member)?)
        {
            return Err(invalid());
        }
        crate::case_check::subset(cas, local.admissible, cas.checked(domain)?, budget)?;
        let expected =
            crate::context_check::restricted(&draft.conclusion, &cas.handle(local.admissible));
        if child.judgment.statement != expected {
            return Err(invalid());
        }
        for residual in &child.conditions.residuals {
            if residual.home != draft.context {
                return Err(invalid());
            }
            crate::case_check::subset(
                cas,
                cas.checked(&residual.region)?,
                local.admissible,
                budget,
            )?;
        }
        certificate.frames.push(frame.clone());
    }
    Ok(certificate)
}
