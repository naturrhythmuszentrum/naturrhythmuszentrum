// SPDX-License-Identifier: AGPL-3.0-only
//! Every new operation retains cumulative Work and exact scratch ceilings on refusal.

use super::{application, budget, trimming_case};
use crate::testing::common::TestResult;
use nrz_cas::domain::Layer;
use nrz_cas::limits::{Budget, BudgetKind, BudgetUsage};
use nrz_cas::proof::{Judgment, Statement};
use nrz_cas::{Fault, FaultKind};

/// A result and its irrevocable ledger, after checking graph rollback locally.
type Attempt = (Result<(), Fault>, BudgetUsage);

fn attempt(operation: u32, kind: BudgetKind, cap: u64, prior: u64) -> TestResult<Attempt> {
    let (mut fixture, mut steps, input) = trimming_case()?;
    let mut limits = budget()?.limits().clone();
    match kind {
        BudgetKind::Work => limits.work = cap + prior,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = cap,
        _ => return Err("boundary kind".into()),
    }
    let mut ledger = Budget::new(limits)?;
    ledger.work(prior, nrz_cas::Location::new("earlier caller work"))?;
    if operation == 0 {
        let before = fixture.cas.counts();
        let result = fixture.cas.trim_emptiness(&input, &mut ledger).map(|_| ());
        assert_eq!(fixture.cas.counts(), before);
        return Ok((result, ledger.usage()));
    }
    let trimmed = fixture.cas.trim_emptiness(&input, &mut budget()?)?;
    steps.push(application(&fixture, trimmed, vec![0, 1])?);
    if operation == 1 {
        let before = fixture.cas.counts();
        let result = fixture
            .cas
            .check_proof(&fixture.context, &steps, &mut ledger)
            .map(|_| ());
        if result.is_err() {
            assert_eq!(fixture.cas.counts(), before);
        }
        return Ok((result, ledger.usage()));
    }
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &steps, &mut budget()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let (mut target, _, input) = trimming_case()?;
    let expected = Judgment {
        context: target.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: Statement::Empty {
            region: input.nodes.last().ok_or("root")?.region.clone(),
        },
        residuals: Vec::new(),
    };
    let before = target.cas.counts();
    let result = target
        .cas
        .replay(&bytes, &expected, &mut ledger)
        .map(|_| ());
    if result.is_err() {
        assert_eq!(target.cas.counts(), before);
    }
    Ok((result, ledger.usage()))
}

fn boundary(operation: u32, kind: BudgetKind, required: u64) -> TestResult {
    for prior in [0, 37] {
        for short in [0, 1] {
            let cap = required - short;
            let (result, usage) = attempt(operation, kind, cap, prior)?;
            if short == 0 {
                result?;
                if kind == BudgetKind::Work {
                    assert_eq!(usage.work, required + prior);
                }
            } else {
                let fault = result.err().ok_or("below-cap operation accepted")?;
                let offset = if kind == BudgetKind::Work { prior } else { 0 };
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget, limit, needed, .. }
                    if budget == kind && if kind == BudgetKind::Work { limit == cap + offset && needed == required + offset } else { needed == limit + 1 }),
                    "{fault:?}"
                );
                assert!(usage.work >= prior);
            }
        }
    }
    Ok(())
}

/// Trimming, checking and fresh replay stop exactly at N-minus-one after prior caller work.
/// ´test:integration:emptiness-cumulative-work-boundaries´
#[test]
fn emptiness_cumulative_work_boundaries() -> TestResult {
    for operation in 0..3 {
        let (result, usage) = attempt(operation, BudgetKind::Work, 100_000_000, 0)?;
        result?;
        boundary(operation, BudgetKind::Work, usage.work)?;
    }
    Ok(())
}

/// Scratch capacity is measured independently and refuses without a partial graph at N-minus-one.
/// ´test:integration:emptiness-scratch-boundaries´
#[test]
fn emptiness_scratch_boundaries() -> TestResult {
    let mut measured = Vec::new();
    for operation in 0..3 {
        let (mut lower, mut upper) = (0, budget()?.limits().staged_payload_bytes);
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let (result, _) = attempt(operation, BudgetKind::StagedPayloadBytes, middle, 0)?;
            match result {
                Ok(()) => upper = middle,
                Err(Fault {
                    kind:
                        FaultKind::Exhausted {
                            budget: BudgetKind::StagedPayloadBytes,
                            ..
                        },
                    ..
                }) => lower = middle + 1,
                Err(error) => return Err(error.into()),
            }
        }
        boundary(operation, BudgetKind::StagedPayloadBytes, lower)?;
        measured.push(lower);
    }
    assert_eq!(measured, [4_800, 20_224, 20_224]);
    Ok(())
}
