// SPDX-License-Identifier: AGPL-3.0-only
//! Typed precision binding, nearest ties and explicit resource boundaries.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, owner, shape};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError,
    PrecisionContract,
};
use nrz_scalar::Precision;

/// The scalar nearest rule and matrix displacement region agree at both parity directions of a tie.
/// ´test:integration:precision-halfway-ties-have-exact-rational-displacement´
#[test]
fn precision_halfway_ties_have_exact_rational_displacement() -> TestResult {
    for (bits, numerator, expected) in [
        (0x3ff0_0000_1000_0000, 16_777_217, 0x3f80_0000),
        (0x3ff0_0000_3000_0000, 16_777_219, 0x3f80_0002),
    ] {
        let declarations = owner()?;
        let quantity = Descriptor::ratio();
        let contract = PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &quantity, &[f64::from_bits(bits)])?;
        let limits = ExecutionLimits::default();
        let operation = OperationDraft::precision(vec![contract], Precision::F64, Precision::F32)
            .check(limits)?;
        let bound = Binding::<f64, f32>::convert_precision(operation, vec![source.value()], limits)
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outputs = bound.publish(candidate)?.into_output();
        let output = outputs.first().ok_or("output")?;
        let center = heap.value(output.value())?.region().get(0, 0)?;
        let error = heap
            .companion(output.evaluation().ok_or("evaluation")?)?
            .region()
            .get(0, 0)?;
        assert_eq!(center.to_bits(), expected);
        assert!(covers(
            f64::from(center),
            f64::from(error),
            numerator,
            16_777_216
        )?);
    }
    Ok(())
}

/// A precision transaction counts all three destination regions before allocating them.
/// ´test:integration:precision-conversion-work-limit-boundaries´
#[test]
fn precision_conversion_work_limit_boundaries() -> TestResult {
    for capacity in [2, 3, 4] {
        let declarations = owner()?;
        let quantity = Descriptor::ratio();
        let contract = PrecisionContract::new(shape(&declarations, &quantity, 1)?, &quantity)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_represented(Extent::new(1, 1)?, &quantity, &[0.125_f32])?;
        let operation = OperationDraft::precision(vec![contract], Precision::F32, Precision::F64)
            .check(ExecutionLimits::default())?;
        let bound = Binding::<f32, f64>::convert_precision(
            operation,
            vec![source.value()],
            ExecutionLimits::new(1, capacity, capacity),
        )
        .validate(&mut heap);
        if capacity < 3 {
            assert!(matches!(bound, Err(OperationError::ResourceLimit)));
        } else {
            let bound = bound?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let outputs = bound.publish(candidate)?.into_output();
            let output = outputs.first().ok_or("output")?;
            assert!(covers(
                heap.value(output.value())?.region().get(0, 0)?,
                heap.companion(output.evaluation().ok_or("evaluation")?)?
                    .region()
                    .get(0, 0)?,
                1,
                8
            )?);
        }
    }
    Ok(())
}
