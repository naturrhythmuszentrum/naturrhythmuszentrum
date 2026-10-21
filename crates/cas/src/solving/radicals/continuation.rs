// SPDX-License-Identifier: AGPL-3.0-only
//! Extend checked coverage through ordinary checked graph premises in the same request.

use super::construction::Construction;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::proof_model::{ApplicationDraft, CheckedApplication, VerifiedWitness};
use crate::solving::coefficients::storage::{reserve, vector};

/// Stable attribution for extending a checked radical proof.
const AT: Location = Location::new("radical presentation continuation");

/// Changed drafts cannot borrow the authority of a different checked application.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Every supplied application remains reachable through strictly backward references.
fn reachable(steps: &[ApplicationDraft], budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::ProofNodes, size(steps.len(), AT)?, AT)?;
    let mut reached = vector(steps.len(), budget)?;
    reached.resize(steps.len(), false);
    let mut pending = vector(1, budget)?;
    pending.push(steps.len().checked_sub(1).ok_or_else(invalid)?);
    while let Some(index) = pending.pop() {
        budget.work(1, AT)?;
        let visited = reached.get_mut(index).ok_or_else(invalid)?;
        if *visited {
            continue;
        }
        *visited = true;
        let step = steps.get(index).ok_or_else(invalid)?;
        if !step.rule.specification().available {
            return Err(invalid());
        }
        reserve(&mut pending, step.premises.len(), budget)?;
        for previous in &step.premises {
            let previous = usize::try_from(*previous).map_err(|_| invalid())?;
            if previous >= index {
                return Err(invalid());
            }
            pending.push(previous);
        }
    }
    budget.work(size(reached.len(), AT)?, AT)?;
    if reached.iter().any(|entry| !entry) {
        return Err(invalid());
    }
    Ok(())
}

/// A stored node can replace only the identical rule, context, claim and auxiliary input.
fn matches(
    cas: &Cas,
    draft: &ApplicationDraft,
    proof: LogicalId,
    budget: &mut Budget,
) -> Result<CheckedApplication, Fault> {
    crate::proof_view::measure(cas, draft, budget)?;
    let application = crate::proof_view::application(cas, proof, budget)?;
    budget.check(BudgetKind::Depth, application.depth, AT)?;
    let actual = (
        &application.rule,
        &application.judgment.context,
        &application.judgment.statement,
        &application.substitutions,
        &application.certificate,
    );
    let expected = (
        &draft.rule,
        &draft.context,
        &draft.conclusion,
        &draft.substitutions,
        &draft.certificate,
    );
    if actual != expected || application.premises.len() != draft.premises.len() {
        return Err(invalid());
    }
    Ok(application)
}

/// Walk the sealed dependency closure, matching every original index and ordered premise edge.
fn prefix(
    cas: &Cas,
    steps: &[ApplicationDraft],
    coverage: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let root = steps.len().checked_sub(1).ok_or_else(invalid)?;
    let mut assigned = vector(steps.len(), budget)?;
    assigned.resize(steps.len(), None);
    let mut pending = vector(1, budget)?;
    pending.push((root, cas.checked(coverage.proof())?));
    while let Some((index, proof)) = pending.pop() {
        budget.work(1, AT)?;
        let slot = assigned.get_mut(index).ok_or_else(invalid)?;
        if let Some(previous) = slot {
            if *previous != proof {
                return Err(invalid());
            }
            continue;
        }
        *slot = Some(proof);
        let draft = steps.get(index).ok_or_else(invalid)?;
        let checked = matches(cas, draft, proof, budget)?;
        reserve(&mut pending, draft.premises.len(), budget)?;
        for (previous, stored) in draft.premises.iter().zip(checked.premises) {
            let previous = usize::try_from(*previous).map_err(|_| invalid())?;
            if previous >= index {
                return Err(invalid());
            }
            pending.push((previous, stored));
        }
    }
    let mut result = vector(assigned.len(), budget)?;
    for proof in assigned {
        result.push(cas.handle(proof.ok_or_else(invalid)?));
    }
    Ok(result)
}

