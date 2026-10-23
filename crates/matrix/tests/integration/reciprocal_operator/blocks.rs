// SPDX-License-Identifier: AGPL-3.0-only
//! A general affine coefficient map reads one complete typed block in logical order.

use crate::kernel_oracle::Rational;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{BindingSymbols, Descriptor, readings::PositiveReciprocalMap};
use nrz_matrix::{Binding, Coverage, ExecutionLimits, Heap, HeapLimits, OperationDraft};
use nrz_scalar::LiteralLimits;

/// Dense signed mixing and an offset-only row contain every independent input-box corner.
/// ´test:integration:reciprocal-block-map-encloses-independent-corners´
#[test]
fn reciprocal_block_map_encloses_independent_corners() -> TestResult {
    let mut prepared = crate::reciprocal_testing::block_plan()?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        &mut ledger()?,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let input = crate::conversion_testing::shape(&owner, &Descriptor::ratio(), 2)?;
    let axis = crate::kernel_testing::axis(2)?;
    let output = crate::kernel_testing::shape(&owner, &axis, &axis)?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::positive_reciprocal_map(
        &prepared.case.cas,
        recipe,
        vec![input],
        output,
        LiteralLimits::default(),
        &mut ledger()?,
    )?
    .check(limits)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let symbols =
        BindingSymbols::declare(&Descriptor::ratio(), [2, 1], [true, true], &mut ledger()?)?;
    let source = heap.import_symbols(
        &symbols,
        &[4.0_f64, 8.0],
        Some(&[0.5, 1.0]),
        Some(&[0.125, 0.25]),
        Coverage::DeclaredBounds,
    )?;
    let bound = Binding::from_symbols(operation, vec![source], limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    corners(&heap, bundle)
}

/// The oracle uses exact integer fractions and checks both separate error channels.
fn corners(heap: &Heap, bundle: nrz_matrix::PublishedBundle<f64>) -> TestResult {
    for (channel, denominators) in [(0, [2, 1]), (1, [8, 4])] {
        for left in [-1, 1] {
            for right in [-1, 1] {
                let first = Rational::new(2, 1)?
                    .divide(Rational::new(4, 1)?.add(Rational::new(left, denominators[0])?)?)?;
                let second = Rational::new(3, 1)?
                    .divide(Rational::new(8, 1)?.add(Rational::new(right, denominators[1])?)?)?;
                let expected = [
                    first.add(second.multiply(Rational::new(2, 1)?)?)?,
                    second.subtract(first)?,
                    Rational::new(5, 1)?,
                    first
                        .multiply(Rational::new(3, 1)?)?
                        .subtract(second.multiply(Rational::new(2, 1)?)?)?,
                ];
                for (index, exact) in expected.into_iter().enumerate() {
                    let (center, total, error) = crate::kernel_testing::entry(heap, bundle, index)?;
                    assert!(exact.covered(center, if channel == 0 { total } else { error })?);
                }
            }
        }
    }
    Ok(())
}
