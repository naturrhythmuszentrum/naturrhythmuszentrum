// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit inverses own typed identity semantics and their own complete certificates.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{axis, entry, execute, output, owner, shape};
use crate::solve_testing::TestResult;
use nrz_matrix::{
    Extent, Heap, HeapLimits, OperationDraft, RecipeEvidence, ShapeLimits, ShapeRecipe,
};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// Inverse publication restores reversed axes and encloses the bounded inverse family.
/// ´test:integration:typed-identity-inverse-has-its-own-bounded-certificate´
#[test]
fn typed_identity_inverse_has_its_own_bounded_certificate() -> TestResult {
    inverse::<f32>()?;
    inverse::<f64>()
}

/// The exact perturbed inverse differs from the private represented inverse witness.
fn inverse<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let variables = axis(2)?;
    let equations = axis(2)?;
    let operator = shape(&declarations, &variables, &equations)?;
    let solution = shape(&declarations, &equations, &variables)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::ZERO, P::ZERO, P::from_u8(4)],
        &[P::ONE / P::from_u8(8), P::ZERO, P::ZERO, P::ZERO],
    )?;
    let outcome = execute(
        &mut heap,
        OperationDraft::inverse(operator, solution, ShapeLimits::default())?,
        vec![source.value()],
    )?;
    let result = output(&outcome)?;
    crate::solve_public::check(
        &heap,
        result,
        &[2.0, 0.0, 0.0, 4.0],
        &[1.0, 0.0, 0.0, 1.0],
        2,
    )?;
    for (index, expected) in [
        Rational::new(8, 15)?,
        Rational::new(0, 1)?,
        Rational::new(0, 1)?,
        Rational::new(1, 4)?,
    ]
    .into_iter()
    .enumerate()
    {
        let (center, total, _) = entry(&heap, result, index)?;
        assert!(expected.covered(center, total)?);
    }
    let record = heap.operation_record(outcome.record())?;
    assert_eq!(record.inputs().len(), 1);
    let restored = record
        .operation()
        .outputs()
        .first()
        .ok_or("output")?
        .shape();
    assert_eq!(restored.recipe(), ShapeRecipe::Inverse);
    assert_eq!(restored.domain(), &equations);
    assert_eq!(restored.codomain(), &variables);
    let RecipeEvidence::Solve(proof) = record.evidence().first().ok_or("proof")? else {
        return Err("solve proof".into());
    };
    assert!(proof.is_inverse());
    assert_eq!(
        proof.rhs_radius().ok_or("complete solve proof")?.to_bits(),
        0.0_f64.to_bits()
    );
    Ok(())
}
