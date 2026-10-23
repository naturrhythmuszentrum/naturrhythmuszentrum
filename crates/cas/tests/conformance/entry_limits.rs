// SPDX-License-Identifier: AGPL-3.0-only
//! Exact entry judgments charge cumulative work and bound all simultaneously live arrays.

use crate::testing::common::TestResult;
use crate::testing::readings::{Fixture, ledger, matrix};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::readings::SelectionSpec;
use nrz_cas::{Fault, FaultKind, Handle};

type Operation = fn(&Fixture, &Handle, &[Handle], &mut Budget) -> Result<(), Fault>;

fn relation(
    fixture: &Fixture,
    _: &Handle,
    selections: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [left, right] = selections else {
        return Err(Fault::invalid(
            nrz_cas::InvalidCode::Reference,
            nrz_cas::Location::new("entry test"),
        ));
    };
    fixture
        .cas
        .entry_set_relation(left, right, budget)
        .map(|_| ())
}

fn coverage(
    fixture: &Fixture,
    base: &Handle,
    selections: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    fixture
        .cas
        .entry_coverage(base, selections, budget)
        .map(|_| ())
}

fn boundaries(operation: Operation, work: u64, staged: u64, output: u64) -> TestResult {
    let mut fixture = Fixture::new(&[("b", matrix(2, 2))])?;
    let base = fixture.symbol("b")?;
    let mut selections = Vec::new();
    for spec in [SelectionSpec::Row(0), SelectionSpec::Column(0)] {
        selections.push(fixture.cas.select_entries(&base, &spec, &mut ledger()?)?);
    }
    let before = fixture.cas.counts();
    let mut measured = ledger()?;
    operation(&fixture, &base, &selections, &mut measured)?;
    assert_eq!(measured.usage().work, work);
    for (kind, needed) in [
        (BudgetKind::Work, work),
        (BudgetKind::StagedPayloadBytes, staged),
        (BudgetKind::OutputBytes, output),
    ] {
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let mut limits = Limits::default();
            match kind {
                BudgetKind::Work => limits.work = limit,
                BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
                BudgetKind::OutputBytes => limits.output_bytes = limit,
                _ => return Err("unexpected limit".into()),
            }
            let result = operation(&fixture, &base, &selections, &mut Budget::new(limits)?);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(
                    matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
                );
            }
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(1, nrz_cas::Location::new("prior entry work"))?;
    assert!(matches!(
        operation(&fixture, &base, &selections, &mut budget),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Two two-entry selections cost exactly 25 Work and 32 bytes of intersection capacity.
/// ´test:integration:entry-relation-work-and-payload-boundaries´
#[test]
fn entry_relation_work_and_payload_boundaries() -> TestResult {
    boundaries(relation, 25, 32, 32)
}

/// A four-entry coverage with two two-entry selections costs 43 Work and 128 staged bytes.
/// ´test:integration:entry-coverage-work-and-payload-boundaries´
#[test]
fn entry_coverage_work_and_payload_boundaries() -> TestResult {
    boundaries(coverage, 43, 128, 96)
}
