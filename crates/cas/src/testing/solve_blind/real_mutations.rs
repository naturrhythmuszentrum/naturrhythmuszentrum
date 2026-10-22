// SPDX-License-Identifier: AGPL-3.0-only
//! Mutations keep the original Real declaration frame and target one checked partition field.

use crate::budget::{Budget, Limits};
use crate::graph::{Cas, LogicalId};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::solving::{PredicateSplitMode, SolveCertificate, SolveStatement};
use crate::testing::common::TestResult;

/// The cubic discriminant guard uses all four original coefficients; quadratic and p guards do not.
fn keys(cas: &Cas, expression: LogicalId) -> TestResult<std::collections::BTreeSet<String>> {
    let mut result = std::collections::BTreeSet::new();
    let mut pending = vec![expression];
    while let Some(id) = pending.pop() {
        let expression = cas.state.expression(id)?;
        if let crate::model::Operator::Free(key) = &expression.operator {
            result.insert(key.clone());
        }
        pending.extend(&expression.operands);
    }
    Ok(result)
}

/// Find the actual independently checked discriminant cover, without consulting the producer's plan.
fn discriminant(cas: &Cas, steps: &[ApplicationDraft]) -> TestResult<usize> {
    for (index, step) in steps.iter().enumerate() {
        let Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::TotalPredicateSplit(value) = certificate.as_ref() else {
            continue;
        };
        if value.mode == PredicateSplitMode::RealSigns
            && keys(cas, value.expression.logical_id())?
                == ["a", "b", "c", "d"]
                    .map(str::to_owned)
                    .into_iter()
                    .collect()
        {
            return Ok(index);
        }
    }
    Err("Real discriminant cover".into())
}

/// Changing the sign convention or omitting the zero child cannot borrow the original complete cover.
pub(crate) fn strata() -> TestResult {
    let (mut fixture, request) = super::real_acceptance::request()?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("Real mutation baseline: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("Real mutation coverage")?;
    let steps = super::audit::applications(&fixture.cas, &coverage)?;
    let position = discriminant(&fixture.cas, &steps)?;
    for omit_zero in [false, true] {
        let mut changed = steps.clone();
        let step = &mut changed[position];
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("Real split certificate".into());
        };
        let SolveCertificate::TotalPredicateSplit(value) = certificate.as_mut() else {
            return Err("Real split shape".into());
        };
        assert_eq!(value.children.len(), 3);
        if omit_zero {
            let removed = value.children.remove(1);
            let Statement::Cover { members, .. } = &mut step.conclusion else {
                return Err("Real split cover".into());
            };
            assert_eq!(members.remove(1), removed);
            assert_eq!(members.len(), 2);
        } else {
            let original = value.expression.clone();
            value.expression = fixture.expression(
                crate::model::Operator::Neg,
                std::slice::from_ref(&original),
                false,
            )?;
            assert_ne!(value.expression, original);
        }
        let fault = fixture
            .cas
            .check_proof(
                &fixture.context,
                &changed,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .err()
            .ok_or("altered Real partition accepted")?;
        assert!(
            matches!(fault.kind, crate::FaultKind::Invalid(_)),
            "{fault:?}"
        );
        assert_eq!(
            fixture
                .cas
                .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?,
            coverage.bytes
        );
    }
    local_piece(&mut fixture.cas, &steps)
}

/// A complete regional theorem survives the failed full cover and retains its exact local context.
fn local_piece(cas: &mut Cas, steps: &[ApplicationDraft]) -> TestResult {
    let (index, step) = steps.iter().enumerate().find(|(_, step)| {
        matches!(&step.conclusion, Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::Coverage(_)))
            && matches!(&step.certificate, Certificate::Solve(value) if matches!(value.as_ref(), SolveCertificate::RealRootCount(_)))
    }).ok_or("Real complete cell piece")?;
    let prefix = super::trace::closure(steps, index)?;
    let proof = cas.check_proof(
        &step.context,
        &prefix,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(proof.is_discharged());
    let bytes = cas.export_witness(&proof, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut fresh = Cas::new(Limits::blind_cubic())?;
    let imported = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = imported.judgment().clone();
    let replay = fresh.replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replay.is_discharged());
    assert_eq!(
        fresh.export_witness(&replay, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}
