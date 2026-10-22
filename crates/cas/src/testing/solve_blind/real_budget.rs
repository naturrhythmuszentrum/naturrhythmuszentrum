// SPDX-License-Identifier: AGPL-3.0-only
//! Exact Real request caps cover the whole branch tree and its independent reconstruction.

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::solving::coefficients::model::CoefficientFailure;
use crate::solving::radicals::proof::Produced;
use crate::testing::common::TestResult;

/// One independently measured cumulative resource.
struct Counter {
    /// The resource named by an exact exhausted fault.
    kind: BudgetKind,
    /// Read the corresponding measured ledger.
    usage: fn(&BudgetUsage) -> u64,
    /// Change this ceiling alone.
    limit: fn(&mut Limits, u64),
}

/// The completed or refused run retains both independent ledgers and its original store.
type Run = (
    super::source::Fixture,
    Result<Produced, CoefficientFailure>,
    BudgetUsage,
    BudgetUsage,
);

/// The same two ledgers cover every branch, every failed attempt and the final presentation.
fn run(limits: Limits) -> TestResult<Run> {
    let (mut fixture, request) = super::real_acceptance::request()?;
    let mut generation = Budget::new(limits.clone())?;
    let mut checker = Budget::new(limits)?;
    let result = crate::solving::solve_search::produce(
        &mut fixture.cas,
        &request,
        &mut crate::solving::radicals::route::Audit::default(),
        &mut generation,
        &mut checker,
    );
    Ok((fixture, result, generation.usage(), checker.usage()))
}

/// Resource names and accessors cannot be interchanged without changing the asserted fault.
fn counters() -> [Counter; 5] {
    [
        Counter {
            kind: BudgetKind::Work,
            usage: |u| u.work,
            limit: |l, n| l.work = n,
        },
        Counter {
            kind: BudgetKind::CoefficientOps,
            usage: |u| u.coefficient_ops,
            limit: |l, n| l.coefficient_ops = n,
        },
        Counter {
            kind: BudgetKind::SolutionBranches,
            usage: |u| u.solution_branches,
            limit: |l, n| l.solution_branches = n,
        },
        Counter {
            kind: BudgetKind::CoefficientMaps,
            usage: |u| u.coefficient_maps,
            limit: |l, n| l.coefficient_maps = n,
        },
        Counter {
            kind: BudgetKind::IsolationSteps,
            usage: |u| u.isolation_steps,
            limit: |l, n| l.isolation_steps = n,
        },
    ]
}

/// All exhausted paths retain their actual resource kind and attempted work.
fn exhausted(
    result: Result<Produced, CoefficientFailure>,
    kind: BudgetKind,
    cap: u64,
) -> TestResult {
    let fault = match result {
        Err(CoefficientFailure::Fault(fault)) => fault,
        Ok(result) => result.fault.ok_or("Real below-cap route succeeded")?,
        Err(error) => return Err(format!("Real resource became unavailable: {error:?}").into()),
    };
    assert!(
        matches!(fault.kind, crate::FaultKind::Exhausted { budget, limit, needed, .. }
        if budget == kind && limit == cap && needed > cap),
        "{fault:?}"
    );
    Ok(())
}

/// N succeeds identically; N-1 fails for the measured counter with no per-stratum reset.
pub(crate) fn measured() -> TestResult {
    let (fixture, baseline, generation, checker) = run(Limits::blind_cubic())?;
    let baseline = baseline?;
    assert!(baseline.fault.is_none());
    assert_eq!(baseline.answer.unresolved_regions, [] as [crate::Handle; 0]);
    let coverage = baseline
        .answer
        .coverage
        .as_ref()
        .ok_or("Real budget coverage")?;
    for counter in counters() {
        let required = (counter.usage)(&generation).max((counter.usage)(&checker));
        assert!(required > 0, "unexercised {:?}", counter.kind);
        for cap in [required, required - 1] {
            let mut limits = Limits::blind_cubic();
            (counter.limit)(&mut limits, cap);
            let (_, result, _, _) = run(limits)?;
            if cap == required {
                let result = result?;
                assert!(
                    result.fault.is_none(),
                    "{:?}: {:?}",
                    counter.kind,
                    result.fault
                );
                assert_eq!(result.answer.unresolved_regions, [] as [crate::Handle; 0]);
                assert_eq!(
                    result
                        .answer
                        .coverage
                        .ok_or("Real exact-cap coverage")?
                        .bytes,
                    coverage.bytes
                );
            } else {
                exhausted(result, counter.kind, cap)?;
            }
        }
    }
    witness(&fixture.cas, coverage)?;
    replay(&baseline.answer, &fixture.cas)
}

/// Witness bytes are measured on the complete exported dependency DAG, with no omitted branch.
fn witness(cas: &crate::Cas, proof: &crate::proof::VerifiedWitness) -> TestResult {
    let length = u64::try_from(proof.bytes.len())?;
    for cap in [length, length.checked_sub(1).ok_or("empty Real witness")?] {
        let mut budget = Budget::new(Limits {
            witness_bytes: cap,
            ..Limits::blind_cubic()
        })?;
        let result = cas.export_witness(proof, &mut budget);
        if cap == length {
            assert_eq!(result?, proof.bytes);
        } else {
            let fault = result.err().ok_or("Real short witness cap accepted")?;
            assert!(
                matches!(fault.kind, crate::FaultKind::Exhausted { budget: BudgetKind::WitnessBytes, limit, .. } if limit == cap),
                "{fault:?}"
            );
        }
    }
    Ok(())
}

/// A fresh expected coverage theorem is independently checked at its exact replay work boundary.
fn replay(answer: &crate::solving::SolveAnswer, cas: &crate::Cas) -> TestResult {
    let coverage = answer.coverage.as_ref().ok_or("Real replay coverage")?;
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut fixture, request) = super::real_acceptance::request()?;
    let imported = fixture
        .cas
        .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let crate::proof::Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("Real replay theorem".into());
    };
    let crate::solving::SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("Real replay claim".into());
    };
    let crate::task::Task::Solve(original) = &request.task else {
        return Err("Real replay original".into());
    };
    assert_eq!(claim.problem, original.problem);
    assert_eq!(claim.region, fixture.region);
    let expected = crate::proof::Judgment {
        context: fixture.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: crate::proof::Statement::Solve(Box::new(
            crate::solving::SolveStatement::Coverage(crate::solving::StatementCoverage {
                problem: original.problem.clone(),
                set: claim.set.clone(),
                region: fixture.region.clone(),
            }),
        )),
    };
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture.cas.replay(&bytes, &expected, &mut measured)?;
    let required = measured.usage().work;
    for cap in [
        required,
        required.checked_sub(1).ok_or("empty Real replay")?,
    ] {
        let result = fixture.cas.replay(
            &bytes,
            &expected,
            &mut Budget::new(Limits {
                work: cap,
                ..Limits::blind_cubic()
            })?,
        );
        if cap == required {
            assert!(result?.is_discharged());
        } else {
            let fault = result.err().ok_or("Real short replay cap accepted")?;
            assert!(
                matches!(fault.kind, crate::FaultKind::Exhausted { budget: BudgetKind::Work, limit, .. } if limit == cap),
                "{fault:?}"
            );
        }
    }
    Ok(())
}
