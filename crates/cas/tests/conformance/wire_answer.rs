// SPDX-License-Identifier: AGPL-3.0-only
//! Complete-answer replay binds both expected claims and requires actual checked coverage.

use crate::testing::answer::{budget, encoded, prepared, refused};
use crate::testing::common::TestResult;
use nrz_cas::proof::Statement;
use nrz_cas::solving::{SolveForm, SolveStatement};

fn coverage(
    fixture: &mut crate::testing::answer::AnswerFixture,
) -> TestResult<&mut nrz_cas::solving::StatementCoverage> {
    let Statement::Solve(statement) = &mut fixture.coverage.statement else {
        return Err("coverage".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_mut() else {
        return Err("coverage claim".into());
    };
    Ok(claim)
}

fn change_source(fixture: &mut crate::testing::answer::AnswerFixture) -> TestResult {
    let three = fixture.base.integer(3, false)?;
    let zero = fixture.base.integer(0, false)?;
    let predicate = fixture.base.equation(&three, &zero)?;
    let original = fixture.base.problem("x", &predicate, &[])?;
    assert_eq!(coverage(fixture)?.problem, original);
    let one = fixture.base.integer(1, false)?;
    let predicate = fixture.base.equation(&one, &zero)?;
    let replacement = fixture.base.problem("x", &predicate, &[])?;
    assert_ne!(original, replacement);
    coverage(fixture)?.problem = replacement;
    Ok(())
}

fn change_region(fixture: &mut crate::testing::answer::AnswerFixture) -> TestResult {
    let original = fixture.base.region.clone();
    assert_eq!(coverage(fixture)?.region, original);
    let replacement = fixture.base.coefficient_guard(true)?;
    assert_ne!(replacement, coverage(fixture)?.region);
    coverage(fixture)?.region = replacement;
    Ok(())
}

fn change_form(fixture: &mut crate::testing::answer::AnswerFixture) -> TestResult {
    let Statement::Solve(statement) = &mut fixture.presentation.statement else {
        return Err("presentation".into());
    };
    let SolveStatement::Presentation(display) = statement.as_mut() else {
        return Err("presentation claim".into());
    };
    assert_eq!(display.form, SolveForm::Radicals);
    let replacement = SolveForm::AlgebraicRoots;
    assert_ne!(display.form, replacement);
    display.form = replacement;
    Ok(())
}

fn change_set(fixture: &mut crate::testing::answer::AnswerFixture) -> TestResult<nrz_cas::Handle> {
    let problem = coverage(fixture)?.problem.clone();
    let original = fixture.base.finite(&problem, &[])?;
    assert_eq!(coverage(fixture)?.set, original);
    let one = fixture.base.integer(1, false)?;
    let replacement = fixture.base.finite(&problem, &[one])?;
    assert_ne!(original, replacement);
    coverage(fixture)?.set.clone_from(&replacement);
    Ok(replacement)
}

/// Independently rebuilt source, region, form and set expectations reject each single-field change.
/// ´test:integration:complete-answer-expected-mutations´
#[test]
fn complete_answer_expected_mutations() -> TestResult {
    let bytes = encoded()?;
    for mutation in 0..4 {
        let mut fixture = prepared()?;
        match mutation {
            0 => change_source(&mut fixture)?,
            1 => change_region(&mut fixture)?,
            2 => change_form(&mut fixture)?,
            _ => {
                change_set(&mut fixture)?;
            }
        }
        refused(&mut fixture, &bytes)?;
    }
    Ok(())
}

/// A valid standalone presentation without a coverage application cannot certify a complete answer.
/// ´test:integration:complete-answer-requires-coverage-dependency´
#[test]
fn complete_answer_requires_coverage_dependency() -> TestResult {
    let mut source = prepared()?;
    let mut step = source.steps.pop().ok_or("presentation step")?;
    assert_eq!(step.premises, [1]);
    step.premises.clear();
    assert_eq!(step.premises, [] as [u32; 0]);
    let witness = source
        .base
        .cas
        .check_proof(&source.base.context, &[step], &mut budget()?)?;
    let bytes = source.base.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = prepared()?;
    refused(&mut receiver, &bytes)
}

fn encoded_other_set() -> TestResult<Vec<u8>> {
    let mut source = prepared()?;
    let set = change_set(&mut source)?;
    let mut step = source.base.present(&set, SolveForm::Radicals);
    step.premises = vec![1];
    source.steps.pop().ok_or("old presentation")?;
    source.steps.push(step);
    let witness =
        source
            .base
            .cas
            .check_proof(&source.base.context, &source.steps, &mut budget()?)?;
    Ok(source.base.cas.export_witness(&witness, &mut budget()?)?)
}

/// A reachable coverage theorem for a different set cannot satisfy a same-source presentation.
/// ´test:integration:complete-answer-rejects-different-covered-set´
#[test]
fn complete_answer_rejects_different_covered_set() -> TestResult {
    let bytes = encoded_other_set()?;
    let mut receiver = prepared()?;
    let replacement = change_set(&mut receiver)?;
    receiver.presentation.statement = receiver
        .base
        .present(&replacement, SolveForm::Radicals)
        .conclusion;
    refused(&mut receiver, &bytes)
}

/// Branch policy is part of the complete semantic header and cannot reuse the original theorem.
/// ´test:integration:complete-answer-policy-mutation´
#[test]
fn complete_answer_policy_mutation() -> TestResult {
    let mut bytes = encoded()?;
    let (_, header) = crate::testing::answer::records(&bytes)?
        .into_iter()
        .find(|(tag, _)| *tag == 0)
        .ok_or("header record")?;
    assert_eq!(
        bytes.get(header.start..header.start + 8),
        Some([1, 0, 0, 0, 0, 0, 0, 1].as_slice())
    );
    let policy = bytes.get_mut(header.start + 8).ok_or("branch policy")?;
    assert_eq!(*policy, 0);
    let replacement = 1;
    assert_ne!(*policy, replacement);
    *policy = replacement;
    refused(&mut prepared()?, &bytes)
}

/// Replacing the product's identity premise with its own application is refused as a forward reference.
/// ´test:integration:complete-answer-premise-mutation´
#[test]
fn complete_answer_premise_mutation() -> TestResult {
    let mut bytes = encoded()?;
    let records = crate::testing::answer::records(&bytes)?;
    let applications: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, (tag, _))| *tag == 11)
        .collect();
    assert_eq!(applications.len(), 3);
    let (identity, _) = applications.first().ok_or("identity record")?;
    let (product, (_, payload)) = applications.get(1).ok_or("product record")?;
    assert_eq!(
        bytes.get(payload.start..payload.start + 6),
        Some([1, 0, 0, 0, 0, 0].as_slice())
    );
    let mut offset = payload.start + 14;
    assert_eq!(crate::testing::answer::index(&bytes, &mut offset)?, 1);
    let position = offset;
    assert_eq!(
        crate::testing::answer::index(&bytes, &mut offset)?,
        u32::try_from(*identity)?
    );
    let replacement = u32::try_from(*product)?;
    assert_ne!(replacement, u32::try_from(*identity)?);
    bytes
        .get_mut(position..offset)
        .ok_or("premise slot")?
        .copy_from_slice(&replacement.to_le_bytes());
    refused(&mut prepared()?, &bytes)
}

