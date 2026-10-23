// SPDX-License-Identifier: AGPL-3.0-only
//! Empty-region DAGs enter the existing complete-family-piece and cover vocabulary.

use crate::Handle;
use crate::domain::{ContextDraft, RegionDraft, RegionExpr};
use crate::limits::{Budget, Limits};
use crate::proof::{
    ApplicationDraft, Certificate, EmptinessDag, EmptyEvidence, EmptyNode, Statement,
    VerifiedWitness,
};
use crate::request::Task;
use crate::solving::{FamilyCoverGroup, FamilyCoverRequest, SolveReason};
use crate::syntax::Draft;
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, reference};

mod limits;
mod local;

fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::blind_cubic())?)
}

fn setup() -> TestResult<(Fixture, Handle, Handle, Vec<ApplicationDraft>)> {
    let (mut fixture, request) = super::request(2)?;
    let Task::Solve(specification) = &request.task else {
        return Err("solve request".into());
    };
    let problem = specification.problem.clone();
    let original =
        crate::solving::graph::problem(&fixture.cas.state, problem.logical_id())?.clone();
    let original_region = fixture
        .cas
        .state
        .region(original.region.logical_id())?
        .clone();
    let over = original_region.lies_over.ok_or("family region")?;
    let family = fixture.cas.handle(over.family);
    let stratum = fixture.cas.handle(over.stratum);
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: original_region.ambient,
                expression: RegionExpr::Empty,
            },
            lies_over: crate::solving::LiesOver {
                family: reference(&family),
                stratum: reference(&stratum),
            },
        },
    )?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&region),
            assumptions: Vec::new(),
        }),
    )?;
    let leaf = ApplicationDraft {
        rule: crate::rule::Primitive::RegionConstruct,
        context: context.clone(),
        conclusion: Statement::Empty {
            region: region.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let dag = ApplicationDraft {
        rule: crate::rule::Primitive::RegionDecide,
        context,
        conclusion: leaf.conclusion.clone(),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Emptiness(Box::new(EmptinessDag {
            nodes: vec![EmptyNode {
                region,
                evidence: EmptyEvidence::Lemma { premise: 0 },
            }],
        })),
    };
    Ok((fixture, problem, request.context, vec![leaf, dag]))
}

fn empty_witness(fixture: &mut Fixture, steps: &[ApplicationDraft]) -> TestResult<VerifiedWitness> {
    let context = steps.last().ok_or("root")?.context.clone();
    Ok(fixture.cas.check_proof(&context, steps, &mut ledger()?)?)
}

/// A compact empty total region remains an explicit original family piece when a base cover is pending.
/// ´test:crate:family-emptiness-piece-retained-and-replayed´
#[test]
fn family_emptiness_piece_retained_and_replayed() -> TestResult {
    let (mut fixture, problem, parent, steps) = setup()?;
    let witness = empty_witness(&mut fixture, &steps)?;
    let piece =
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut ledger()?, &mut ledger()?)?;
    let bytes = fixture
        .cas
        .export_witness(&piece.coverage, &mut ledger()?)?;
    let imported = crate::Cas::new(Limits::blind_cubic())?.inspect(&bytes, &mut ledger()?)?;
    assert!(imported.is_discharged());
    let request = FamilyCoverRequest {
        context: parent,
        problem,
        base_cover: None,
        groups: vec![FamilyCoverGroup {
            context: witness.judgment().context.clone(),
            pieces: vec![piece.coverage],
            cover: None,
        }],
    };
    let result = fixture
        .cas
        .family_cover(&request, &mut ledger()?, &mut ledger()?)?;
    assert_eq!(result.pending, Some(SolveReason::BaseCoveragePending));
    assert_eq!(result.pieces.len(), 1);
    assert_eq!(
        result.pieces.first().ok_or("retained piece")?.descriptor,
        piece.descriptor
    );
    Ok(())
}

/// A changed predicate, region or missing premise cannot justify an empty original solution set.
/// ´test:crate:family-emptiness-coverage-mutations-refuse´
#[test]
fn family_emptiness_coverage_mutations_refuse() -> TestResult {
    let (mut fixture, problem, _, prefix) = setup()?;
    let witness = empty_witness(&mut fixture, &prefix)?;
    let piece =
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut ledger()?, &mut ledger()?)?;
    let checked = crate::proof_view::application(
        &fixture.cas,
        piece.coverage.proof().logical_id(),
        &mut ledger()?,
    )?;
    let final_step = ApplicationDraft {
        rule: checked.rule,
        context: checked.judgment.context,
        conclusion: checked.judgment.statement,
        premises: vec![1],
        substitutions: Vec::new(),
        certificate: checked.certificate,
    };
    for change in [0, 1, 2] {
        let mut altered = final_step.clone();
        let Certificate::Solve(certificate) = &mut altered.certificate else {
            return Err("solve certificate".into());
        };
        let crate::solving::SolveCertificate::TotalPredicateSplit(value) = certificate.as_mut()
        else {
            return Err("split certificate".into());
        };
        if change == 0 {
            value.expression.clone_from(&fixture.real);
        } else if change == 1 {
            value.cover = None;
        } else {
            value.region.clone_from(
                &crate::solving::graph::problem(&fixture.cas.state, problem.logical_id())?.region,
            );
        }
        let mut steps = prefix.clone();
        steps.push(altered);
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .check_proof(&final_step.context, &steps, &mut ledger()?)
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Empty-piece proposal and independent checking retain separate Work and byte ledgers.
/// ´test:crate:family-emptiness-resource-measurements´
#[test]
fn family_emptiness_resource_measurements() -> TestResult {
    let (mut fixture, problem, _, steps) = setup()?;
    let witness = empty_witness(&mut fixture, &steps)?;
    let mut generation = ledger()?;
    let mut checking = ledger()?;
    let (result, production_rows) = crate::testing::work_trace::measured(|| {
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut generation, &mut checking)
    })?;
    let piece = result?;
    let bytes = fixture
        .cas
        .export_witness(&piece.coverage, &mut ledger()?)?;
    let mut replay = ledger()?;
    let mut target = crate::Cas::new(Limits::blind_cubic())?;
    let (result, replay_rows) =
        crate::testing::work_trace::measured(|| target.inspect(&bytes, &mut replay))?;
    assert!(result?.is_discharged());
    assert_eq!(
        [
            generation.usage().work,
            checking.usage().work,
            replay.usage().work,
            u64::try_from(bytes.len())?
        ],
        [4_155, 73_370, 200_040, 1_775]
    );
    assert_eq!(
        production_rows.iter().map(|row| row.work).sum::<u64>(),
        generation.usage().work + checking.usage().work
    );
    assert_eq!(
        replay_rows.iter().map(|row| row.work).sum::<u64>(),
        replay.usage().work
    );
    let production_rules: u64 = production_rows
        .iter()
        .filter(|row| row.rule != "replay")
        .map(|row| row.work)
        .sum();
    let replay_rules: u64 = replay_rows
        .iter()
        .filter(|row| row.rule != "replay")
        .map(|row| row.work)
        .sum();
    assert_eq!(
        (production_rules, replay_rules),
        (703, 2_550),
        "production={production_rows:?}; replay={replay_rows:?}"
    );
    Ok(())
}
