// SPDX-License-Identifier: AGPL-3.0-only
//! Compose nonnegative contributions and preserve full logical multiplicity.

use crate::testing::{Binary, Rational, TestResult, covers};
use nrz_accuracy::{BoundVectorRef, Nonnegative, NormKind};
use nrz_scalar::ArithmeticContext;
use num_bigint::BigInt;

/// Bound two linear combinations and then their combined Euclidean radius.
fn lifecycle<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let entries = [P::ONE, P::from_u8(2)];
    let coefficients = [P::from_u8(3), P::from_u8(4)];
    let inputs = BoundVectorRef::new(&entries)?;
    let contraction = inputs.dot(BoundVectorRef::new(&coefficients)?, &context)?;
    let total = contraction.add(Nonnegative::ONE, &context)?;
    covers(total, &Rational::from_integer(BigInt::from(12)));
    let output = [total.get(), P::from_u8(5)];
    let summary = BoundVectorRef::new(&output)?.norm(NormKind::Euclidean, &context)?;
    let upper = summary.upper().get().rational();
    assert!(&upper * &upper >= Rational::from_integer(BigInt::from(169)));
    let symmetric = [P::ONE, P::from_u8(2), P::from_u8(2), P::from_u8(3)];
    let full = BoundVectorRef::new(&symmetric)?.norm(NormKind::Frobenius, &context)?;
    let radius = full.upper().get().rational();
    assert!(&radius * &radius >= Rational::from_integer(BigInt::from(18)));
    assert_eq!(full.entry_count(), 4);
    Ok(())
}

/// Composition remains outward and duplicated matrix entries contribute twice.
/// ´test:integration:bound-vector-composition-lifecycle´
#[test]
fn bound_vector_composition_lifecycle() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}
