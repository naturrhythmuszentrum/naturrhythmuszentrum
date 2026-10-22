// SPDX-License-Identifier: AGPL-3.0-only
//! Complete answers bind both expected claims in one fresh cumulative replay.

use std::fmt::Write;

use crate::budget::{Budget, BudgetUsage, Limits};
use crate::graph::Cas;
use crate::model::Field;
use crate::proof_model::{Judgment, VerifiedWitness};
use crate::solving::{SolveAnswer, SolveForm};
use crate::testing::common::TestResult;
use crate::testing::replay_answer::{Obligation, expected, measured, proposal};

/// A prepared receiver has original source and proposed syntax, with no checked applications.
pub(crate) type Receiver = (super::source::Fixture, Judgment, Judgment);

/// Measurements retain both standalone baselines and the one-closure replacement.
#[derive(Debug)]
pub(crate) struct Measurement {
    pub(crate) coverage: BudgetUsage,
    pub(crate) presentation: BudgetUsage,
    pub(crate) answer: BudgetUsage,
    pub(crate) coverage_bytes: usize,
    pub(crate) presentation_bytes: usize,
    pub(crate) obligations: Vec<Obligation>,
    pub(crate) coverage_obligations: Vec<Obligation>,
}

pub(crate) fn receiver(
    cas: &Cas,
    answer: &SolveAnswer,
    keys: &[&str],
    field: &Field,
) -> TestResult<Receiver> {
    let (mut fixture, request) = super::source::symbolic_in_field(keys, field)?;
    let crate::task::Task::Solve(original) = &request.task else {
        return Err("expected original solve request".into());
    };
    let form = if *field == Field::Real {
        SolveForm::RealExact
    } else {
        SolveForm::Radicals
    };
    let set = proposal(cas, &answer.set, &mut fixture.cas)?;
    let (coverage, presentation) = expected(&fixture, &original.problem, &set, form);
    Ok((fixture, coverage, presentation))
}

pub(crate) fn exported(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Vec<u8>> {
    assert!(witness.is_discharged());
    assert_eq!(
        witness.hypotheses(),
        [] as [crate::domain_model::HypothesisRef; 0]
    );
    Ok(cas.export_witness(witness, &mut Budget::new(Limits::blind_cubic())?)?)
}

/// Each baseline starts with the same independently constructed expected source and syntax-only proposal.
fn standalone(
    prepared: Receiver,
    bytes: &[u8],
    presentation: bool,
) -> TestResult<(BudgetUsage, Vec<Obligation>)> {
    let (mut fixture, coverage, display) = prepared;
    let expected = if presentation { &display } else { &coverage };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let (witness, obligations) =
        measured(|| Ok(fixture.cas.replay(bytes, expected, &mut budget)?))?;
    assert_eq!(exported(&fixture.cas, &witness)?, bytes);
    Ok((budget.usage(), obligations))
}

/// The shared closure has exactly the same per-rule arithmetic as standalone presentation.
fn same_obligations(actual: &[Obligation], baseline: &[Obligation]) {
    assert_eq!(actual.len(), baseline.len());
    for (left, right) in actual.iter().zip(baseline) {
        assert_eq!(left.rule, right.rule);
        assert_eq!(
            left.after.coefficient_ops - left.before.coefficient_ops,
            right.after.coefficient_ops - right.before.coefficient_ops
        );
        assert_eq!(
            left.after.work - left.before.work,
            right.after.work - right.before.work
        );
    }
}

/// Produce once, then independently replay separate and complete claims in three proof-empty receivers.
pub(crate) fn measurement(keys: &[&str], field: &Field) -> TestResult<Measurement> {
    let (mut fixture, mut request) = super::source::symbolic_in_field(keys, field)?;
    if *field == Field::Real {
        let crate::task::Task::Solve(specification) = &mut request.task else {
            return Err("Real request".into());
        };
        specification.form = SolveForm::AnyExact;
    }
    let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
        return Err("complete symbolic answer missing".into());
    };
    let coverage = answer.coverage.as_ref().ok_or("coverage missing")?;
    if *field == Field::Real {
        super::oracle::real_samples(&fixture.cas, coverage)?;
    } else {
        super::oracle::samples(&fixture.cas, coverage, keys)?;
    }
    replay(&fixture.cas, &answer, keys, field)
}

