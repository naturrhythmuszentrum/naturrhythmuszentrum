// SPDX-License-Identifier: AGPL-3.0-only
//! Separate base and fibre covers preserve every complete piece.

use super::{request, stratum};
use crate::Handle;
use crate::domain::ContextDraft;
use crate::limits::{Budget, Limits};
use crate::proof::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::request::{Outcome, Task};
use crate::solving::{
    CertificateTotalPredicateSplit, FamilyCoverGroup, FamilyCoverRequest, PredicateSplitMode,
    SolveCertificate, SolveReason, SolveSchema,
};
use crate::syntax::Draft;
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, reference};

/// Check a singleton fibre cover through the existing region constructor.
fn fibre_cover(fixture: &mut Fixture, context: &Handle) -> TestResult<VerifiedWitness> {
    let domain = fixture
        .cas
        .handle(fixture.cas.state.context(context.logical_id())?.admissible);
    let draft = ApplicationDraft {
        rule: crate::rule::Primitive::RegionConstruct,
        context: context.clone(),
        conclusion: Statement::Cover {
            domain: domain.clone(),
            members: vec![domain],
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    Ok(fixture
        .cas
        .check_proof(context, &[draft], &mut Budget::new(Limits::blind_cubic())?)?)
}

/// Use the existing Real trichotomy with the exact three original base strata.
fn base_cover(fixture: &mut Fixture, groups: &[FamilyCoverGroup]) -> TestResult<VerifiedWitness> {
    let mut members = Vec::new();
    for group in groups {
        let frame = fixture.cas.state.context(group.context.logical_id())?;
        let over = fixture
            .cas
            .state
            .region(frame.admissible)?
            .lies_over
            .as_ref()
            .ok_or("missing stratum")?;
        members.push(fixture.cas.handle(over.stratum));
    }
    let first = members.first().ok_or("no strata")?;
    let crate::domain::RegionExpr::GuardTrue { base, .. } =
        fixture.cas.state.region(first.logical_id())?.expression
    else {
        return Err("base guard".into());
    };
    let domain = fixture.cas.handle(base);
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&domain),
            assumptions: Vec::new(),
        }),
    )?;
    let draft = ApplicationDraft {
        rule: crate::rule::Primitive::Solve(SolveSchema::TotalPredicateSplit),
        context: context.clone(),
        conclusion: Statement::Cover {
            domain: domain.clone(),
            members: members.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::TotalPredicateSplit(
            CertificateTotalPredicateSplit {
                mode: PredicateSplitMode::RealSigns,
                expression: fixture.real.clone(),
                region: domain,
                children: members,
                branches: Vec::new(),
                cover: None,
            },
        ))),
    };
    Ok(fixture
        .cas
        .check_proof(&context, &[draft], &mut Budget::new(Limits::blind_cubic())?)?)
}

/// Solve each original stratum in one graph, without replacing the unspecialized problem.
fn pieces() -> TestResult<(Fixture, FamilyCoverRequest)> {
    let (mut fixture, original) = request(2)?;
    let Task::Solve(specification) = &original.task else {
        return Err("solve task".into());
    };
    let mut groups = Vec::new();
    for sign in [-1, 0, 1] {
        let mut local = original.clone();
        stratum(&mut fixture, &mut local, sign)?;
        let Outcome::Solved { answer, .. } = fixture.cas.run(&local) else {
            return Err(format!("stratum {sign} incomplete").into());
        };
        let coverage = answer.coverage.ok_or("missing complete piece")?;
        let cover = fibre_cover(&mut fixture, &local.context)?;
        groups.push(FamilyCoverGroup {
            context: local.context,
            pieces: vec![coverage],
            cover: Some(cover),
        });
    }
    let base = base_cover(&mut fixture, &groups)?;
    Ok((
        fixture,
        FamilyCoverRequest {
            context: original.context,
            problem: specification.problem.clone(),
            base_cover: Some(base),
            groups,
        },
    ))
}

