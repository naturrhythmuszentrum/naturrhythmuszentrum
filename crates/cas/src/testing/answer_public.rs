// SPDX-License-Identifier: AGPL-3.0-only
//! Small independently constructed complete-answer expectations through public APIs.

use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Judgment, Statement};
use crate::test_api::solving::{SolveForm, SolveStatement};
use crate::testing::common::TestResult;
use crate::testing::solve_elementary::{Fixture, constant_cover};

/// Both client expectations are constructed before any witness import.
pub(crate) struct AnswerFixture {
    pub(crate) base: Fixture,
    pub(crate) coverage: Judgment,
    pub(crate) presentation: Judgment,
    pub(crate) steps: Vec<ApplicationDraft>,
}

pub(crate) fn budget() -> TestResult<Budget> {
    Ok(Budget::new(Limits::blind_cubic())?)
}

/// The independently known nonzero constant equation has the empty solution set.
pub(crate) fn prepared() -> TestResult<AnswerFixture> {
    let (base, mut steps) = constant_cover()?;
    let coverage = Judgment {
        context: base.context.clone(),
        layer: crate::test_api::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: steps.last().ok_or("coverage step")?.conclusion.clone(),
    };
    let Statement::Solve(statement) = &coverage.statement else {
        return Err("coverage statement".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("coverage shape".into());
    };
    let mut step = base.present(&claim.set, SolveForm::Radicals);
    step.premises = vec![1];
    let presentation = Judgment {
        statement: step.conclusion.clone(),
        ..coverage.clone()
    };
    steps.push(step);
    Ok(AnswerFixture {
        base,
        coverage,
        presentation,
        steps,
    })
}

/// The transport contains only the ordinary presentation closure.
pub(crate) fn encoded() -> TestResult<Vec<u8>> {
    let mut fixture = prepared()?;
    let witness =
        fixture
            .base
            .cas
            .check_proof(&fixture.base.context, &fixture.steps, &mut budget()?)?;
    Ok(fixture.base.cas.export_witness(&witness, &mut budget()?)?)
}

/// Any refusal must roll back all imported graph objects while retaining the spent ledger.
pub(crate) fn refused(fixture: &mut AnswerFixture, bytes: &[u8]) -> TestResult {
    let before = fixture.base.cas.counts();
    let mut ledger = budget()?;
    assert!(
        fixture
            .base
            .cas
            .replay_answer(bytes, &fixture.coverage, &fixture.presentation, &mut ledger)
            .is_err()
    );
    assert_eq!(fixture.base.cas.counts(), before);
    Ok(())
}

/// Decode only record boundaries independently of the production reader.
pub(crate) fn records(bytes: &[u8]) -> TestResult<Vec<(u8, std::ops::Range<usize>)>> {
    assert_eq!(bytes.get(..8), Some(b"NRZCAS04".as_slice()));
    let mut offset = 8;
    let manifest = index(bytes, &mut offset)?;
    for _ in 0..manifest {
        let length = usize::try_from(index(bytes, &mut offset)?)?;
        offset = offset.checked_add(length).ok_or("manifest length")?;
        index(bytes, &mut offset)?;
    }
    let count = index(bytes, &mut offset)?;
    let mut result = Vec::new();
    for _ in 0..count {
        let tag = *bytes.get(offset).ok_or("record tag")?;
        offset += 1;
        let length = usize::try_from(index(bytes, &mut offset)?)?;
        let end = offset.checked_add(length).ok_or("record length")?;
        bytes.get(offset..end).ok_or("record payload")?;
        result.push((tag, offset..end));
        offset = end;
    }
    index(bytes, &mut offset)?;
    assert_eq!(offset, bytes.len());
    Ok(result)
}

pub(crate) fn index(bytes: &[u8], offset: &mut usize) -> TestResult<u32> {
    let end = offset.checked_add(4).ok_or("index end")?;
    let value = u32::from_le_bytes(bytes.get(*offset..end).ok_or("index bytes")?.try_into()?);
    *offset = end;
    Ok(value)
}
