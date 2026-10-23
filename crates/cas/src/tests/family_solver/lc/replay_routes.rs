// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh default-budget replay agrees with the original independently encoded oracle.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::proof_model::Judgment;
use crate::request::Outcome;
use crate::testing::common::TestResult;
use crate::testing::sorts::Fixture;
use crate::{Cas, Handle};

fn source() -> TestResult<(Fixture, Handle, Vec<u8>)> {
    let (mut fixture, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
        return Err("LC source coverage".into());
    };
    let witness = answer.coverage.as_ref().ok_or("LC coverage")?;
    let bytes = fixture.cas.export_witness(witness, &mut super::ledger()?)?;
    Ok((fixture, answer.set, bytes))
}

fn receiver(source: &Cas, set: &Handle) -> TestResult<(Fixture, Judgment)> {
    let (mut fixture, request) = super::fixture::request([1, 1], true)?;
    let proposed = crate::testing::replay_answer::proposal(source, set, &mut fixture.cas)?;
    let expected = super::replay::expected(&fixture, &request, &proposed)?;
    assert_eq!(fixture.cas.state.proof_count(), 0);
    Ok((fixture, expected))
}

/// Default replay and the original complete replay prove the same unspecialized two-parameter theorem.
/// ´test:crate:lc-replay-default-agrees-with-full-oracle´
#[test]
fn lc_replay_default_agrees_with_full_oracle() -> TestResult {
    let (original, set, bytes) = source()?;
    let mut costs = Vec::new();
    for full in [false, true] {
        let (mut target, expected) = receiver(&original.cas, &set)?;
        let mut budget = if full {
            super::ledger()?
        } else {
            Budget::new(Limits::default())?
        };
        let witness = if full {
            crate::testing::replay_answer::full_replay(
                &mut target.cas,
                &bytes,
                &expected,
                &mut budget,
            )?
        } else {
            target.cas.replay(&bytes, &expected, &mut budget)?
        };
        assert!(witness.is_discharged());
        assert_eq!(witness.judgment(), &expected);
        assert_eq!(
            target.cas.export_witness(&witness, &mut super::ledger()?)?,
            bytes
        );
        crate::testing::solve_blind::family_lc(&target.cas, &witness)?;
        costs.push(budget.usage().work);
    }
    assert_eq!(costs, [1_433_783, 2_033_541]);
    Ok(())
}

fn capped_replay(
    original: &Cas,
    set: &Handle,
    bytes: &[u8],
    kind: BudgetKind,
    short: u64,
    prior: u64,
) -> TestResult {
    let required = 1_433_783;
    let cap = required - short;
    let (mut target, expected) = receiver(original, set)?;
    let before = target.cas.counts();
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = cap + prior,
        BudgetKind::ReplayWork => limits.replay_work = cap,
        _ => return Err("replay cap kind".into()),
    }
    let mut ledger = Budget::new(limits)?;
    ledger.work(prior, crate::Location::new("prior caller work"))?;
    let result = target.cas.replay(bytes, &expected, &mut ledger);
    if short == 0 {
        assert!(result?.is_discharged());
        assert_eq!(ledger.usage().work, required + prior);
    } else {
        let fault = result.err().ok_or("below-cap replay accepted")?;
        let offset = if kind == BudgetKind::Work { prior } else { 0 };
        assert!(
            matches!(fault.kind, crate::FaultKind::Exhausted { budget, limit, needed, .. }
            if budget == kind && limit == cap + offset && needed == required + offset),
            "{fault:?}"
        );
        assert_eq!(target.cas.counts(), before);
        assert_eq!(target.cas.state.proof_count(), 0);
        assert!(ledger.usage().work > prior);
    }
    Ok(())
}

/// Exact total and replay-only ceilings include earlier caller work and roll back an N-minus-one refusal.
/// ´test:crate:lc-replay-exact-work-boundaries´
#[test]
fn lc_replay_exact_work_boundaries() -> TestResult {
    let (original, set, bytes) = source()?;
    for kind in [BudgetKind::Work, BudgetKind::ReplayWork] {
        for prior in [0, 37] {
            for short in [0, 1] {
                capped_replay(&original.cas, &set, &bytes, kind, short, prior)?;
            }
        }
    }
    Ok(())
}

/// Empty zero-set lemmas and the retained complete search agree on the original LC judgment.
/// ´test:crate:lc-empty-exclusion-agrees-with-search´
#[test]
fn lc_empty_exclusion_agrees_with_search() -> TestResult {
    let (original, set, bytes) = source()?;
    let mut costs = Vec::new();
    for oracle in [false, true] {
        let (mut target, expected) = receiver(&original.cas, &set)?;
        let mut budget = super::ledger()?;
        let result = if oracle {
            crate::testing::exclusion_oracle::run(|| {
                target.cas.replay(&bytes, &expected, &mut budget)
            })
        } else {
            target.cas.replay(&bytes, &expected, &mut budget)
        };
        let witness = result?;
        assert!(witness.is_discharged());
        assert_eq!(witness.judgment(), &expected);
        crate::testing::solve_blind::family_lc(&target.cas, &witness)?;
        costs.push(budget.usage().work);
    }
    assert_eq!(costs, [1_433_783, 1_782_724]);
    Ok(())
}
