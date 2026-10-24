// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-box guards and exact correspondence refuse before any result becomes public.

mod numeric;
mod refusals;

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::{draft, plan, proved};
use nrz_bindings::readings::PositiveReciprocalMap;
use nrz_matrix::{
    Binding, DependencyPolicy, DomainReason, ExecutionLimits, Extent, Heap, HeapLimits,
    KernelError, OperationError, SymbolState,
};

/// A positive center cannot discharge a zero-containing total box, and a negative box refuses too.
/// ´test:integration:positive-reciprocal-box-refusal-is-atomic´
#[test]
fn positive_reciprocal_box_refusal_is_atomic() -> TestResult {
    let (mut prepared, recipe) = proved()?;
    let coverage = crate::reciprocal_testing::readings::coverage(&mut prepared.case)?;
    let operation = draft(&prepared, &recipe)?.check(ExecutionLimits::default())?;
    let graph = std::rc::Rc::new(prepared.case.cas);
    for (center, radius) in [(0.01, 0.01), (-0.01, 0.001)] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let first = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            recipe.frame().entries()[0].descriptor(),
            &[0.025_f64],
            &[0.001],
        )?;
        let second = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            recipe.frame().entries()[1].descriptor(),
            &[center],
            &[radius],
        )?;
        let mut bound = Binding::new(
            operation.clone(),
            vec![first.value(), second.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let planned = bound.result_bindings().to_vec();
        let readings = bound.predeclare_result_readings(
            std::rc::Rc::clone(&graph),
            coverage.clone(),
            DependencyPolicy::WholeBase,
            &mut ledger()?,
        )?;
        let failure = bound
            .evaluate(bound.normalized_inputs()?)
            .err()
            .ok_or("invalid box admitted")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Kernel(KernelError::Domain {
                entry: 1,
                reason: DomainReason::NonpositiveInput,
                ..
            })
        ));
        drop(bound);
        assert_eq!(readings[0].state(), SymbolState::Unavailable);
        assert!(heap.reading_entries(&readings[0]).is_err());
        assert!(
            planned
                .iter()
                .all(|value| value.state() == SymbolState::Unavailable)
        );
    }
    Ok(())
}

/// A changed coefficient sign cannot claim correspondence to the original LC similarity.
/// ´test:integration:reciprocal-operator-rejects-changed-exact-image´
#[test]
fn reciprocal_operator_rejects_changed_exact_image() -> TestResult {
    let mut prepared = plan()?;
    prepared.definition.map =
        crate::reading_testing::map(2, 4, &[0, 0, 1, 0, 0, 1, 0, 0], &[0; 4])?;
    assert!(
        PositiveReciprocalMap::prove(
            &mut prepared.case.cas,
            &prepared.frame,
            prepared.definition,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

mod costs;
