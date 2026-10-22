// SPDX-License-Identifier: AGPL-3.0-only
//! Private independent contractions retain exact rational term meanings.

use crate::contractions::{ContractionWork, product};
use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, owner, shape};
use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft};
use nrz_bindings::Descriptor;
use nrz_scalar::ArithmeticContext;

/// The verifier encloses exact dots and each separate contribution without a backend candidate.
/// ´test:crate:independent-contraction-retains-each-product-term´
#[test]
fn independent_contraction_retains_each_product_term() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[3.0_f64],
        &[0.125],
    )?;
    let right = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::copy(vec![contract.clone(), contract]).check(limits)?;
    let bound =
        Binding::new(operation, vec![left.value(), right.value()], limits).validate(&mut heap)?;
    let inputs = bound.normalized_inputs()?;
    let first = inputs.views.first().ok_or("left")?;
    let second = inputs.views.get(1).ok_or("right")?;
    let mut work = ContractionWork::new(1);
    let result = product(first, second, 0, 0, &mut work, &ArithmeticContext::new()?)?;
    let exact = Rational::new(6, 1)?;
    assert!(Rational::binary(result.nominal.lower().get())?.at_most(exact)?);
    assert!(exact.at_most(Rational::binary(result.nominal.upper().get())?)?);
    for ((numerator, denominator), term) in [(3, 4), (1, 4), (1, 32)]
        .into_iter()
        .zip(result.contributions)
    {
        assert!(Rational::new(numerator, denominator)?.covered(0.0, term.get())?);
    }
    Ok(())
}
