// SPDX-License-Identifier: AGPL-3.0-only
//! Exact slot, kind and original-reference identities are required independently of float values.

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::{plan, proved, shapes};
use nrz_bindings::{
    Limits,
    readings::{PositiveReciprocalMap, algebra::Cas},
};
use nrz_matrix::OperationDraft;
use nrz_scalar::LiteralLimits;

/// Repeated formal coordinates cannot be bound as independently varying input slots.
/// ´test:integration:reciprocal-operator-refuses-duplicate-formal-inputs´
#[test]
fn reciprocal_operator_refuses_duplicate_formal_inputs() -> TestResult {
    let mut prepared = plan()?;
    prepared.definition.inputs[1] = prepared.definition.inputs[0].clone();
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

/// Equal-sized scalar slots cannot exchange inductance and capacitance meanings.
/// ´test:integration:reciprocal-operator-refuses-swapped-input-kinds´
#[test]
fn reciprocal_operator_refuses_swapped_input_kinds() -> TestResult {
    let (prepared, recipe) = proved()?;
    let (mut inputs, output) = shapes(&recipe)?;
    inputs.swap(0, 1);
    assert!(
        OperationDraft::positive_reciprocal_map(
            &prepared.case.cas,
            recipe,
            inputs,
            output,
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Portable equality does not let a foreign exact graph authenticate a local recipe key.
/// ´test:integration:reciprocal-operator-refuses-foreign-correspondence´
#[test]
fn reciprocal_operator_refuses_foreign_correspondence() -> TestResult {
    let (_, recipe) = proved()?;
    let (inputs, output) = shapes(&recipe)?;
    let graph = Cas::new(Limits::default())?;
    assert!(
        OperationDraft::positive_reciprocal_map(
            &graph,
            recipe,
            inputs,
            output,
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Whole-family cancellation cannot hide an extra denominator whose numerical source is unbound.
/// ´test:integration:reciprocal-original-denominators-remain-guarded´
#[test]
fn reciprocal_original_denominators_remain_guarded() -> TestResult {
    let (mut prepared, target) = crate::reciprocal_testing::hidden_denominator()?;
    let witness = prepared.case.cas.prove_reading_equality(
        &prepared.case.context,
        prepared.definition.original.expression(),
        target.expression(),
        &mut ledger()?,
    )?;
    assert!(witness.is_discharged());
    assert!(matches!(
        PositiveReciprocalMap::prove(
            &mut prepared.case.cas,
            &prepared.frame,
            prepared.definition,
            &mut ledger()?
        ),
        Err(nrz_bindings::readings::ReadingError::Definition)
    ));
    Ok(())
}

/// Irrational references retain exact correspondence but cannot enter this rational numerical kernel.
/// ´test:integration:reciprocal-irrational-coefficient-export-refuses´
#[test]
fn reciprocal_irrational_coefficient_export_refuses() -> TestResult {
    let mut prepared = crate::reciprocal_testing::irrational_plan()?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        &mut ledger()?,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let input =
        crate::conversion_testing::shape(&owner, recipe.frame().entries()[0].descriptor(), 1)?;
    let output = crate::conversion_testing::shape(&owner, &nrz_bindings::Descriptor::ratio(), 1)?;
    assert!(matches!(
        OperationDraft::positive_reciprocal_map(
            &prepared.case.cas,
            recipe,
            vec![input],
            output,
            LiteralLimits::default(),
            &mut ledger()?
        ),
        Err(nrz_matrix::OperationError::Reading(
            nrz_bindings::readings::ReadingError::Units(
                nrz_bindings::UnitError::UnsupportedExactConstantExport
            )
        ))
    ));
    Ok(())
}

/// Statistical operands keep their model identity and refuse this deterministic-only recipe.
/// ´test:integration:reciprocal-statistical-operands-refuse-without-erasure´
#[test]
fn reciprocal_statistical_operands_refuse_without_erasure() -> TestResult {
    use nrz_matrix::{
        Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapLimits,
        JointLimits, JointModel, Source, SourceFactor, TargetMap,
    };
    let (prepared, recipe) = crate::reciprocal_testing::proved()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = Vec::new();
    let mut sources = Vec::new();
    for (entry, value) in recipe.frame().entries().iter().zip([0.025_f64, 0.000_01]) {
        inputs.push(
            heap.import_represented(Extent::new(1, 1)?, entry.descriptor(), &[value])?
                .value(),
        );
        sources.push(Source::new(entry.descriptor(), 1)?);
    }
    let limits = JointLimits::default();
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[1.0, 1.0], &[1.0], limits)?,
        &std::array::from_fn::<_, 4, _>(|_| CovarianceBlock::Known),
        limits,
    )?;
    let identity = model.id();
    let targets = [
        TargetMap::capture(&heap, inputs[0])?,
        TargetMap::capture(&heap, inputs[1])?,
    ];
    let companion = heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0], vec![1]]),
        limits,
    )?;
    let execution = ExecutionLimits::default();
    let operation = crate::reciprocal_testing::draft(&prepared, &recipe)?.check(execution)?;
    assert!(matches!(
        Binding::new(operation, inputs, execution).validate(&mut heap),
        Err(nrz_matrix::OperationError::Heap(
            nrz_matrix::HeapError::MissingCoverage
        ))
    ));
    assert_eq!(heap.statistical(companion)?.model().id(), identity);
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
