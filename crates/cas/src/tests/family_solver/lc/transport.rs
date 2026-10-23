// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar interpretation consumes the same checked Real base projection as component interpretation.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::evaluate::Evaluation;
use crate::testing::common::TestResult;
use crate::testing::sorts::Fixture;
use crate::{Fault, FaultKind, Handle, InvalidCode, Location};

struct Transport {
    source: Fixture,
    domain: Handle,
    region: Handle,
    subject: Handle,
    premises: Vec<CheckedApplication>,
}

impl Transport {
    fn evaluation(&self) -> Evaluation<'_> {
        Evaluation {
            cas: &self.source.cas,
            engine: Engine {
                state: &self.source.cas.state,
                domain: &self.domain,
            },
            region: &self.region,
            premises: &self.premises,
            free: &[],
            bound: &[],
        }
    }
}

fn fixture() -> TestResult<Transport> {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let crate::request::Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC scalar transport baseline".into());
    };
    let proof = answer.coverage.as_ref().ok_or("LC coverage")?;
    let root = crate::proof_view::application(
        &source.cas,
        proof.proof.logical_id(),
        &mut super::ledger()?,
    )?;
    let premises = root
        .premises
        .iter()
        .map(|id| {
            Ok(crate::proof_view::application(
                &source.cas,
                *id,
                &mut super::ledger()?,
            )?)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let predicate = premises
        .iter()
        .find_map(|premise| match &premise.judgment.statement {
            Statement::PredicateFact { predicate, .. } => Some(predicate),
            _ => None,
        })
        .ok_or("LC product sign")?;
    let (_, [subject]) =
        crate::solving::elementary::evidence::predicate(&source.cas, predicate.logical_id())?
    else {
        return Err("LC positive product".into());
    };
    let subject = source.cas.handle(*subject);
    let crate::request::Task::Solve(specification) = &request.task else {
        return Err("LC solve request".into());
    };
    let problem =
        crate::solving::graph::problem(&source.cas.state, specification.problem.logical_id())?;
    let domain = problem.coefficient_domain.clone();
    let region = problem.region.clone();
    Ok(Transport {
        source,
        domain,
        region,
        subject,
        premises,
    })
}

fn limited(work: u64, payload: u64) -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work,
        staged_payload_bytes: payload,
        ..Limits::blind_cubic()
    })?)
}

fn payload(fixture: &Transport) -> TestResult<u64> {
    let mut lower = 0;
    let mut upper = Limits::blind_cubic().staged_payload_bytes;
    let evaluation = fixture.evaluation();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let mut budget = limited(Limits::blind_cubic().work, middle)?;
        match evaluation.expression(fixture.subject.logical_id(), true, &mut budget) {
            Ok(_) => upper = middle,
            Err(Fault {
                kind:
                    FaultKind::Exhausted {
                        budget: BudgetKind::StagedPayloadBytes,
                        ..
                    },
                ..
            }) => lower = middle + 1,
            Err(fault) => return Err(fault.into()),
        }
    }
    Ok(upper)
}

/// Actual transported scalar evaluation succeeds exactly at its Work and scratch ceilings.
/// ´test:crate:lc-scalar-base-transport-boundaries´
#[test]
fn lc_scalar_base_transport_boundaries() -> TestResult {
    let fixture = fixture()?;
    let evaluation = fixture.evaluation();
    let mut measured = super::ledger()?;
    let value = evaluation.expression(fixture.subject.logical_id(), true, &mut measured)?;
    let work = measured.usage().work;
    let bytes = payload(&fixture)?;
    let exact = evaluation.expression(
        fixture.subject.logical_id(),
        true,
        &mut limited(work, bytes)?,
    )?;
    assert!(value.equals(&exact, &evaluation.engine, &mut super::ledger()?)?);
    for (kind, mut budget) in [
        (BudgetKind::Work, limited(work - 1, bytes)?),
        (BudgetKind::StagedPayloadBytes, limited(work, bytes - 1)?),
    ] {
        let fault = evaluation
            .expression(fixture.subject.logical_id(), true, &mut budget)
            .err()
            .ok_or("short scalar transport budget")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
    }
    let mut cumulative = limited(work, bytes)?;
    cumulative.work(1, Location::new("prior scalar transport work"))?;
    assert!(
        evaluation
            .expression(fixture.subject.logical_id(), true, &mut cumulative)
            .is_err()
    );
    assert_eq!((work, bytes), (174, 576));
    Ok(())
}

/// A retained Real scalar map supplies no implicit projection into the family context.
/// ´test:crate:lc-scalar-base-transport-requires-projection´
#[test]
fn lc_scalar_base_transport_requires_projection() -> TestResult {
    let mut fixture = fixture()?;
    fixture
        .premises
        .retain(|premise| !matches!(&premise.judgment.statement, Statement::Subset { .. }));
    let evaluation = fixture.evaluation();
    let before = fixture.source.cas.counts();
    let fault = evaluation
        .expression(fixture.subject.logical_id(), true, &mut super::ledger()?)
        .err()
        .ok_or("implicit LC projection")?;
    assert_eq!(
        fault,
        Fault::invalid(InvalidCode::Rule, Location::new("local family evaluation"))
    );
    assert_eq!(fixture.source.cas.counts(), before);
    Ok(())
}