/// Missing either enclosing cover retains all independently complete local answers.
/// ´test:crate:mixed-family-cover-pending-retains-pieces´
#[test]
fn mixed_family_cover_pending_retains_pieces() -> TestResult {
    let (mut fixture, mut request) = pieces()?;
    let base = request.base_cover.take().ok_or("base proof")?;
    let result = fixture.cas.family_cover(
        &request,
        &mut Budget::new(Limits::blind_cubic())?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(result.pending, Some(SolveReason::BaseCoveragePending));
    assert_eq!(result.pieces.len(), 3);
    request.base_cover = Some(base);
    request.groups[0].cover = None;
    let result = fixture.cas.family_cover(
        &request,
        &mut Budget::new(Limits::blind_cubic())?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(result.pending, Some(SolveReason::FibreCoveragePending));
    assert_eq!(result.pieces.len(), 3);
    Ok(())
}

/// Both independent covers expand into a fresh independently replayable whole theorem.
/// ´test:crate:mixed-family-full-cover-replays´
#[test]
fn mixed_family_full_cover_replays() -> TestResult {
    let (mut fixture, request) = pieces()?;
    let result = fixture.cas.family_cover(
        &request,
        &mut Budget::new(Limits::blind_cubic())?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(result.pending, None);
    assert_eq!(result.pieces.len(), 3);
    let coverage = result.coverage.ok_or("missing whole coverage")?;
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(
        crate::Cas::new(Limits::blind_cubic())?
            .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
            .is_discharged()
    );
    Ok(())
}

/// Removing the zero stratum prevents a whole cover while preserving both nonzero pieces.
/// ´test:crate:mixed-family-omitted-zero-keeps-pieces´
#[test]
fn mixed_family_omitted_zero_keeps_pieces() -> TestResult {
    let (mut fixture, mut request) = pieces()?;
    incomplete_base_cover(
        &mut fixture,
        request.base_cover.as_ref().ok_or("base cover")?,
    )?;
    assert_eq!(request.groups.len(), 3);
    let original = request
        .groups
        .iter()
        .map(|group| group.context.clone())
        .collect::<Vec<_>>();
    let removed = request.groups.remove(1);
    assert_eq!(removed.context, original[1]);
    assert_ne!(
        request
            .groups
            .iter()
            .map(|group| group.context.clone())
            .collect::<Vec<_>>(),
        original
    );
    let result = fixture.cas.family_cover(
        &request,
        &mut Budget::new(Limits::blind_cubic())?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(result.pending, Some(SolveReason::BaseCoveragePending));
    assert_eq!(result.pieces.len(), 2);
    assert!(result.coverage.is_none());
    Ok(())
}

/// A proved empty solution piece is retained separately from a placement refusal.
/// ´test:crate:mixed-family-empty-piece-retained´
#[test]
fn mixed_family_empty_piece_retained() -> TestResult {
    let (mut fixture, request) = request(5)?;
    let Task::Solve(specification) = &request.task else {
        return Err("solve task".into());
    };
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("empty piece {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("empty coverage")?;
    let crate::proof::Statement::Solve(statement) = &coverage.judgment().statement else {
        return Err("coverage statement".into());
    };
    let crate::solving::SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("coverage claim".into());
    };
    let crate::solving::SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&fixture.cas.state, claim.set.logical_id())?
    else {
        return Err("empty set".into());
    };
    assert_eq!(set.branches.len(), 1);
    let crate::solving::SolveDraft::SolutionBranch(branch) =
        crate::solving::graph::object(&fixture.cas.state, set.branches[0].logical_id())?
    else {
        return Err("empty branch".into());
    };
    assert_eq!(branch.kind, crate::solving::SolutionBranchKind::Empty);
    let cover = FamilyCoverRequest {
        context: request.context.clone(),
        problem: specification.problem.clone(),
        base_cover: None,
        groups: vec![FamilyCoverGroup {
            context: request.context,
            pieces: vec![coverage],
            cover: None,
        }],
    };
    let result = fixture.cas.family_cover(
        &cover,
        &mut Budget::new(Limits::blind_cubic())?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(result.pending, Some(SolveReason::BaseCoveragePending));
    assert_eq!(result.pieces.len(), 1);
    Ok(())
}

/// Omitting the zero member from both stated inventory and certificate still fails Real trichotomy.
fn incomplete_base_cover(fixture: &mut Fixture, proof: &VerifiedWitness) -> TestResult {
    let mut changed = crate::testing::solve_blind::family_applications(&fixture.cas, proof)?;
    let step = changed.last_mut().ok_or("base cover application")?;
    let Statement::Cover { members, .. } = &mut step.conclusion else {
        return Err("base inventory".into());
    };
    let original = members.clone();
    assert_eq!(original.len(), 3);
    let removed = members.remove(1);
    assert_eq!(removed, original[1]);
    assert_ne!(*members, original);
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("split certificate".into());
    };
    let SolveCertificate::TotalPredicateSplit(split) = certificate.as_mut() else {
        return Err("base split".into());
    };
    assert_eq!(split.children, original);
    split.children.remove(1);
    assert_ne!(split.children, original);
    assert!(
        fixture
            .cas
            .check_proof(
                &proof.judgment().context,
                &changed,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}
