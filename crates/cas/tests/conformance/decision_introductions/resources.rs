// SPDX-License-Identifier: AGPL-3.0-only
//! Literal resource boundaries retain atomic failure and monotone caller accounting.

use super::super::decision_baseline::Example;
use super::{frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{Cas, Fault, FaultKind};

#[derive(Clone, Copy, Debug)]
enum Lifecycle {
    Introduction,
    Frame,
    Interned,
    Membership,
    Import,
    Citations,
}

fn limits(kind: BudgetKind, amount: u64) -> Limits {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::PayloadBytes => limits.payload_bytes = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        _ => {}
    }
    limits
}

fn outcome<T>(result: Result<T, Fault>, kind: BudgetKind) -> TestResult<bool> {
    match result {
        Ok(value) => {
            drop(value);
            Ok(true)
        }
        Err(fault) if matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind) => {
            Ok(false)
        }
        Err(fault) => Err(fault.into()),
    }
}

fn perform(
    example: Example,
    lifecycle: Lifecycle,
    kind: BudgetKind,
    budget: &mut Budget,
) -> TestResult<bool> {
    let mut fixture = prepared_introduction(example, true)?;
    let source = &mut fixture.partition.source;
    if matches!(lifecycle, Lifecycle::Introduction) {
        let before = source.cas.counts();
        let result = source
            .cas
            .check_proof(&source.context, &fixture.steps, budget);
        if result.is_err() {
            assert_eq!(source.cas.counts(), before);
        }
        return outcome(result, kind);
    }
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &intro);
    let source = &mut fixture.partition.source;
    if matches!(lifecycle, Lifecycle::Frame) {
        let before = source.cas.counts();
        let result = source.cas.admit(&[draft], budget);
        if result.is_err() {
            assert_eq!(source.cas.counts(), before);
        }
        return outcome(result, kind);
    }
    let frame = admit(&mut source.cas, draft.clone(), &mut ledger()?)?;
    if matches!(lifecycle, Lifecycle::Interned) {
        let before = source.cas.counts();
        let result = source.cas.admit(&[draft], budget);
        assert_eq!(source.cas.counts(), before);
        return outcome(result, kind);
    }
    later(&mut fixture, &frame, lifecycle, kind, budget)
}

fn later(
    fixture: &mut super::IntroductionFixture,
    frame: &nrz_cas::Handle,
    lifecycle: Lifecycle,
    kind: BudgetKind,
    budget: &mut Budget,
) -> TestResult<bool> {
    let source = &mut fixture.partition.source;
    let step = membership(&source.cas, frame, &fixture.option)?;
    if matches!(lifecycle, Lifecycle::Membership) {
        let before = source.cas.counts();
        let result = source.cas.check_proof(frame, &[step], budget);
        if result.is_err() {
            assert_eq!(source.cas.counts(), before);
        }
        return outcome(result, kind);
    }
    let member = source.cas.check_proof(frame, &[step], &mut ledger()?)?;
    if matches!(lifecycle, Lifecycle::Citations) {
        return outcome(source.cas.proof_citations(member.proof(), budget), kind);
    }
    let bytes = source.cas.export_witness(&member, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let before = receiver.counts();
    let result = receiver.inspect(&bytes, budget);
    if result.is_err() {
        assert_eq!(receiver.counts(), before);
    }
    outcome(result, kind)
}

fn attempt(
    example: Example,
    lifecycle: Lifecycle,
    kind: BudgetKind,
    amount: u64,
) -> TestResult<bool> {
    perform(
        example,
        lifecycle,
        kind,
        &mut Budget::new(limits(kind, amount))?,
    )
}

fn boundaries(example: Example, amounts: &[[u64; 4]; 6]) -> TestResult {
    for (lifecycle, amounts) in [
        Lifecycle::Introduction,
        Lifecycle::Frame,
        Lifecycle::Interned,
        Lifecycle::Membership,
        Lifecycle::Import,
        Lifecycle::Citations,
    ]
    .into_iter()
    .zip(amounts)
    {
        for (kind, amount) in [
            BudgetKind::Work,
            BudgetKind::StagedPayloadBytes,
            BudgetKind::PayloadBytes,
            BudgetKind::OutputBytes,
        ]
        .into_iter()
        .zip(amounts)
        {
            assert!(
                attempt(example, lifecycle, kind, *amount)?,
                "{example:?}:{lifecycle:?}:{kind:?}"
            );
            if *amount != 0 {
                assert!(
                    !attempt(example, lifecycle, kind, amount - 1)?,
                    "{example:?}:{lifecycle:?}:{kind:?}"
                );
            }
        }
        carried(example, lifecycle, amounts)?;
    }
    Ok(())
}

fn carried(example: Example, lifecycle: Lifecycle, amounts: &[u64; 4]) -> TestResult {
    let at = nrz_cas::Location::new("retained caller ledger");
    for short in [0, 1] {
        let mut budget = Budget::new(limits(BudgetKind::Work, amounts[0] + 37 - short))?;
        budget.work(37, at)?;
        let success = perform(example, lifecycle, BudgetKind::Work, &mut budget)?;
        assert_eq!(success, short == 0);
        assert!(budget.usage().work >= 37);
        if success {
            assert_eq!(budget.usage().work, amounts[0] + 37);
        }
        let mut budget = Budget::new(limits(
            BudgetKind::StagedPayloadBytes,
            amounts[1] + 257 - short,
        ))?;
        let success = budget.with_retained_payload(257, at, |budget| {
            perform(example, lifecycle, BudgetKind::StagedPayloadBytes, budget)
        })?;
        assert_eq!(success, short == 0);
    }
    Ok(())
}

/// Exact N and N−1 ceilings include prior Work and retained caller bytes.
/// ´test:integration:decision-introduction-resources-integer´
#[test]
fn decision_introduction_resources_integer() -> TestResult {
    boundaries(
        Example::Integer,
        &[
            [322_565, 79_328, 36_373, 792],
            [1_648, 38_553, 37_628, 0],
            [1_646, 38_553, 129, 0],
            [113_794, 84_768, 38_688, 0],
            [538_289, 84_192, 33_902, 792],
            [625, 600, 0, 600],
        ],
    )
}

/// Exact N and N−1 ceilings include prior Work and retained caller bytes.
/// ´test:integration:decision-introduction-resources-sign´
#[test]
fn decision_introduction_resources_sign() -> TestResult {
    boundaries(
        Example::Sign,
        &[
            [147_865, 52_780, 20_845, 800],
            [1_648, 23_025, 22_100, 0],
            [1_646, 23_025, 129, 0],
            [65_237, 47_040, 23_160, 0],
            [237_975, 52_332, 18_368, 800],
            [625, 600, 0, 600],
        ],
    )
}

/// Exact N and N−1 ceilings include prior Work and retained caller bytes.
/// ´test:integration:decision-introduction-resources-phase´
#[test]
fn decision_introduction_resources_phase() -> TestResult {
    boundaries(
        Example::Phase,
        &[
            [195_044, 67_560, 35_431, 792],
            [2_402, 37_782, 36_857, 0],
            [2_400, 37_782, 191, 0],
            [100_405, 71_744, 37_968, 0],
            [359_124, 71_168, 28_454, 792],
            [906, 864, 0, 864],
        ],
    )
}
