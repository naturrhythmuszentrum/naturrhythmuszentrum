// SPDX-License-Identifier: AGPL-3.0-only
//! The cumulative portable contract checks both halves of each complete symbolic answer.

use crate::testing::common::TestResult;

/// All six original acceptance boundaries use fresh syntax-only receivers and independent source claims.
/// ´test:crate:complete-answer-symbolic-fixtures´
#[test]
fn complete_answer_symbolic_fixtures() -> TestResult {
    let report = crate::testing::solve_blind::complete_answer_symbolic()?;
    assert_eq!(report.lines().count(), 8);
    Ok(())
}

/// Complex coverage and presentation fit exactly at pinned cumulative coefficient and Work caps.
/// ´test:crate:complete-answer-complex-boundaries´
#[test]
fn complete_answer_complex_boundaries() -> TestResult {
    crate::testing::solve_blind::complete_answer_boundaries(&crate::model::Field::Complex)
}

/// Real coverage and presentation fit exactly at pinned cumulative coefficient and Work caps.
/// ´test:crate:complete-answer-real-boundaries´
#[test]
fn complete_answer_real_boundaries() -> TestResult {
    crate::testing::solve_blind::complete_answer_boundaries(&crate::model::Field::Real)
}
