// SPDX-License-Identifier: AGPL-3.0-only
//! Exact statistical allocation observations retain the original sources and publication roles.

use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::vector;
use nrz_matrix::{Heap, HeapLimits, JointLimits, OperationDraft, StatisticsInventory};
use nrz_scalar::Scalar;

/// Complete generic capture and cross covariance retain measured counts, capacities, peaks and metadata.
/// ´test:integration:statistical-allocation-costs-are-exact´
#[test]
fn statistical_allocation_costs_are_exact() -> TestResult {
    let single = measured::<f32>()?;
    let double = measured::<f64>()?;
    assert_eq!(
        single,
        [
            [46, 198, 34, 616, 376, 5_128, 1_536],
            [46, 198, 34, 616, 376, 4_960, 1_536],
            [22, 116, 16, 368, 144, 3_496, 1_280],
        ]
    );
    assert_eq!(
        double,
        [
            [46, 198, 34, 1_232, 752, 5_128, 3_072],
            [46, 198, 34, 1_232, 752, 4_960, 3_072],
            [22, 116, 16, 736, 288, 3_496, 2_560],
        ]
    );
    Ok(())
}
/// A two-entry copy includes explicit means, then reloads selected propagated parameters.
fn measured<P: Scalar + 'static>() -> TestResult<[[usize; 7]; 3]> {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = vector::<P>(&mut heap, true)?;
    let copied = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![contract.clone()]),
        vec![source.value()],
    )?)?;
    let first = heap
        .moments(heap.moment_companion(copied.value())?)?
        .allocation_inventory()
        .clone();
    let repeated = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![contract]),
        vec![copied.value()],
    )?)?;
    let second = heap
        .moments(heap.moment_companion(repeated.value())?)?
        .allocation_inventory()
        .clone();
    let cross = heap.materialize_cross_covariance(
        copied.value(),
        repeated.value(),
        JointLimits::default(),
    )?;
    let third = heap.moments(cross)?.allocation_inventory().clone();
    assert!(!first.input_is_propagated(0)?);
    assert!(second.input_is_propagated(0)?);
    assert!(third.input_is_propagated(0)?);
    assert!(third.input_is_propagated(1)?);
    assert_eq!(first.source_columns(), second.source_columns());
    assert_eq!(first.source_columns(), third.source_columns());
    assert_ne!(first.execution(), second.execution());
    assert_ne!(first.execution(), third.execution());
    Ok([counts(&first)?, counts(&second)?, counts(&third)?])
}
/// Requests, primitive coordinates, observable owners/capacities, common peak, metadata and backend bound.
fn counts(inventory: &StatisticsInventory) -> TestResult<[usize; 7]> {
    let mut result = [
        inventory.allocations().len(),
        0,
        0,
        0,
        inventory.peak_payload_bytes(),
        inventory.metadata_bytes(),
        inventory.backend_allowance_bytes(),
    ];
    for allocation in inventory.allocations() {
        result[1] += allocation.float_entries()?;
        if let Some(bytes) = allocation.owned_bytes() {
            result[2] += 1;
            result[3] += bytes;
        }
    }
    Ok(result)
}
