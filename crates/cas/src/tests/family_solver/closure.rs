// SPDX-License-Identifier: AGPL-3.0-only
//! Complete mixed answers replay once against independently rebuilt original requests.

use super::{Budget, Limits, TestResult, request, stratum};
use crate::proof_model::{Judgment, Statement};
use crate::solving::{SolveAnswer, SolveStatement, StatementCoverage, StatementPresentation};
use std::fmt::Write;

fn expected(
    fixture: &super::Fixture,
    original: &crate::request::Request,
    set: &crate::Handle,
) -> TestResult<(Judgment, Judgment)> {
    let crate::task::Task::Solve(specification) = &original.task else {
        return Err("mixed original request".into());
    };
    let region = fixture.cas.handle(
        fixture
            .cas
            .state
            .context(original.context.logical_id())?
            .admissible,
    );
    let coverage = Judgment {
        context: original.context.clone(),
        layer: crate::domain_model::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: specification.problem.clone(),
            set: set.clone(),
            region: region.clone(),
        }))),
    };
    let presentation = Judgment {
        statement: Statement::Solve(Box::new(SolveStatement::Presentation(
            StatementPresentation {
                set: set.clone(),
                form: specification.form,
                region,
            },
        ))),
        ..coverage.clone()
    };
    Ok((coverage, presentation))
}

fn replay(
    source: &super::Fixture,
    answer: &SolveAnswer,
    degree: i64,
    sign: Option<i8>,
) -> TestResult<(crate::budget::BudgetUsage, usize)> {
    let (mut receiver, mut original) = request(degree)?;
    if let Some(sign) = sign {
        stratum(&mut receiver, &mut original, sign)?;
    }
    let set = crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut receiver.cas)?;
    let (coverage, presentation) = expected(&receiver, &original, &set)?;
    assert_eq!(receiver.cas.state.proof_count(), 0);
    let bytes = source.cas.export_witness(
        answer.facts.last().ok_or("mixed presentation")?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let (checked_coverage, checked_presentation) =
        receiver
            .cas
            .replay_answer(&bytes, &coverage, &presentation, &mut budget)?;
    let mut export = Budget::new(Limits::blind_cubic())?;
    assert_eq!(
        receiver
            .cas
            .export_witness(&checked_presentation, &mut export)?,
        bytes
    );
    let expected_bytes = source.cas.export_witness(
        answer.coverage.as_ref().ok_or("mixed coverage")?,
        &mut export,
    )?;
    assert_eq!(
        receiver
            .cas
            .export_witness(&checked_coverage, &mut export)?,
        expected_bytes
    );
    Ok((budget.usage(), bytes.len()))
}

fn measurements() -> TestResult<String> {
    let mut report = String::new();
    for (degree, sign) in [
        (1, None),
        (2, None),
        (3, None),
        (4, None),
        (2, Some(-1)),
        (2, Some(0)),
        (2, Some(1)),
    ] {
        let (mut fixture, mut original) = request(degree)?;
        if let Some(sign) = sign {
            stratum(&mut fixture, &mut original, sign)?;
        }
        let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&original) else {
            return Err("mixed complete answer missing".into());
        };
        let (usage, bytes) = replay(&fixture, &answer, degree, sign)?;
        writeln!(
            report,
            "degree={degree} sign={sign:?} coefficient_ops={} work={} bytes={bytes}",
            usage.coefficient_ops, usage.work
        )?;
    }
    Ok(report)
}

/// Original mixed linear, binomial, general cubic and signed-square pieces bind both expected claims.
/// ´test:crate:mixed-complete-answer-closure´
#[test]
fn mixed_complete_answer_closure() -> TestResult {
    let report = measurements()?;
    assert_eq!(report.lines().count(), 7);
    Ok(())
}

/// Recognize the four-step original cubic Horner equation, with one identical root at every power.
fn cubic_equation_root(
    cas: &crate::Cas,
    left: &crate::Handle,
) -> TestResult<Option<crate::LogicalId>> {
    let mut value = left.logical_id();
    let mut root = None;
    for _ in 0..4 {
        let addition = cas.state.expression(value)?;
        if addition.operator != crate::model::Operator::Add {
            return Ok(None);
        }
        let [product, _] = addition.operands.as_slice() else {
            return Ok(None);
        };
        let multiplication = cas.state.expression(*product)?;
        if multiplication.operator != crate::model::Operator::Mul {
            return Ok(None);
        }
        let [previous, selected] = multiplication.operands.as_slice() else {
            return Ok(None);
        };
        if root.is_some_and(|expected| expected != *selected) {
            return Ok(None);
        }
        root = Some(*selected);
        value = *previous;
    }
    let initial = cas.state.expression(value)?;
    if initial.operator != crate::model::Operator::Rational(crate::Rational::zero()) {
        return Ok(None);
    }
    Ok(root)
}

/// The original general cubic keeps two selected-generator equations and derives the last linear root.
/// ´test:crate:mixed-cubic-equation-inventory´
#[test]
fn mixed_cubic_equation_inventory() -> TestResult {
    let (mut fixture, original) = request(4)?;
    let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&original) else {
        return Err("mixed cubic inventory answer".into());
    };
    let coverage = answer.coverage.as_ref().ok_or("mixed inventory coverage")?;
    let applications = crate::testing::solve_blind::family_applications(&fixture.cas, coverage)?;
    let mut roots = std::collections::BTreeSet::new();
    for application in applications {
        if let Statement::Equality { left, right, .. } = application.conclusion {
            let zero = fixture.cas.state.expression(right.logical_id())?;
            if zero.operator == crate::model::Operator::Rational(crate::Rational::zero())
                && let Some(root) = cubic_equation_root(&fixture.cas, &left)?
            {
                assert!(roots.insert(root));
            }
        }
    }
    assert_eq!(roots.len(), 2);
    replay(&fixture, &answer, 4, None)?;
    Ok(())
}