/// Existing ledger charges survive the entire answer, and a failed later import refunds nothing.
/// ´test:integration:complete-answer-cumulative-ledger´
#[test]
fn complete_answer_cumulative_ledger() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    let bytes = encoded()?;
    let mut baseline = prepared()?;
    let mut measured = budget()?;
    baseline.base.cas.replay_answer(
        &bytes,
        &baseline.coverage,
        &baseline.presentation,
        &mut measured,
    )?;
    let usage = measured.usage();
    let mut receiver = prepared()?;
    let mut ledger = Budget::new(Limits {
        work: usage.work + 17,
        ..Limits::blind_cubic()
    })?;
    let at = nrz_cas::Location::new("prior answer work");
    ledger.work(17, at)?;
    ledger.solving(BudgetKind::CoefficientOps, 3, at)?;
    assert_eq!(ledger.usage().work, 18);
    assert_eq!(ledger.usage().coefficient_ops, 3);
    let before = receiver.base.cas.counts();
    let fault = receiver
        .base
        .cas
        .replay_answer(
            &bytes,
            &receiver.coverage,
            &receiver.presentation,
            &mut ledger,
        )
        .err()
        .ok_or("cumulative cap exceeded")?;
    assert!(matches!(fault.kind, nrz_cas::FaultKind::Exhausted {
        budget: BudgetKind::Work, limit, needed, ..
    } if limit == usage.work + 17 && needed == usage.work + 18));
    assert_eq!(receiver.base.cas.counts(), before);
    assert_eq!(ledger.usage().coefficient_ops, usage.coefficient_ops + 3);
    assert!(ledger.usage().work > 18);
    Ok(())
}

/// The documented capability index distinguishes registered schemas from executable checkers.
/// ´test:integration:complete-answer-registry-inventory´
#[test]
fn complete_answer_registry_inventory() {
    use nrz_cas::proof::{PRIMITIVES, Primitive};
    assert_eq!(PRIMITIVES.len(), 80);
    assert_eq!(
        PRIMITIVES
            .iter()
            .filter(|rule| rule.specification().available)
            .count(),
        78
    );
    assert_eq!(
        PRIMITIVES
            .iter()
            .filter(|rule| matches!(rule, Primitive::Solve(_)))
            .count(),
        21
    );
    assert!(PRIMITIVES.iter().all(|rule| rule.specification().version
        == if *rule == Primitive::RegionDecide {
            2
        } else {
            1
        }));
    let unavailable: Vec<_> = PRIMITIVES
        .iter()
        .filter(|rule| !rule.specification().available)
        .map(|rule| rule.specification().key)
        .collect();
    assert_eq!(unavailable, ["root.isolate", "solve.local-branch"]);
}
