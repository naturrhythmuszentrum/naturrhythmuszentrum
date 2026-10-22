// SPDX-License-Identifier: AGPL-3.0-only
//! Physical squared-voltage descriptors and deterministic interaction radii follow the worked oracle.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, entry, execute, output};
use crate::shape_testing::{axis, dense, owner, quantity};
use nrz_bindings::{Descriptor, Dimension, ExponentPolicy, Registry};
use nrz_matrix::{Elementwise, EntryDraft, Extent, Heap, HeapLimits, OperationDraft};

/// The native square carries squared physical kinds and includes the design's quadratic radii.
/// ´test:integration:squared-voltage-region-encloses-the-worked-interaction´
#[test]
fn squared_voltage_region_encloses_the_worked_interaction() -> TestResult {
    let voltage = quantity("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let squared = quantity("squared-voltage", [4, 2, -6, -2, 0, 0, 0])?;
    let (input, destination) = voltage_shapes(&voltage, &squared)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source =
        heap.import_declared_bounds(Extent::new(2, 1)?, &voltage, &[2.0_f64, 3.0], &[0.1, 0.2])?;
    let result = execute(
        &mut heap,
        OperationDraft::map_elementwise(input, destination, Elementwise::Square)?,
        vec![source.value()],
    )?;
    assert_eq!(
        heap.value(output(&result)?.value())?.region().descriptor(),
        &squared
    );
    for (index, (nominal, numerator, denominator)) in
        [(4, 441, 100), (9, 256, 25)].into_iter().enumerate()
    {
        let (center, radius, error) = entry(&heap, output(&result)?, index)?;
        assert!(Rational::new(nominal, 1)?.covered(center, error)?);
        assert!(Rational::new(numerator, denominator)?.covered(center, radius)?);
        assert!(
            Rational::new(numerator, denominator)?
                .subtract(Rational::new(nominal, 1)?)?
                .at_most(Rational::binary(radius)?)?
        );
    }
    heap.compact()?;
    assert!(heap.operation_record(result.record()).is_ok());
    Ok(())
}

/// Register the physical power relation and independently check both ordered region shapes.
fn voltage_shapes(
    voltage: &Descriptor,
    squared: &Descriptor,
) -> TestResult<(nrz_matrix::CheckedShape, nrz_matrix::CheckedShape)> {
    let exponents = Dimension::integers([2, 0, 0, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_power(
        voltage.kind().clone(),
        exponents
            .exponents()
            .first()
            .ok_or("square exponent")?
            .clone(),
        squared.kind().clone(),
    )?;
    let declarations = owner(registry)?;
    let column = axis(&["ratio"], &Descriptor::ratio())?;
    let input_axis = axis(&["first", "second"], voltage)?;
    let output_axis = axis(&["first-square", "second-square"], squared)?;
    let input = dense(
        &declarations,
        &column,
        &input_axis,
        vec![EntryDraft::Declared(voltage.clone()); 2],
    )?;
    let destination = dense(
        &declarations,
        &column,
        &output_axis,
        vec![EntryDraft::Declared(squared.clone()); 2],
    )?;
    Ok((input, destination))
}
