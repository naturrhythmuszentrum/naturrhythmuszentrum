// SPDX-License-Identifier: AGPL-3.0-only
//! Historical decisions cannot authorize stale later reads, even after equal-value writes.

use super::{Status, TestResult, execute, ledger, setup};
use nrz_scalar::Scalar;

/// A selected original impedance or radius replacement retires every dependent continuation.
/// ´test:integration:lc-driven-continuation-revalidates-selected-inputs´
#[test]
fn lc_driven_continuation_revalidates_selected_inputs() -> TestResult {
    stale::<f32>()?;
    stale::<f64>()
}

/// Equal-value source and companion writes cannot revive a historical successful selection.
fn stale<P: Scalar + 'static>() -> TestResult {
    for companion in [false, true] {
        let mut case = setup::<P>(false)?;
        execute(
            &mut case,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )?;
        let source = &case.sources[0][if companion { 3 } else { 0 }];
        if companion {
            let values = case.heap.reading_companion_entries(source, 4)?;
            case.heap
                .replace_reading_companion_entries(source, 4, &values, &mut ledger()?)?;
        } else {
            let values = case.heap.reading_entries(source)?;
            case.heap
                .replace_reading_entries(source, &values, &mut ledger()?)?;
        }
        assert!(
            case.batch
                .phase(super::FIRST_ROW, &case.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        assert!(
            case.batch
                .current(0, &case.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        assert_eq!(case.batch.statuses(0), Some([Status::Successful; 3]));
        case.batch
            .phase(super::LAST_ROW, &case.heap, &case.context, &mut ledger()?)?;
        case.batch
            .invariants()
            .result(1, &case.heap, &case.context, &mut ledger()?)?;
    }
    Ok(())
}

/// A refused original middle decision keeps both stable outer rows and the independent root.
/// ´test:integration:lc-driven-continuation-retains-refused-middle´
#[test]
fn lc_driven_continuation_retains_refused_middle() -> TestResult {
    refused::<f32>()?;
    refused::<f64>()
}

/// The middle original source becomes stale before its first decision attempt.
fn refused<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false)?;
    let source = &case.sources[1][0];
    let values = case.heap.reading_entries(source)?;
    case.heap
        .replace_reading_entries(source, &values, &mut ledger()?)?;
    execute(
        &mut case,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    assert_eq!(case.batch.point_keys().collect::<Vec<_>>(), [41, 9, 73]);
    assert_eq!(
        case.batch.statuses(1),
        Some([Status::BindingRefused, Status::Blocked, Status::Blocked])
    );
    for row in [0, 2] {
        assert_eq!(case.batch.statuses(row), Some([Status::Successful; 3]));
        case.batch
            .current(row, &case.heap, &case.context, &mut ledger()?)?;
        case.batch
            .phase(row, &case.heap, &case.context, &mut ledger()?)?;
    }
    case.batch
        .invariants()
        .result(1, &case.heap, &case.context, &mut ledger()?)?;
    Ok(())
}
