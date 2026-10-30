// SPDX-License-Identifier: AGPL-3.0-only
//! Original lattice snapshots preserve spent Work and return no partial inventory on refusal.

use super::{decision_fixture::ledger, lattice_inputs};
use crate::binding_testing::TestResult;
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{FaultKind, Location, limits::BudgetKind},
};
use nrz_matrix::{DecisionSnapshotError, Heap, HeapLimits};

/// Every capture and native lease owner has a literal cumulative N/N−1 Work boundary.
/// ´test:integration:lattice-inputs-work-boundaries´
/// ´claim:matrix:lattice-inputs-work-boundaries´
#[test]
fn lattice_inputs_work_boundaries() -> TestResult {
    for (kind, capture_limits, check_limits) in [
        (0, [4124, 4123], [114, 113]),
        (1, [6434, 6433], [178, 177]),
        (2, [6434, 6433], [178, 177]),
    ] {
        let association = lattice_inputs::association(kind)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = lattice_inputs::inputs(&mut heap, &association)?;
        let records = heap.provenance_usage();
        for (position, work) in capture_limits.into_iter().enumerate() {
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            budget.work(17, Location::new("prior lattice capture"))?;
            let result = heap.freeze_lattice_inputs(&association, &inputs, &mut budget);
            if position == 0 {
                let frozen = result?;
                assert_eq!(frozen.usage()[0].work, 17);
                assert_eq!(frozen.usage()[1].work, work);
            } else {
                let Err(DecisionSnapshotError::Resources(fault)) = result else {
                    panic!("one-below capture Work must refuse without a snapshot");
                };
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
            }
            assert!(budget.usage().work > 17);
            assert!(budget.usage().work <= work);
            assert_eq!(heap.provenance_usage(), records);
        }
        let frozen = heap.freeze_lattice_inputs(&association, &inputs, &mut ledger()?)?;
        for (position, work) in check_limits.into_iter().enumerate() {
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            budget.work(17, Location::new("prior lattice lease"))?;
            let result = heap.check_lattice_inputs(&frozen, &mut budget);
            if position == 0 {
                result?;
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(DecisionSnapshotError::Resources(fault)) = result else {
                    panic!("one-below lease Work must refuse");
                };
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
            }
            assert!(budget.usage().work > 17);
            assert!(budget.usage().work <= work);
        }
        heap.check_lattice_inputs(&frozen, &mut ledger()?)?;
        assert_eq!(heap.provenance_usage(), records);
    }
    Ok(())
}

/// Returned and staged byte limits remain independent, including caller-retained payload.
/// ´test:integration:lattice-inputs-storage-boundaries´
/// ´claim:matrix:lattice-inputs-storage-boundaries´
#[test]
fn lattice_inputs_storage_boundaries() -> TestResult {
    let association = lattice_inputs::association(1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = lattice_inputs::inputs(&mut heap, &association)?;
    let records = heap.provenance_usage();
    for (staged_payload_bytes, output_bytes, refusal) in [
        (6293, 6240, None),
        (6292, 6240, Some(BudgetKind::StagedPayloadBytes)),
        (6293, 6239, Some(BudgetKind::OutputBytes)),
    ] {
        let mut budget = Budget::new(Limits {
            staged_payload_bytes,
            output_bytes,
            ..Limits::default()
        })?;
        let at = Location::new("prior lattice payload");
        budget.work(17, at)?;
        let result = budget.with_retained_payload(13, at, |budget| {
            heap.freeze_lattice_inputs(&association, &inputs, budget)
        });
        if let Some(expected) = refusal {
            let Err(DecisionSnapshotError::Resources(fault)) = result else {
                panic!("one-below snapshot storage must refuse");
            };
            let FaultKind::Exhausted { budget: kind, .. } = fault.kind else {
                panic!("snapshot storage must refuse at its resource checker");
            };
            assert_eq!(kind, expected);
            assert_eq!(budget.usage().work, 17);
        } else {
            assert_eq!(result?.retained_payload_bytes(), 6240);
            assert_eq!(budget.usage().work, 6434);
        }
        assert_eq!(heap.provenance_usage(), records);
    }
    Ok(())
}
