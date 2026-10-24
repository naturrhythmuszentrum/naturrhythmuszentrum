// SPDX-License-Identifier: AGPL-3.0-only
//! Public source-Jacobian operations share exact cumulative resource boundaries.

use crate::testing::common::TestResult;
use crate::testing::jacobians::{coordinates, product};
use crate::testing::map_inverses::{ledger, limits};
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::{Fault, FaultKind, Location};

fn run(operation: u32, budget: &mut Budget, rollback: bool) -> TestResult {
    let (mut case, original, _) = product()?;
    let key = if operation == 0 {
        None
    } else {
        Some(case.cas.source_jacobian_key(
            &case.context,
            &original,
            &coordinates(),
            &mut ledger()?,
        )?)
    };
    let before = case.cas.counts();
    let result = if let Some(key) = key {
        case.cas.source_jacobian(&key, budget).map(|_| ())
    } else {
        case.cas
            .source_jacobian_key(&case.context, &original, &coordinates(), budget)
            .map(|_| ())
    };
    if rollback {
        assert_eq!(case.cas.counts(), before);
    }
    Ok(result?)
}

fn bounded(kind: BudgetKind, limit: u64) -> TestResult<Budget> {
    let mut profile = limits();
    match kind {
        BudgetKind::Work => profile.work = limit,
        BudgetKind::StagedPayloadBytes => profile.staged_payload_bytes = limit,
        BudgetKind::PayloadBytes => profile.payload_bytes = limit,
        BudgetKind::OutputBytes => profile.output_bytes = limit,
        _ => return Err("unexpected Jacobian boundary kind".into()),
    }
    Ok(Budget::new(profile)?)
}

fn capacity(operation: u32, kind: BudgetKind) -> TestResult<u64> {
    let mut low = 0;
    let mut high = match kind {
        BudgetKind::StagedPayloadBytes => limits().staged_payload_bytes,
        BudgetKind::PayloadBytes => limits().payload_bytes,
        BudgetKind::OutputBytes => limits().output_bytes,
        _ => return Err("unexpected Jacobian capacity kind".into()),
    };
    while low < high {
        let middle = low + (high - low) / 2;
        match run(operation, &mut bounded(kind, middle)?, false) {
            Ok(()) => high = middle,
            Err(error) => match error.downcast_ref::<Fault>() {
                Some(Fault {
                    kind: FaultKind::Exhausted { budget: actual, .. },
                    ..
                }) if *actual == kind => low = middle + 1,
                _ => return Err(error),
            },
        }
    }
    Ok(high)
}

/// Keying and proof generation succeed at exact Work and payload N and refuse at N minus one.
/// ´test:integration:source-jacobian-work-and-payload-boundaries´
#[test]
fn source_jacobian_work_and_payload_boundaries() -> TestResult {
    for operation in 0..2 {
        let mut measured = ledger()?;
        run(operation, &mut measured, false)?;
        for (kind, needed) in [
            (BudgetKind::Work, measured.usage().work),
            (
                BudgetKind::StagedPayloadBytes,
                capacity(operation, BudgetKind::StagedPayloadBytes)?,
            ),
            (
                BudgetKind::PayloadBytes,
                capacity(operation, BudgetKind::PayloadBytes)?,
            ),
            (
                BudgetKind::OutputBytes,
                capacity(operation, BudgetKind::OutputBytes)?,
            ),
        ] {
            assert!(needed > 0);
            run(operation, &mut bounded(kind, needed)?, false)?;
            let result = run(operation, &mut bounded(kind, needed - 1)?, true);
            assert!(result.is_err());
        }
        let mut cumulative = bounded(BudgetKind::Work, measured.usage().work)?;
        cumulative.work(1, Location::new("prior Jacobian request"))?;
        assert!(run(operation, &mut cumulative, true).is_err());
    }
    Ok(())
}

/// Record new operation costs without relying on numerical timing as proof authority.
/// ´test:integration:source-jacobian-resource-measurements´
#[test]
fn source_jacobian_resource_measurements() -> TestResult {
    let mut measurements = Vec::new();
    for operation in 0..2 {
        let mut measured = ledger()?;
        run(operation, &mut measured, false)?;
        measurements.push([
            u64::from(operation),
            measured.usage().work,
            measured.usage().arithmetic_steps,
            measured.usage().condition_steps,
            capacity(operation, BudgetKind::StagedPayloadBytes)?,
            capacity(operation, BudgetKind::PayloadBytes)?,
            capacity(operation, BudgetKind::OutputBytes)?,
        ]);
    }
    assert_eq!(
        measurements,
        [
            [0, 57_315, 0, 12, 12_284, 5_537, 2_683],
            [1, 606_780, 780, 155, 56_968, 21_558, 16_264],
        ]
    );
    Ok(())
}
