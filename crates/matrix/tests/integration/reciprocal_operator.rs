// SPDX-License-Identifier: AGPL-3.0-only
//! The varying original LC operator crosses exact normalization, bounded execution and publication.

mod blocks;
mod stale;

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use crate::reciprocal_testing::{draft, proved};
use nrz_bindings::BindingSymbols;
use nrz_matrix::{
    Binding, Coverage, DependencyPolicy, ExecutionLimits, Heap, HeapLimits, SymbolState,
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Both formats enclose independently computed original LC coefficients for changed positive inputs.
/// ´test:integration:varying-lc-reciprocal-operator-publishes-complete-readings´
#[test]
fn varying_lc_reciprocal_operator_publishes_complete_readings() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}

/// A complete descriptor family and every native companion remain pending until the same commit.
fn lifecycle<P: Scalar + 'static>() -> TestResult {
    let (mut prepared, recipe) = proved()?;
    let coverage = crate::reciprocal_testing::readings::coverage(&mut prepared.case)?;
    let operation = draft(&prepared, &recipe)?.check(ExecutionLimits::default())?;
    let graph = Rc::new(prepared.case.cas);
    for multiplier in [[1, 1], [2, 1], [1, 2], [4, 4]] {
        let values = [
            P::from_u8(multiplier[0]) / P::from_u8(40),
            P::from_u8(multiplier[1]) / (P::from_u8(100) * P::from_u8(100) * P::from_u8(10)),
        ];
        let totals = [values[0] / P::from_u8(8), values[1] / P::from_u8(8)];
        let errors = [totals[0] / P::from_u8(4), totals[1] / P::from_u8(4)];
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = import(&mut heap, &recipe, &values, &totals, &errors)?;
        let mut bound =
            Binding::from_symbols(operation.clone(), inputs, ExecutionLimits::default())
                .validate(&mut heap)?;
        let planned = bound.predeclare_result_readings(
            Rc::clone(&graph),
            coverage.clone(),
            DependencyPolicy::WholeBase,
            &mut ledger()?,
        )?;
        assert_eq!(planned[0].state(), SymbolState::Pending);
        assert!(bound.reading_entries(&planned[0]).is_err());
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outcome = bound.publish(candidate)?;
        assert_eq!(planned[0].state(), SymbolState::Published);
        let bundle = crate::kernel_testing::output(&outcome)?;
        assert_eq!(
            heap.reading_entries(&planned[0])?,
            heap.value(bundle.value())?.region().export()?
        );
        for (index, source, reference) in [
            (1, 0, Rational::new(-1, 40)?),
            (2, 1, Rational::new(1, 100_000)?),
        ] {
            let (center, total, error) = crate::kernel_testing::entry(&heap, bundle, index)?;
            for sign in [-1, 1] {
                let input = Rational::binary(values[source].to_f64())?;
                let bound = Rational::binary(totals[source].to_f64())?
                    .multiply(&Rational::new(sign, 1)?)?;
                let nominal = Rational::binary(errors[source].to_f64())?
                    .multiply(&Rational::new(sign, 1)?)?;
                assert!(
                    reference
                        .divide(&input.add(&bound)?)?
                        .covered(center, total)?
                );
                assert!(
                    reference
                        .divide(&input.add(&nominal)?)?
                        .covered(center, error)?
                );
            }
            assert!(error > 0.0 && error < total);
        }
        for index in [0, 3] {
            let (center, total, error) = crate::kernel_testing::entry(&heap, bundle, index)?;
            assert_eq!([center.to_bits(), total.to_bits(), error.to_bits()], [0; 3]);
        }
    }
    Ok(())
}

/// Independent declared physical boxes and numerical-only errors enter through the ordinary symbol API.
fn import<P: Scalar + 'static>(
    heap: &mut Heap,
    recipe: &nrz_bindings::readings::PositiveReciprocalMap,
    values: &[P; 2],
    totals: &[P; 2],
    errors: &[P; 2],
) -> TestResult<Vec<nrz_matrix::ValueBinding<P>>> {
    let mut result = Vec::new();
    for (index, meaning) in recipe.frame().entries().iter().enumerate() {
        let symbols =
            BindingSymbols::declare(meaning.descriptor(), [1, 1], [true, true], &mut ledger()?)?;
        result.push(heap.import_symbols(
            &symbols,
            &values[index..=index],
            Some(&totals[index..=index]),
            Some(&errors[index..=index]),
            Coverage::DeclaredBounds,
        )?);
    }
    Ok(result)
}
