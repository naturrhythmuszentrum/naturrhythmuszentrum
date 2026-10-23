// SPDX-License-Identifier: AGPL-3.0-only
//! Cache misses, hits and reading composition retain cumulative Work and exact payload bounds.

use crate::testing::common::TestResult;
use crate::testing::jacobian_composition::varying_coefficient;
use crate::testing::jacobians::{coordinates, product};
use crate::testing::map_inverses::{ledger, limits};
use nrz_cas::jacobians::SourceJacobianCache;
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::{Fault, FaultKind, Location};

fn run(operation: u32, budget: &mut Budget, rollback: bool) -> TestResult {
    if operation == 2 {
        let mut case = varying_coefficient()?;
        let before = case.fixture.cas.counts();
        let result = case.fixture.cas.compose_reading_jacobian(
            &case.fixture.context,
            &case.original,
            &case.coordinates,
            &case.readings,
            budget,
        );
        if rollback {
            assert_eq!(case.fixture.cas.counts(), before);
        }
        return Ok(result.map(|_| ())?);
    }
    let (mut case, original, _) = product()?;
    let key =
        case.cas
            .source_jacobian_key(&case.context, &original, &coordinates(), &mut ledger()?)?;
    let mut cache = SourceJacobianCache::new();
    if operation == 1 {
        cache.get_or_derive(&mut case.cas, &key, &mut ledger()?)?;
    }
    let before = case.cas.counts();
    let retained = cache.len();
    let result = cache.get_or_derive(&mut case.cas, &key, budget).map(|_| ());
    if rollback {
        assert_eq!(case.cas.counts(), before);
        assert_eq!(cache.len(), retained);
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
        _ => return Err("unexpected derivative resource".into()),
    }
    Ok(Budget::new(profile)?)
}

fn capacity(operation: u32, kind: BudgetKind) -> TestResult<u64> {
    let mut low = 0;
    let mut high = match kind {
        BudgetKind::StagedPayloadBytes => limits().staged_payload_bytes,
        BudgetKind::PayloadBytes => limits().payload_bytes,
        BudgetKind::OutputBytes => limits().output_bytes,
        _ => return Err("unexpected derivative capacity".into()),
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

/// Every new operation has exact Work, staged and output boundaries with atomic refusal.
/// ´test:integration:reading-jacobian-cache-work-and-payload-boundaries´
#[test]
fn reading_jacobian_cache_work_and_payload_boundaries() -> TestResult {
    for operation in 0..3 {
        let mut measured = ledger()?;
        run(operation, &mut measured, false)?;
        let mut boundaries = vec![
            (BudgetKind::Work, measured.usage().work),
            (
                BudgetKind::StagedPayloadBytes,
                capacity(operation, BudgetKind::StagedPayloadBytes)?,
            ),
            (
                BudgetKind::OutputBytes,
                capacity(operation, BudgetKind::OutputBytes)?,
            ),
        ];
        if operation != 1 {
            boundaries.push((
                BudgetKind::PayloadBytes,
                capacity(operation, BudgetKind::PayloadBytes)?,
            ));
        }
        for (kind, needed) in boundaries {
            assert!(needed > 0);
            run(operation, &mut bounded(kind, needed)?, false)?;
            assert!(run(operation, &mut bounded(kind, needed - 1)?, true).is_err());
        }
        let mut cumulative = bounded(BudgetKind::Work, measured.usage().work)?;
        cumulative.work(1, Location::new("prior reading Jacobian work"))?;
        assert!(run(operation, &mut cumulative, true).is_err());
    }
    Ok(())
}

/// Exact pins expose each operation's measured Work and retained payload capacities.
/// ´test:integration:reading-jacobian-reuse-resource-measurements´
#[test]
fn reading_jacobian_reuse_resource_measurements() -> TestResult {
    let mut measurements = Vec::new();
    for operation in 0..3 {
        let mut measured = ledger()?;
        run(operation, &mut measured, false)?;
        let payload = if operation == 1 {
            0
        } else {
            capacity(operation, BudgetKind::PayloadBytes)?
        };
        measurements.push([
            u64::from(operation),
            measured.usage().work,
            measured.usage().arithmetic_steps,
            measured.usage().condition_steps,
            capacity(operation, BudgetKind::StagedPayloadBytes)?,
            payload,
            capacity(operation, BudgetKind::OutputBytes)?,
        ]);
    }
    assert_eq!(
        measurements,
        [
            [0, 657_128, 780, 155, 57_416, 21_558, 24_040],
            [1, 2_187, 0, 0, 24_040, 0, 24_040],
            [2, 820_924, 1_428, 171, 74_814, 20_714, 39_024],
        ]
    );
    Ok(())
}
