// SPDX-License-Identifier: AGPL-3.0-only
//! Shared sign subjects retain their complete nested scratch payload through product checking.

use crate::budget::{Budget, BudgetKind};
use crate::proof::Statement;
use crate::request::{Outcome, Task};
use crate::solving::elementary::{evaluate::Evaluation, exclusion};
use crate::testing::common::TestResult;

/// Original LC sign leaves are checked once with exact Work and retained scratch boundaries.
/// ´test:crate:lc-inline-sign-leaf-boundaries´
#[test]
fn lc_inline_sign_leaf_boundaries() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC coverage".into());
    };
    let witness = answer.coverage.as_ref().ok_or("LC coverage")?;
    let root = crate::proof_view::application(
        &source.cas,
        witness.proof().logical_id(),
        &mut super::ledger()?,
    )?;
    let mut premises = Vec::new();
    for id in root.premises {
        premises.push(crate::proof_view::application(
            &source.cas,
            id,
            &mut super::ledger()?,
        )?);
    }
    let Task::Solve(specification) = &request.task else {
        return Err("LC solve request".into());
    };
    let original = crate::solving::elementary::proof::problem(&source.cas, &specification.problem)?;
    let evaluation = Evaluation {
        exclusions: &[],
        cas: &source.cas,
        engine: crate::solving::coefficients::arithmetic::Engine {
            state: &source.cas.state,
            domain: &original.coefficient_domain,
        },
        region: &original.region,
        premises: &premises,
        free: &[],
        bound: &[],
    };
    let slots: Vec<_> = premises
        .iter()
        .enumerate()
        .filter_map(|(index, premise)| {
            matches!(
                &premise.judgment.statement,
                Statement::Empty { .. } | Statement::PredicateFact { .. }
            )
            .then_some(index)
        })
        .collect();
    boundaries(|budget| exclusion::values(&evaluation, &slots, budget))
}

fn boundaries(
    mut check: impl FnMut(&mut Budget) -> Result<exclusion::Lemmas, crate::Fault>,
) -> TestResult {
    let mut ledger = super::ledger()?;
    let lemmas = check(&mut ledger)?;
    let required = ledger.usage().work;
    assert_eq!(lemmas.values.len(), 3);
    assert!(lemmas.bytes > 0);
    for short in [0, 1] {
        let mut limits = ledger.limits().clone();
        limits.work = required - short;
        let result = check(&mut Budget::new(limits)?);
        if short == 0 {
            result?;
        } else {
            assert!(matches!(
                result.err().ok_or("short leaf compilation accepted")?.kind,
                crate::FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
        }
    }
    let mut lower = 0;
    let mut upper = ledger.limits().staged_payload_bytes;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if staged(&mut check, middle)? {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower >= lemmas.bytes);
    assert!(staged(&mut check, lower)?);
    assert!(!staged(&mut check, lower - 1)?);
    assert_eq!((required, lemmas.bytes, lower), (361, 5_944, 5_944));
    Ok(())
}

fn staged(
    check: &mut impl FnMut(&mut Budget) -> Result<exclusion::Lemmas, crate::Fault>,
    cap: u64,
) -> TestResult<bool> {
    let mut limits = super::ledger()?.limits().clone();
    limits.staged_payload_bytes = cap;
    match check(&mut Budget::new(limits)?) {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(
                fault.kind,
                crate::FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    ..
                }
            ));
            Ok(false)
        }
    }
}
