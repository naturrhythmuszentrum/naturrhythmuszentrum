// SPDX-License-Identifier: AGPL-3.0-only
//! Earlier closed presentations preserve exact branch identities across coefficient strata.

use super::super::evaluate::Evaluation;
use super::super::proof::{AT, invalid, premise};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::storage::vector;
use crate::solving::{
    CertificateRootSelectionRefinement, SolutionSetData, SolveDraft, SolveForm, SolveStatement,
};

/// Composition retains a separately checked complete cover of the exact target set.
fn coverage(
    evaluation: &Evaluation<'_>,
    context: &Handle,
    target: &Handle,
    problem: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for proof in evaluation.premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &proof.judgment.statement
            && let SolveStatement::Coverage(claim) = statement.as_ref()
            && (
                &proof.judgment.context,
                &claim.problem,
                &claim.set,
                &claim.region,
            ) == (context, problem, target, evaluation.region)
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Child statements have already discharged local root obligations before context closing.
fn child<'a>(
    evaluation: &Evaluation<'a>,
    context: &Handle,
    target: &SolutionSetData<Handle>,
    form: SolveForm,
    proof: &CheckedApplication,
    budget: &mut Budget,
) -> Result<&'a SolutionSetData<Handle>, Fault> {
    let Statement::Solve(statement) = &proof.judgment.statement else {
        return Err(invalid());
    };
    let SolveStatement::Presentation(claim) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&proof.judgment.context, claim.form) != (context, form) {
        return Err(invalid());
    }
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&evaluation.cas.state, evaluation.cas.checked(&claim.set)?)?
    else {
        return Err(invalid());
    };
    if set.problem != target.problem {
        return Err(invalid());
    }
    super::super::member::contained(evaluation.cas, &claim.region, evaluation.region, budget)?;
    for handle in &set.branches {
        budget.work(1, AT)?;
        let SolveDraft::SolutionBranch(branch) =
            crate::solving::graph::object(&evaluation.cas.state, evaluation.cas.checked(handle)?)?
        else {
            return Err(invalid());
        };
        if branch.problem != target.problem {
            return Err(invalid());
        }
        super::super::member::contained(evaluation.cas, &branch.region, &claim.region, budget)?;
    }
    Ok(set)
}

/// Every target branch is supplied verbatim; coverage never grants a representation by itself.
pub(super) fn check(
    evaluation: &Evaluation<'_>,
    draft: &ApplicationDraft,
    certificate: &CertificateRootSelectionRefinement,
    target: &SolutionSetData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    coverage(
        evaluation,
        &draft.context,
        &certificate.target,
        &target.problem,
        budget,
    )?;
    let form = certificate.form.ok_or_else(invalid)?;
    let mut covered = vector(target.branches.len(), budget)?;
    for slot in &certificate.compatibility {
        budget.work(1, AT)?;
        let set = child(
            evaluation,
            &draft.context,
            target,
            form,
            premise(evaluation.premises, *slot)?,
            budget,
        )?;
        for branch in &set.branches {
            budget.work(crate::graph::size(target.branches.len(), AT)?, AT)?;
            if !target.branches.contains(branch) {
                return Err(invalid());
            }
            budget.work(crate::graph::size(covered.len(), AT)?, AT)?;
            if !covered.contains(branch) {
                covered.push(branch.clone());
            }
        }
    }
    if covered.len() != target.branches.len() {
        return Err(invalid());
    }
    Ok(())
}
