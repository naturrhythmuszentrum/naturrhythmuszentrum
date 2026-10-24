// SPDX-License-Identifier: AGPL-3.0-only
//! Exact source domains and complete root inventories are mandatory theorem endpoints.

use crate::proof::{Certificate, Statement};
use crate::request::{Outcome, Request};
use crate::solving::{
    SolutionBranchKind, SolveAnswer, SolveCertificate, SolveDraft, SolveStatement,
};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, reference};
use crate::{Fault, InvalidCode, Location};

fn solved() -> TestResult<(Fixture, Request, SolveAnswer)> {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let outcome = source.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("LC refusal baseline: {outcome:?}").into());
    };
    Ok((source, request, answer))
}

fn retarget(
    receiver: &mut Fixture,
    request: &Request,
    set: &crate::Handle,
) -> TestResult<crate::Handle> {
    let crate::request::Task::Solve(specification) = &request.task else {
        return Err("LC target request".into());
    };
    let problem = specification.problem.clone();
    let original = crate::solving::graph::problem(&receiver.cas.state, problem.logical_id())?;
    let region = original.region.clone();
    let domain = original.domain.clone();
    let SolveDraft::SolutionSet(original) =
        crate::solving::graph::object(&receiver.cas.state, set.logical_id())?
    else {
        return Err("LC imported set".into());
    };
    let [branch] = original.branches.as_slice() else {
        return Err("LC imported branch".into());
    };
    let SolveDraft::SolutionBranch(branch) =
        crate::solving::graph::object(&receiver.cas.state, branch.logical_id())?
    else {
        return Err("LC imported finite branch".into());
    };
    let SolutionBranchKind::Finite { tuples } = &branch.kind else {
        return Err("LC proposed tuples".into());
    };
    let kind = SolutionBranchKind::Finite {
        tuples: tuples.iter().map(reference).collect(),
    };
    let branch = admit(
        &mut receiver.cas,
        crate::syntax::Draft::Solve(SolveDraft::SolutionBranch(
            crate::solving::SolutionBranchData {
                problem: reference(&problem),
                region: reference(&region),
                domain: reference(&domain),
                kind,
            },
        )),
    )?;
    admit(
        &mut receiver.cas,
        crate::syntax::Draft::Solve(SolveDraft::SolutionSet(crate::solving::SolutionSetData {
            problem: reference(&problem),
            branches: vec![reference(&branch)],
        })),
    )
}

/// Exact expected replay refuses absent signs, missing original divisors and sibling strata.
/// ´test:crate:lc-root-replay-retains-original-premises´
#[test]
fn lc_root_replay_retains_original_premises() -> TestResult {
    let (source, _, answer) = solved()?;
    let proof = answer.coverage.as_ref().ok_or("LC coverage")?;
    let bytes = source.cas.export_witness(proof, &mut super::ledger()?)?;
    let mut traces = Vec::new();
    for (signs, retain) in [
        ([0, 1], true),
        ([1, 0], true),
        ([-1, 1], true),
        ([1, -1], true),
        ([1, 1], false),
    ] {
        let (mut receiver, request) = super::fixture::request(signs, retain)?;
        let set =
            crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut receiver.cas)?;
        let set = retarget(&mut receiver, &request, &set)?;
        let expected = super::replay::expected(&receiver, &request, &set)?;
        let before = receiver.cas.counts();
        let mut budget = super::ledger()?;
        let (result, rows) = crate::testing::work_trace::measured(|| {
            receiver.cas.replay(&bytes, &expected, &mut budget)
        })?;
        traces.push((
            signs,
            retain,
            budget.usage().work,
            super::costs::totals(rows.iter())?,
        ));
        let fault = result.err().ok_or("changed LC premises accepted")?;
        assert_eq!(
            fault,
            Fault::invalid(
                InvalidCode::EndpointMismatch,
                Location::new("typed proof admission")
            )
        );
        assert_eq!(
            crate::testing::replay_answer::full_replay(
                &mut receiver.cas,
                &bytes,
                &expected,
                &mut super::ledger()?,
            )
            .err(),
            Some(fault),
        );
        let mut oracle_budget = super::ledger()?;
        let oracle = crate::testing::exclusion_oracle::run(|| {
            receiver.cas.replay(&bytes, &expected, &mut oracle_budget)
        });
        assert_eq!(oracle.err(), Some(fault));
        assert_eq!(receiver.cas.counts(), before);
    }
    assert_eq!(
        traces,
        [
            (
                [0, 1],
                true,
                1_600_130,
                [221_241, 75_578, 74_839, 1_228_472]
            ),
            (
                [1, 0],
                true,
                1_598_505,
                [221_241, 75_578, 74_839, 1_226_847]
            ),
            (
                [-1, 1],
                true,
                1_604_303,
                [221_241, 75_578, 74_839, 1_232_645]
            ),
            (
                [1, -1],
                true,
                1_602_675,
                [221_241, 75_578, 74_839, 1_231_017]
            ),
            (
                [1, 1],
                false,
                1_594_865,
                [221_241, 75_578, 74_839, 1_223_207]
            ),
        ]
    );
    Ok(())
}