fn replay(
    cas: &Cas,
    answer: &SolveAnswer,
    keys: &[&str],
    field: &Field,
) -> TestResult<Measurement> {
    let coverage_bytes = exported(cas, answer.coverage.as_ref().ok_or("coverage")?)?;
    let presentation_bytes = exported(cas, answer.facts.last().ok_or("presentation")?)?;
    let (coverage, coverage_obligations) =
        standalone(receiver(cas, answer, keys, field)?, &coverage_bytes, false)?;
    let (presentation, original) = standalone(
        receiver(cas, answer, keys, field)?,
        &presentation_bytes,
        true,
    )?;
    let (mut fixture, expected_coverage, expected_presentation) =
        receiver(cas, answer, keys, field)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let ((checked_coverage, checked_presentation), obligations) = measured(|| {
        Ok(fixture.cas.replay_answer(
            &presentation_bytes,
            &expected_coverage,
            &expected_presentation,
            &mut budget,
        )?)
    })?;
    assert_eq!(exported(&fixture.cas, &checked_coverage)?, coverage_bytes);
    assert_eq!(
        exported(&fixture.cas, &checked_presentation)?,
        presentation_bytes
    );
    same_obligations(&obligations, &original);
    assert_eq!(budget.usage().coefficient_ops, presentation.coefficient_ops);
    assert!(budget.usage().coefficient_ops <= Limits::blind_cubic().coefficient_ops * 9 / 10);
    Ok(Measurement {
        coverage,
        presentation,
        answer: budget.usage(),
        coverage_bytes: coverage_bytes.len(),
        presentation_bytes: presentation_bytes.len(),
        obligations,
        coverage_obligations,
    })
}

/// The six named acceptance boundaries include both original and renamed declaration frames.
pub(crate) fn symbolic() -> TestResult<String> {
    let ordinary = ["d", "c", "b", "a"];
    let renamed = ["delta", "gamma", "beta", "alpha"];
    let mut report = String::new();
    for (name, keys, field) in [
        ("blind_symbolic_complex_complete", &ordinary, Field::Complex),
        ("blind_symbolic_acceptance_audit", &ordinary, Field::Complex),
        (
            "blind_complete_coefficient_renaming",
            &ordinary,
            Field::Complex,
        ),
        (
            "blind_complete_coefficient_renaming",
            &renamed,
            Field::Complex,
        ),
        (
            "blind_symbolic_source_retains_parameters",
            &ordinary,
            Field::Complex,
        ),
        (
            "blind_symbolic_source_retains_parameters",
            &renamed,
            Field::Complex,
        ),
        ("blind_missing_stratum_mutations", &ordinary, Field::Complex),
        ("blind_symbolic_real_complete", &ordinary, Field::Real),
    ] {
        let result = measurement(keys, &field)?;
        writeln!(
            report,
            "{name} {keys:?} before_coeff={} before_work={} before_bytes={} after_coeff={} after_work={} after_bytes={} coverage_coeff={} coverage_work={} coverage_bytes={} presentation_coeff={} presentation_work={} obligations={} coverage_rules={:?} answer_rules={:?}",
            result.coverage.coefficient_ops + result.presentation.coefficient_ops,
            result.coverage.work + result.presentation.work,
            result.coverage_bytes + result.presentation_bytes,
            result.answer.coefficient_ops,
            result.answer.work,
            result.presentation_bytes,
            result.coverage.coefficient_ops,
            result.coverage.work,
            result.coverage_bytes,
            result.presentation.coefficient_ops,
            result.presentation.work,
            result.obligations.len(),
            attribution(&result.coverage_obligations),
            attribution(&result.obligations)
        )?;
    }
    Ok(report)
}

/// Rule totals remain available without printing from the committed test harness.
fn attribution(rows: &[Obligation]) -> std::collections::BTreeMap<&'static str, (u64, u64, usize)> {
    let mut result = std::collections::BTreeMap::new();
    for row in rows {
        let totals = result
            .entry(row.rule.specification().key)
            .or_insert((0, 0, 0));
        totals.0 += row.after.coefficient_ops - row.before.coefficient_ops;
        totals.1 += row.after.work - row.before.work;
        totals.2 += 1;
    }
    result
}
