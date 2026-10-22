// SPDX-License-Identifier: AGPL-3.0-only
//! Bindings/scalar/accuracy adapters retain exact map identity and numerical companion targets.

use crate::conversion_oracle::covers;
use crate::conversion_testing::{TestResult, contract, measure, reference_owner, shape};
use crate::{Extent, Heap, HeapLimits, RecipeEvidence};
use nrz_bindings::{Descriptor, Role};

/// The recorded coefficients retain their own exact-definition enclosures and displacements.
/// ´test:crate:conversion-record-retains-lowered-constant-evidence´
#[test]
fn conversion_record_retains_lowered_constant_evidence() -> TestResult {
    let declarations = reference_owner(&Descriptor::ratio())?;
    let source = Descriptor::ratio();
    let target = Descriptor::new(
        source.kind().clone(),
        measure(100, 1, 0, 1)?,
        Role::Difference,
    )?;
    let input_shape = shape(&declarations, &source, 1)?;
    let output_shape = shape(&declarations, &target, 1)?;
    let map = declarations.conversion_map(&source, &target)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &source, &[125.0_f64])?;
    let limits = crate::ExecutionLimits::default();
    let operation =
        crate::OperationDraft::convert(vec![contract(input_shape, output_shape, map.clone())?])
            .check(limits)?;
    let bound = crate::Binding::new(operation, vec![input.value()], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let output = *outcome.output().first().ok_or("output")?;
    let record = heap.operation_record(outcome.record())?;
    let Some(RecipeEvidence::Conversion(claim)) = record.evidence().first() else {
        return Err("conversion evidence".into());
    };
    let scale = claim.constants().first().ok_or("scale")?;
    assert_eq!(claim.map(), &map);
    assert!(covers(scale.center(), scale.error(), 1, 100)?);
    assert!(covers(
        heap.value(output.value())?.region().get(0, 0)?,
        heap.companion(output.evaluation().ok_or("error")?)?
            .region()
            .get(0, 0)?,
        5,
        4
    )?);
    assert_eq!(
        heap.value(output.value())?.coverage(),
        crate::Coverage::RepresentedInputs
    );
    Ok(())
}
