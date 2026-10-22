// SPDX-License-Identifier: AGPL-3.0-only
//! Complete Real acceptance uses one generation ledger and one independent replay ledger.

use crate::budget::{Budget, BudgetUsage, Limits};
use crate::graph::{Cas, Vertex};
use crate::model::{AtomicSort, Field, Sort};
use crate::proof_model::{Judgment, Statement};
use crate::solving::{
    SolveAnswer, SolveDraft, SolveForm, SolveStatement, StatementCoverage, StatementPresentation,
};
use crate::testing::common::TestResult;

/// The explicit request revision leaves every original declaration and occurrence intact.
pub(super) fn request() -> TestResult<(super::source::Fixture, crate::request_model::Request)> {
    let (fixture, mut request) =
        super::source::symbolic_in_field(&["d", "c", "b", "a"], &Field::Real)?;
    let crate::task::Task::Solve(value) = &mut request.task else {
        return Err("Real request".into());
    };
    assert_eq!(value.form, SolveForm::Radicals);
    value.form = SolveForm::AnyExact;
    Ok((fixture, request))
}

/// This independent walk inspects every imported dependency, including certificate-only references.
fn real_only(cas: &Cas) {
    for value in cas.state.graph.node_weights() {
        match value {
            Vertex::Expression(value) => {
                assert_ne!(value.sort, Sort::Value(AtomicSort::Scalar(Field::Complex)));
            }
            Vertex::Solve(value) => match value.as_ref() {
                SolveDraft::RootRelation(value) => assert_eq!(value.ambient, Field::Real),
                SolveDraft::CoordinateSpace(value) => assert!(
                    value
                        .coordinates
                        .iter()
                        .all(|entry| entry.field == Field::Real)
                ),
                SolveDraft::CoefficientMap(value) => assert!(!matches!(
                    &value.target,
                    crate::solving::MapTarget::ScalarAmbient {
                        field: Field::Complex,
                        ..
                    }
                )),
                SolveDraft::Polynomial(value) => {
                    assert!(!value.variables.contains(&Field::Complex));
                }
                SolveDraft::CoefficientDomain(value) => assert!(!matches!(
                    value,
                    crate::solving::CoefficientDomainData::Gaussian
                )),
                _ => {}
            },
            Vertex::Header(value) => assert!(
                value
                    .environment
                    .declarations
                    .iter()
                    .all(|entry| entry.sort != Sort::Value(AtomicSort::Scalar(Field::Complex)))
            ),
            _ => {}
        }
    }
}

/// Expected source, context and region are reconstructed before either theorem is imported.
pub(super) fn fresh(cas: &Cas, answer: &SolveAnswer) -> TestResult<(BudgetUsage, BudgetUsage)> {
    let (mut fixture, request) = request()?;
    let crate::task::Task::Solve(original) = &request.task else {
        return Err("Real fresh request".into());
    };
    let mut replay = Budget::new(Limits::blind_cubic())?;
    let coverage = answer.coverage.as_ref().ok_or("Real fresh coverage")?;
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let imported = fixture
        .cas
        .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("Real imported solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("Real imported coverage".into());
    };
    let set = claim.set.clone();
    let expected = Judgment {
        context: fixture.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: original.problem.clone(),
            set: set.clone(),
            region: fixture.region.clone(),
        }))),
    };
    replay_one(&mut fixture.cas, &bytes, &expected, &mut replay)?;
    let coverage_usage = replay.usage();
    let presentation = answer.facts.last().ok_or("Real fresh presentation")?;
    let bytes = cas.export_witness(presentation, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = Judgment {
        statement: Statement::Solve(Box::new(SolveStatement::Presentation(
            StatementPresentation {
                set,
                form: SolveForm::RealExact,
                region: fixture.region.clone(),
            },
        ))),
        ..expected
    };
    replay_one(&mut fixture.cas, &bytes, &expected, &mut replay)?;
    real_only(&fixture.cas);
    Ok((coverage_usage, replay.usage()))
}

/// Canonical re-export and discharged expected claims are both required.
fn replay_one(cas: &mut Cas, bytes: &[u8], expected: &Judgment, budget: &mut Budget) -> TestResult {
    let proof = cas.replay(bytes, expected, budget)?;
    assert!(proof.is_discharged());
    assert_eq!(proof.hypotheses(), [] as [crate::domain::HypothesisRef; 0]);
    assert_eq!(
        cas.export_witness(&proof, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// All branches and failed attempts remain charged; returned proof data has no authority over the oracles.
pub(super) fn measurement(
    fixture: &mut super::source::Fixture,
    request: &crate::request_model::Request,
) -> TestResult<String> {
    let limits = Limits::blind_cubic();
    let mut generation = Budget::new(limits.clone())?;
    let mut checker = Budget::new(limits.clone())?;
    let mut audit = crate::solving::radicals::route::Audit::default();
    let result = crate::solving::solve_search::produce(
        &mut fixture.cas,
        request,
        &mut audit,
        &mut generation,
        &mut checker,
    )?;
    assert!(result.fault.is_none(), "{:?}", result.fault);
    assert_eq!(result.answer.form, SolveForm::RealExact);
    assert_eq!(result.answer.unresolved_regions, [] as [crate::Handle; 0]);
    let coverage = result
        .answer
        .coverage
        .as_ref()
        .ok_or("Real audit coverage")?;
    assert!(coverage.is_discharged());
    let inventory = super::oracle::real_samples(&fixture.cas, coverage)?;
    let (fresh_coverage, replay) = fresh(&fixture.cas, &result.answer)?;
    let coverage_bytes = coverage.bytes.len();
    let presentation_bytes = result
        .answer
        .facts
        .last()
        .ok_or("Real audit presentation")?
        .bytes
        .len();
    for usage in [generation.usage(), checker.usage(), replay] {
        assert_eq!(usage.orbit_permutations, 0);
        assert_eq!(usage.orbit_terms, 0);
        assert_eq!(usage.symmetric_steps, 0);
        assert!(usage.coefficient_ops <= limits.coefficient_ops);
        assert!(usage.work <= limits.work);
    }
    Ok(format!(
        "requested_form=AnyExact\nresult=Solved(RealExact)\nindependent_strata={inventory:?}\nfresh_coverage=true\nfresh_presentation=true\nreal_only_closure=true\nlimits={limits:?}\nphases={:#?}\ncoverage_bytes={coverage_bytes}\npresentation_bytes={presentation_bytes}\nfinal_generation={:?}\nreplay={:?}\nfresh_coverage_replay={fresh_coverage:?}\nfresh_replay={replay:?}\n",
        audit.phases,
        generation.usage(),
        checker.usage()
    ))
}

/// Public acceptance is also exercised by the original integration fixture.
pub(crate) fn check() -> TestResult {
    let (mut fixture, request) = request()?;
    let record = measurement(&mut fixture, &request)?;
    assert!(record.contains("result=Solved(RealExact)"));
    Ok(())
}
