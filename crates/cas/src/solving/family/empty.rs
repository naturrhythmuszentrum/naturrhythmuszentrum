// SPDX-License-Identifier: AGPL-3.0-only
//! A checked empty total region becomes the same retained family-piece vocabulary as root coverage.

use super::cover::FamilyPiece;
use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::proof_model::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::solving::{
    CertificateTotalPredicateSplit, FibrePieceData, PredicateSplitMode, SolutionBranchData,
    SolutionBranchKind, SolutionSetData, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    StatementCoverage,
};

impl Cas {
    /// Retain an independently proved empty total region as a complete original family piece.
    ///
    /// The resulting coverage cites the empty-region proof and can be supplied directly
    /// to `family_cover`; the original problem, domain, family and stratum remain explicit.
    /// # Errors
    /// Refuses a foreign or conditional proof, changed context, non-family region,
    /// unrelated original problem and exhausted generation or independent checker resources.
    pub fn family_empty_piece(
        &mut self,
        problem: &Handle,
        empty: &VerifiedWitness,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<FamilyPiece, Fault> {
        self.proof_limits(checker, |cas, checker| {
            cas.transaction(generation, |cas, generation| {
                produce(cas, problem, empty, generation, checker)
            })
        })
    }
}

/// Branch syntax names the original domain even though its proved region has no assignments.
fn empty_set(
    cas: &mut Cas,
    problem: &Handle,
    region: &Handle,
    generation: &mut Budget,
) -> Result<Handle, Fault> {
    let original = crate::solving::elementary::proof::problem(cas, problem)?;
    let branch = SolveDraft::SolutionBranch(SolutionBranchData {
        problem: Reference::Existing(problem.clone()),
        region: Reference::Existing(region.clone()),
        domain: Reference::Existing(original.domain.clone()),
        kind: SolutionBranchKind::Empty,
    });
    let branch = crate::solving::towers::materialize::admit(cas, branch, generation)?;
    crate::solving::towers::materialize::admit(
        cas,
        SolveDraft::SolutionSet(SolutionSetData {
            problem: Reference::Existing(problem.clone()),
            branches: vec![Reference::Existing(branch)],
        }),
        generation,
    )
}

/// The independent checker consumes the graph-owned premise, never a cached caller view.
fn produce(
    cas: &mut Cas,
    problem: &Handle,
    empty: &VerifiedWitness,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<FamilyPiece, Fault> {
    let premise = crate::proof_view::application(cas, cas.checked(empty.proof())?, generation)?;
    if !premise.conditions.residuals.is_empty() {
        return Err(invalid());
    }
    let Statement::Empty { region } = &premise.judgment.statement else {
        return Err(invalid());
    };
    let over = cas
        .state
        .region(cas.checked(region)?)?
        .lies_over
        .clone()
        .ok_or_else(invalid)?;
    let predicate = crate::solving::elementary::proof::problem(cas, problem)?
        .predicate
        .clone();
    let set = empty_set(cas, problem, region, generation)?;
    let draft = coverage_draft(problem, region, &set, &predicate, &premise.judgment.context);
    let region = region.clone();
    let mut application = crate::scoped_check::check_application(cas, &draft, &[premise], checker)?;
    application.premises = vec![cas.checked(empty.proof())?];
    let proof = cas.publish_scoped(application, checker)?;
    let coverage = cas.seal_scoped(&proof, checker)?;
    let family = cas.handle(over.family);
    let stratum = cas.handle(over.stratum);
    let descriptor = crate::solving::towers::materialize::admit(
        cas,
        SolveDraft::FibrePiece(FibrePieceData {
            family: Reference::Existing(family),
            stratum: Reference::Existing(stratum),
            region: Reference::Existing(region),
            problem: Reference::Existing(problem.clone()),
            set: Reference::Existing(set),
        }),
        generation,
    )?;
    Ok(FamilyPiece {
        descriptor,
        coverage,
    })
}

/// Empty coverage retains the exact original predicate and its one checked region premise.
fn coverage_draft(
    problem: &Handle,
    region: &Handle,
    set: &Handle,
    predicate: &Handle,
    context: &Handle,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: crate::rule::Primitive::Solve(SolveSchema::TotalPredicateSplit),
        context: context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: region.clone(),
        }))),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
            CertificateTotalPredicateSplit {
                mode: PredicateSplitMode::EmptyRegion,
                expression: predicate.clone(),
                region: region.clone(),
                children: Vec::new(),
                branches: Vec::new(),
                cover: Some(0),
            },
        ))),
    }
}
