// SPDX-License-Identifier: AGPL-3.0-only
//! Derived numerical and product regions overlap their chosen total during solves.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{axis, entry, execute, output, owner, shape};
use crate::solve_testing::TestResult;
use nrz_bindings::Descriptor;
use nrz_matrix::{Extent, Heap, HeapLimits, OperationDraft, RecipeEvidence, ShapeLimits};

/// A product input and an import with the same total give bit-identical solve radii.
/// ´test:integration:solve-consumes-a-product-total-exactly-once´
#[test]
fn solve_consumes_a_product_total_exactly_once() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let rectangle = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64],
        &[0.125],
    )?;
    let second = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let multiplied = execute(
        &mut heap,
        OperationDraft::matmul(rectangle.clone(), rectangle.clone(), ShapeLimits::default())?,
        vec![first.value(), second.value()],
    )?;
    let product = output(&multiplied)?;
    let (center, total, error) = entry(&heap, product, 0)?;
    assert!(Rational::new(2, 1)?.covered(center, error)?);
    let copied_total = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[center],
        &[total],
    )?;
    let operator =
        heap.import_declared_bounds(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64], &[0.0])?;
    let proposal = OperationDraft::solve(
        rectangle.clone(),
        rectangle.clone(),
        rectangle,
        ShapeLimits::default(),
    )?;
    let derived = execute(
        &mut heap,
        proposal.clone(),
        vec![operator.value(), product.value()],
    )?;
    let imported = execute(
        &mut heap,
        proposal,
        vec![operator.value(), copied_total.value()],
    )?;
    for outcome in [&derived, &imported] {
        crate::solve_public::check(&heap, output(outcome)?, &[2.0], &[2.0], 1)?;
        let record = heap.operation_record(outcome.record())?;
        let RecipeEvidence::Solve(proof) = record.evidence().first().ok_or("proof")? else {
            return Err("solve proof".into());
        };
        assert!(
            Rational::binary(proof.rhs_radius().ok_or("complete solve proof")?)?.at_most(
                Rational::binary(total)?.multiply(Rational::new(1_000_001, 1_000_000)?)?
            )?
        );
    }
    assert_eq!(
        entry(&heap, output(&derived)?, 0)?.1.to_bits(),
        entry(&heap, output(&imported)?, 0)?.1.to_bits()
    );
    Ok(())
}
