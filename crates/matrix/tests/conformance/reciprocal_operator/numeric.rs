// SPDX-License-Identifier: AGPL-3.0-only
//! The finite reciprocal schedule refuses at each independent numerical phase boundary.

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::{draft, proved};
use nrz_matrix::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError};

/// Numerical work, scratch and slot boundaries are independent and precede publication.
/// ´test:integration:reciprocal-work-and-scratch-boundaries´
#[test]
fn reciprocal_work_and_scratch_boundaries() -> TestResult {
    let (prepared, recipe) = proved()?;
    let proposal = draft(&prepared, &recipe)?;
    for limits in [
        ExecutionLimits::new(2, 955, 956),
        ExecutionLimits::new(2, 956, 955),
        ExecutionLimits::new(1, 956, 956),
    ] {
        assert!(matches!(
            proposal.clone().check(limits),
            Err(OperationError::ResourceLimit)
        ));
    }
    let limits = ExecutionLimits::new(2, 956, 956);
    let operation = proposal.check(limits)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    for (entry, value) in recipe.frame().entries().iter().zip([0.025_f64, 0.000_01]) {
        inputs.push(
            heap.import_represented(Extent::new(1, 1)?, entry.descriptor(), &[value])?
                .value(),
        );
    }
    let bound = Binding::new(operation, inputs, limits).validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    let bundle = crate::kernel_testing::output(&outcome)?;
    for (index, numerator, denominator, value) in [(1, -1, 40, 0.025), (2, 1, 100_000, 0.000_01)] {
        let (center, total, error) = crate::kernel_testing::entry(&heap, bundle, index)?;
        let expected = crate::reciprocal_testing::oracle::Rational::new(numerator, denominator)?
            .divide(&crate::reciprocal_testing::oracle::Rational::binary(value)?)?;
        assert!(expected.covered(center, total)?);
        assert!(expected.covered(center, error)?);
    }
    Ok(())
}

/// Unused source domains and unrepresentable reciprocal images cannot leave partial numerical bundles.
/// ´test:integration:reciprocal-unused-input-and-overflow-refuse-atomically´
#[test]
fn reciprocal_unused_input_and_overflow_refuse_atomically() -> TestResult {
    refused::<f32>()?;
    refused::<f64>()
}

/// Every formal input remains bound even when its affine coefficient column is entirely zero.
fn refused<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let mut prepared = crate::reciprocal_testing::unused_input_plan()?;
    let recipe = nrz_bindings::readings::PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition.clone(),
        &mut crate::reading_testing::ledger()?,
    )?;
    let limits = ExecutionLimits::default();
    let operation = draft(&prepared, &recipe)?.check(limits)?;
    for values in [[P::ONE, P::ZERO], [P::ZERO.next_up(), P::ONE]] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut inputs = Vec::new();
        for (entry, value) in recipe.frame().entries().iter().zip(values) {
            inputs.push(
                heap.import_represented(Extent::new(1, 1)?, entry.descriptor(), &[value])?
                    .value(),
            );
        }
        let usage = heap.provenance_usage();
        let bound = Binding::new(operation.clone(), inputs.clone(), limits).validate(&mut heap)?;
        let outputs = bound.result_bindings().to_vec();
        assert!(bound.evaluate(bound.normalized_inputs()?).is_err());
        drop(bound);
        assert!(
            outputs
                .iter()
                .all(|output| output.state() == nrz_matrix::SymbolState::Unavailable)
        );
        assert_eq!(heap.provenance_usage(), usage);
        for (input, value) in inputs.into_iter().zip(values) {
            assert!(heap.value(input)?.region().get(0, 0)?.same_bits(value));
        }
    }
    Ok(())
}