fn incomplete(source: &mut Fixture, set: &crate::Handle) -> TestResult<crate::Handle> {
    let SolveDraft::SolutionSet(original) =
        crate::solving::graph::object(&source.cas.state, set.logical_id())?
    else {
        return Err("LC solution set".into());
    };
    let problem = original.problem.clone();
    let [branch] = original.branches.as_slice() else {
        return Err("LC solution branch".into());
    };
    let SolveDraft::SolutionBranch(branch) =
        crate::solving::graph::object(&source.cas.state, branch.logical_id())?
    else {
        return Err("LC finite branch".into());
    };
    let SolutionBranchKind::Finite { tuples } = &branch.kind else {
        return Err("LC finite roots".into());
    };
    assert_eq!(tuples.len(), 2);
    let reduced = crate::solving::SolutionBranchData {
        problem: reference(&branch.problem),
        region: reference(&branch.region),
        domain: reference(&branch.domain),
        kind: SolutionBranchKind::Finite {
            tuples: vec![reference(tuples.first().ok_or("first LC root")?)],
        },
    };
    let branch = admit(
        &mut source.cas,
        crate::syntax::Draft::Solve(SolveDraft::SolutionBranch(reduced)),
    )?;
    admit(
        &mut source.cas,
        crate::syntax::Draft::Solve(SolveDraft::SolutionSet(crate::solving::SolutionSetData {
            problem: reference(&problem),
            branches: vec![reference(&branch)],
        })),
    )
}

/// Omitting either a factor or a represented root cannot inherit complete coverage.
/// ´test:crate:lc-root-factor-coverage-rejects-omissions´
#[test]
fn lc_root_factor_coverage_rejects_omissions() -> TestResult {
    let (mut source, request, answer) = solved()?;
    let proof = answer.coverage.as_ref().ok_or("LC coverage")?;
    let original = crate::testing::solve_blind::family_applications(&source.cas, proof)?;
    let reduced = incomplete(&mut source, &answer.set)?;
    let mut traces = Vec::new();
    for remove_factor in [false, true] {
        let mut steps = original.clone();
        let step = steps.last_mut().ok_or("LC product application")?;
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("LC certificate".into());
        };
        let SolveCertificate::ProductCompleteness(value) = certificate.as_mut() else {
            return Err("LC product".into());
        };
        if remove_factor {
            value.factors.pop().ok_or("LC factor")?;
        } else {
            value.set.clone_from(&reduced);
            let Statement::Solve(statement) = &mut step.conclusion else {
                return Err("LC statement".into());
            };
            let SolveStatement::Coverage(claim) = statement.as_mut() else {
                return Err("LC claim".into());
            };
            claim.set.clone_from(&reduced);
        }
        let before = source.cas.counts();
        let mut budget = super::ledger()?;
        let (result, rows) = crate::testing::work_trace::measured(|| {
            source
                .cas
                .check_proof(&request.context, &steps, &mut budget)
        })?;
        traces.push((
            remove_factor,
            budget.usage().work,
            super::costs::totals(rows.iter())?,
        ));
        let fault = result.err().ok_or("incomplete LC set accepted")?;
        assert_eq!(
            fault,
            Fault::invalid(InvalidCode::Rule, Location::new("elementary solving"))
        );
        let mut oracle_budget = super::ledger()?;
        let oracle = crate::testing::exclusion_oracle::run(|| {
            source
                .cas
                .check_proof(&request.context, &steps, &mut oracle_budget)
        });
        assert_eq!(oracle.err(), Some(fault));
        assert_eq!(source.cas.counts(), before);
    }
    assert_eq!(
        traces,
        [
            (false, 363_897, [205_391, 36_228, 74_778, 47_500]),
            (true, 422_203, [221_241, 78_735, 74_753, 47_474]),
        ]
    );
    Ok(())
}

/// The reciprocal witness requires the explicitly supplied product sign in its exact context.
/// ´test:crate:lc-root-refinement-requires-product-sign´
#[test]
fn lc_root_refinement_requires_product_sign() -> TestResult {
    let (mut source, _, answer) = solved()?;
    let proof = answer.coverage.as_ref().ok_or("LC coverage")?;
    let mut steps = crate::testing::solve_blind::family_applications(&source.cas, proof)?;
    let index = steps
        .iter()
        .position(|step| {
            step.rule
                == crate::rule::Primitive::Solve(
                    crate::solving::SolveSchema::RootSelectionRefinement,
                )
        })
        .ok_or("LC root refinement")?;
    let step = steps.get_mut(index).ok_or("LC selection step")?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("LC refinement certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
        return Err("LC refinement payload".into());
    };
    assert_eq!(value.compatibility.len(), 1);
    value.compatibility.clear();
    let context = step.context.clone();
    let closure = crate::testing::solve_blind::family_closure(&steps, index)?;
    let before = source.cas.counts();
    let fault = source
        .cas
        .check_proof(&context, &closure, &mut super::ledger()?)
        .err()
        .ok_or("missing product sign accepted")?;
    assert_eq!(
        fault,
        Fault::invalid(InvalidCode::Rule, Location::new("elementary solving"))
    );
    let mut oracle_budget = super::ledger()?;
    let oracle = crate::testing::exclusion_oracle::run(|| {
        source
            .cas
            .check_proof(&context, &closure, &mut oracle_budget)
    });
    assert_eq!(oracle.err(), Some(fault));
    assert_eq!(source.cas.counts(), before);
    Ok(())
}
