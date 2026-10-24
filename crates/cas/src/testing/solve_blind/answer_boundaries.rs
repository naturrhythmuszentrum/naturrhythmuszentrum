// SPDX-License-Identifier: AGPL-3.0-only
//! Exact cumulative portable-answer caps for proof-empty original symbolic receivers.

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::graph::Cas;
use crate::model::Field;
use crate::solving::{SolveAnswer, SolveForm};
use crate::testing::common::TestResult;

/// Fixed tower-closure diagnostics 2 and 4 measurements include both expected comparisons and sealing both outputs.
#[derive(Clone, Copy)]
struct Boundary {
    kind: BudgetKind,
    required: u64,
}

fn source(field: &Field) -> TestResult<(Cas, SolveAnswer)> {
    let (mut fixture, mut request) =
        super::source::symbolic_in_field(&["d", "c", "b", "a"], field)?;
    if *field == Field::Real {
        let crate::task::Task::Solve(specification) = &mut request.task else {
            return Err("Real source".into());
        };
        specification.form = SolveForm::AnyExact;
    }
    let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
        return Err("boundary answer missing".into());
    };
    Ok((fixture.cas, answer))
}

fn usage(measured: &BudgetUsage, kind: BudgetKind) -> TestResult<u64> {
    match kind {
        BudgetKind::Work => Ok(measured.work),
        BudgetKind::CoefficientOps => Ok(measured.coefficient_ops),
        _ => Err("unexpected answer counter".into()),
    }
}

fn budget(boundary: Boundary, cap: u64) -> TestResult<Budget> {
    let mut limits = Limits::blind_cubic();
    match boundary.kind {
        BudgetKind::Work => limits.work = cap,
        BudgetKind::CoefficientOps => limits.coefficient_ops = cap,
        _ => return Err("unexpected answer limit".into()),
    }
    Ok(Budget::new(limits)?)
}

/// Every cap rebuilds the same original syntax and admits the proposed set without a checked proof.
fn at_cap(
    cas: &Cas,
    answer: &SolveAnswer,
    field: &Field,
    boundary: Boundary,
    cap: u64,
) -> TestResult {
    let bytes = super::answer::exported(cas, answer.facts.last().ok_or("presentation")?)?;
    let (mut fixture, coverage, presentation) =
        super::answer::receiver(cas, answer, &["d", "c", "b", "a"], field)?;
    assert_eq!(fixture.cas.state.proof_count(), 0);
    let before = fixture.cas.counts();
    let mut ledger = budget(boundary, cap)?;
    let result = fixture
        .cas
        .replay_answer(&bytes, &coverage, &presentation, &mut ledger);
    if cap == boundary.required {
        let (checked_coverage, checked_presentation) = result?;
        assert_eq!(usage(&ledger.usage(), boundary.kind)?, boundary.required);
        assert_eq!(
            super::answer::exported(&fixture.cas, &checked_presentation)?,
            bytes
        );
        assert_eq!(
            super::answer::exported(&fixture.cas, &checked_coverage)?,
            super::answer::exported(cas, answer.coverage.as_ref().ok_or("coverage")?)?
        );
    } else {
        let fault = result.err().ok_or("below-cap answer accepted")?;
        assert!(
            matches!(fault.kind, crate::FaultKind::Exhausted { budget, limit, needed, .. }
            if budget == boundary.kind && limit == cap && needed == boundary.required),
            "{fault:?}"
        );
        assert_eq!(fixture.cas.counts(), before);
        assert_eq!(fixture.cas.state.proof_count(), 0);
        assert!(ledger.usage().work > 0);
        assert!(ledger.usage().coefficient_ops > 0);
    }
    Ok(())
}

/// The receiver and replay ledger both retain the unchanged blind-cubic profile except the tested cap.
pub(crate) fn check(field: &Field) -> TestResult {
    let (coefficient_ops, work) = match field {
        Field::Complex => (2_940_416, 358_125_477),
        Field::Real => (2_008_224, 346_398_574),
        _ => return Err("symbolic boundary field".into()),
    };
    let (cas, answer) = source(field)?;
    for boundary in [
        Boundary {
            kind: BudgetKind::CoefficientOps,
            required: coefficient_ops,
        },
        Boundary {
            kind: BudgetKind::Work,
            required: work,
        },
    ] {
        for cap in [boundary.required, boundary.required - 1] {
            at_cap(&cas, &answer, field, boundary, cap)?;
        }
    }
    Ok(())
}