/// New applications use the ordinary scoped checker and retain its complete condition provenance.
fn append(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    proofs: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut earlier = vector(draft.premises.len(), budget)?;
    let mut ids = vector(draft.premises.len(), budget)?;
    for index in &draft.premises {
        let proof = proofs
            .get(usize::try_from(*index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let id = cas.checked(proof)?;
        ids.push(id);
        earlier.push(crate::proof_view::application(cas, id, budget)?);
    }
    let mut application = crate::scoped_check::check_application(cas, draft, &earlier, budget)?;
    application.premises = ids;
    cas.publish_scoped(application, budget)
}

/// Coverage is already sealed; failed new obligations roll back without revoking it or refunding work.
pub(super) fn extend(
    cas: &mut Cas,
    construction: &Construction,
    count: usize,
    coverage: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    cas.proof_limits(budget, |cas, budget| {
        reachable(&construction.steps, budget)?;
        if count >= construction.steps.len()
            || construction.steps.last().ok_or_else(invalid)?.context != construction.context
        {
            return Err(invalid());
        }
        let mut proofs = prefix(
            cas,
            construction.steps.get(..count).ok_or_else(invalid)?,
            coverage,
            budget,
        )?;
        reserve(&mut proofs, construction.steps.len() - count, budget)?;
        cas.transaction(budget, |candidate, budget| {
            for draft in construction.steps.get(count..).ok_or_else(invalid)? {
                proofs.push(append(candidate, draft, &proofs, budget)?);
            }
            candidate.seal_scoped(proofs.last().ok_or_else(invalid)?, budget)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{Cas, Construction, VerifiedWitness};
    use crate::budget::Budget;
    use crate::limits::Limits;
    use crate::proof_model::{Certificate, Statement};
    use crate::rule::Primitive;
    use crate::solving::{SolveCertificate, SolveForm, SolveSchema, SolveStatement};
    use crate::testing::common::TestResult;

    type Prepared = (Cas, Construction, VerifiedWitness, usize);

    fn prepared() -> TestResult<Prepared> {
        let (mut fixture, request) = crate::testing::solve_blind::polynomial(&[1, 1])?;
        let mut generation = Budget::new(Limits::blind_cubic())?;
        let proposal = crate::solving::solve_search::propose(
            &mut fixture.cas,
            &request,
            &mut crate::solving::radicals::route::Audit::default(),
            &mut generation,
        )?;
        let mut construction = proposal.construction;
        let count = construction.steps.len();
        let coverage = fixture.cas.check_proof(
            &construction.context,
            &construction.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let Statement::Solve(claim) = &construction
            .steps
            .last()
            .ok_or("missing coverage")?
            .conclusion
        else {
            return Err("coverage kind".into());
        };
        let SolveStatement::Coverage(claim) = claim.as_ref() else {
            return Err("coverage claim".into());
        };
        let set = claim.set.clone();
        super::super::proof::present(
            &mut construction,
            &set,
            SolveForm::Radicals,
            Vec::new(),
            &mut generation,
        )?;
        Ok((fixture.cas, construction, coverage, count))
    }

    fn inspect(witness: &VerifiedWitness) -> TestResult {
        let mut fresh = Cas::new(Limits::blind_cubic())?;
        let checked = fresh.inspect(&witness.bytes, &mut Budget::new(Limits::blind_cubic())?)?;
        assert!(checked.is_discharged());
        assert_eq!(checked.bytes, witness.bytes);
        Ok(())
    }

    /// Reused checked premises still export a complete proof independently accepted in a fresh store.
    /// ´test:unit:continuation-fresh-presentation´
    #[test]
    fn continuation_fresh_presentation() -> TestResult {
        let (mut cas, construction, coverage, count) = prepared()?;
        let presentation = super::extend(
            &mut cas,
            &construction,
            count,
            &coverage,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert!(
            matches!(&presentation.judgment.statement, Statement::Solve(value)
            if matches!(value.as_ref(), SolveStatement::Presentation(value) if value.form == SolveForm::Radicals))
        );
        inspect(&coverage)?;
        inspect(&presentation)
    }

    /// Changed rules, premise indices and a different sealed root cannot impersonate the checked prefix.
    /// ´test:unit:continuation-prefix-mutations´
    #[test]
    fn continuation_prefix_mutations() -> TestResult {
        let (mut cas, mut construction, coverage, count) = prepared()?;
        let presentation = super::extend(
            &mut cas,
            &construction,
            count,
            &coverage,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let index = count.checked_sub(1).ok_or("empty prefix")?;
        let original = construction.steps[index].rule;
        assert_eq!(original, Primitive::Solve(SolveSchema::ProductCompleteness));
        let replacement = Primitive::Solve(SolveSchema::RootSelectionRefinement);
        assert_ne!(replacement, original);
        construction.steps[index].rule = replacement;
        assert!(
            super::extend(
                &mut cas,
                &construction,
                count,
                &coverage,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
        );
        construction.steps[index].rule = original;
        let original = *construction.steps[index]
            .premises
            .first()
            .ok_or("missing premise")?;
        assert_eq!(original, 0);
        let replacement = 1;
        assert_ne!(replacement, original);
        construction.steps[index].premises[0] = replacement;
        assert!(
            super::extend(
                &mut cas,
                &construction,
                count,
                &coverage,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
        );
        construction.steps[index].premises[0] = original;
        assert!(
            super::extend(
                &mut cas,
                &construction,
                count,
                &presentation,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
        );
        Ok(())
    }

    /// A false later obligation rolls back its valid new predecessor while retaining portable coverage.
    /// ´test:unit:continuation-failure-retains-coverage´
    #[test]
    fn continuation_failure_retains_coverage() -> TestResult {
        let (mut cas, mut construction, coverage, count) = prepared()?;
        let mut false_step = construction
            .steps
            .last()
            .ok_or("missing presentation")?
            .clone();
        false_step.premises.push(u32::try_from(count)?);
        let Certificate::Solve(certificate) = &mut false_step.certificate else {
            return Err("presentation certificate".into());
        };
        let SolveCertificate::RootSelectionRefinement(certificate) = certificate.as_mut() else {
            return Err("presentation refinement".into());
        };
        assert_eq!(certificate.form, Some(SolveForm::Radicals));
        let replacement = None;
        assert_ne!(replacement, certificate.form);
        certificate.form = replacement;
        construction.steps.push(false_step);
        let before = cas.state.proof_count();
        assert!(
            super::extend(
                &mut cas,
                &construction,
                count,
                &coverage,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
        );
        assert_eq!(cas.state.proof_count(), before);
        inspect(&coverage)
    }

    /// The measured final work unit seals presentation; one less retains only the original checked coverage.
    /// ´test:unit:continuation-work-boundary´
    #[test]
    fn continuation_work_boundary() -> TestResult {
        let (mut cas, construction, coverage, count) = prepared()?;
        let mut measured = Budget::new(Limits::blind_cubic())?;
        super::extend(&mut cas, &construction, count, &coverage, &mut measured)?;
        let required = measured.usage().work;
        for cap in [
            required,
            required.checked_sub(1).ok_or("empty continuation ledger")?,
        ] {
            let (mut cas, construction, coverage, count) = prepared()?;
            let before = cas.state.proof_count();
            let mut budget = Budget::new(Limits {
                work: cap,
                ..Limits::blind_cubic()
            })?;
            let result = super::extend(&mut cas, &construction, count, &coverage, &mut budget);
            if cap == required {
                inspect(&result?)?;
            } else {
                let error = result.err().ok_or("continuation exceeded work cap")?;
                assert!(matches!(error.kind, crate::FaultKind::Exhausted {
                    budget: crate::limits::BudgetKind::Work, limit, needed, ..
                } if limit == cap && needed > cap));
                assert_eq!(cas.state.proof_count(), before);
                inspect(&coverage)?;
            }
            assert!(budget.usage().work <= cap);
        }
        Ok(())
    }
}
