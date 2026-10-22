// SPDX-License-Identifier: AGPL-3.0-only
//! Complete output reservations include value, total and overlapping numerical regions.

use crate::kernel_testing::output;
use crate::solve_testing::{TestResult, draft};
use nrz_bindings::Descriptor;
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationError};

/// Below, at and above five live scalar slots test the actual output bundle reservation.
/// ´test:integration:solve-output-capacity-boundaries-are-atomic´
#[test]
fn solve_output_capacity_boundaries_are_atomic() -> TestResult {
    for capacity in [4, 5, 6] {
        let mut heap = Heap::new(HeapLimits::new(capacity, capacity, 10))?;
        let operator =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[4.0_f64])?;
        let original = heap.value(operator.value())?.stamp();
        let limits = ExecutionLimits::default();
        let binding = Binding::new(
            draft(1, 1)?.check(limits)?,
            vec![operator.value(), rhs.value()],
            limits,
        )
        .validate(&mut heap);
        if capacity < 5 {
            assert!(matches!(
                binding,
                Err(OperationError::Heap(HeapError::Capacity))
            ));
        } else {
            let bound = binding?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let outcome = bound.publish(candidate)?;
            crate::solve_public::check(&heap, output(&outcome)?, &[2.0], &[4.0], 1)?;
        }
        assert_eq!(heap.value(operator.value())?.stamp(), original);
    }
    Ok(())
}
