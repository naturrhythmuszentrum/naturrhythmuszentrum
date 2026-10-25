// SPDX-License-Identifier: AGPL-3.0-only
//! Original source identities, complete models and positive whole boxes remain required.

use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{
    Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Heap, JointLimits, JointModel,
    ReadingBinding, ReciprocalDerivativeContract, ReciprocalDerivativeEvaluation, Source,
    SourceFactor, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

/// A prepared experiment retains the same original readings for both exact and numerical evidence.
pub(super) struct Experiment {
    pub(super) heap: Heap,
    pub(super) original: [ReadingBinding<f64>; 2],
    pub(super) selected: Vec<ValueBinding<f64>>,
    pub(super) derivative: ReciprocalDerivativeEvaluation<f64>,
}

pub(super) fn prepare_inputs(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    values: [[f64; 3]; 2],
    budget: &mut Budget,
) -> TestResult<Experiment> {
    let (mut heap, original) = fixture.import_with_budget(values, budget)?;
    let selected = original
        .iter()
        .map(|source| heap.select_reading(source, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let derivative = heap.evaluate_reciprocal_derivative(
        contract,
        &original,
        &ArithmeticContext::new()?,
        budget,
    )?;
    Ok(Experiment {
        heap,
        original,
        selected,
        derivative,
    })
}

pub(super) fn attach(experiment: &mut Experiment, known: bool) -> TestResult {
    let handles =
        super::selected_joint::capture_handles(&mut experiment.heap, &experiment.selected)?;
    let sources = experiment
        .selected
        .iter()
        .map(|value| Source::new(value.symbol().descriptor(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let cross = if known {
        CovarianceBlock::Known
    } else {
        CovarianceBlock::Unknown
    };
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[0.007_812_5, 0.001_953_125], &[1.0], limits)?,
        &[
            CovarianceBlock::Known,
            cross.clone(),
            cross,
            CovarianceBlock::Known,
        ],
        limits,
    )?;
    let draft = CompanionSetDraft::new(
        model,
        super::selected_joint::capture_targets(&experiment.heap, handles)?,
        [vec![0], vec![1]],
    );
    experiment.heap.publish_joint(draft, limits)?;
    Ok(())
}

pub(super) fn request(
    fixture: &Fixture,
    experiment: &Experiment,
    budget: &mut Budget,
) -> TestResult<Binding<f64>> {
    Ok(Binding::from_symbols(
        super::operator(fixture)?,
        experiment.selected.clone(),
        ExecutionLimits::default(),
    )
    .with_reciprocal_statistics(experiment.derivative.clone(), budget)?)
}

/// Equal-bit writes to either original source or any original radius invalidate live moments.
/// ´test:integration:lc-native-reciprocal-rechecks-every-original-parameter´
#[test]
fn lc_native_reciprocal_rechecks_every_original_parameter() -> TestResult {
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = super::reciprocal_statistics::derivative(&mut fixture, &mut budget)?;
    for change in 0..6 {
        let mut experiment = prepare_inputs(
            &fixture,
            &contract,
            [
                [0.25, 0.015_625, 0.000_976_562_5],
                [0.0625, 0.003_906_25, 0.000_244_140_625],
            ],
            &mut budget,
        )?;
        attach(&mut experiment, true)?;
        let next = request(&fixture, &experiment, &mut budget)?;
        let bound = request(&fixture, &experiment, &mut budget)?
            .validate_with_resource_budget(&mut experiment.heap, &budget)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let outcome = bound.publish(candidate)?;
        let output = crate::kernel_testing::output(&outcome)?;
        let moments = experiment.heap.moment_companion(output.value())?;
        super::invariants::mutate(&mut experiment.heap, &experiment.original, change)?;
        assert!(
            next.validate_with_resource_budget(&mut experiment.heap, &budget)
                .is_err()
        );
        assert!(experiment.heap.moments(moments).is_err());
        assert!(experiment.heap.operation_record(outcome.record()).is_err());
        assert!(
            experiment
                .derivative
                .values(&experiment.heap, &ArithmeticContext::new()?, &mut budget)
                .is_err()
        );
        assert!(contract.exact().native().witness().is_discharged());
    }
    Ok(())
}

/// An equal-valued reimport or foreign heap cannot replace the selected derivative inputs.
/// ´test:integration:lc-native-reciprocal-refuses-substituted-inputs´
#[test]
fn lc_native_reciprocal_refuses_substituted_inputs() -> TestResult {
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = super::reciprocal_statistics::derivative(&mut fixture, &mut budget)?;
    let values = [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]];
    let mut original = prepare_inputs(&fixture, &contract, values, &mut budget)?;
    let mut foreign = prepare_inputs(&fixture, &contract, values, &mut budget)?;
    attach(&mut foreign, true)?;
    foreign.derivative = original.derivative.clone();
    assert!(
        request(&fixture, &foreign, &mut budget)?
            .validate_with_resource_budget(&mut foreign.heap, &budget)
            .is_err()
    );
    let replacement = fixture.import_into(&mut original.heap, values, &mut budget)?;
    original.selected = replacement
        .iter()
        .map(|source| original.heap.select_reading(source, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    attach(&mut original, true)?;
    assert!(
        request(&fixture, &original, &mut budget)?
            .validate_with_resource_budget(&mut original.heap, &budget)
            .is_err()
    );
    assert_eq!(original.heap.provenance_usage().records(), 0);
    assert_eq!(foreign.heap.provenance_usage().records(), 0);
    Ok(())
}

/// A derivative supplies no missing source coverage or unknown cross-correlation.
/// ´test:integration:lc-native-reciprocal-requires-complete-source-model´
#[test]
fn lc_native_reciprocal_requires_complete_source_model() -> TestResult {
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = super::reciprocal_statistics::derivative(&mut fixture, &mut budget)?;
    for unknown in [false, true] {
        let mut experiment = prepare_inputs(
            &fixture,
            &contract,
            [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
            &mut budget,
        )?;
        if unknown {
            attach(&mut experiment, false)?;
        }
        let result = request(&fixture, &experiment, &mut budget)?
            .validate_with_resource_budget(&mut experiment.heap, &budget);
        if unknown {
            assert!(matches!(
                result,
                Err(nrz_matrix::OperationError::Statistics(
                    nrz_matrix::StatisticsError::Joint(nrz_matrix::JointError::MissingCrossBlock)
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(nrz_matrix::OperationError::Statistics(
                    nrz_matrix::StatisticsError::MissingSource { slot: 0, entry: 0 }
                ))
            ));
        }
        assert_eq!(experiment.heap.provenance_usage().records(), 0);
    }
    Ok(())
}

/// A positive represented point never substitutes for a strictly positive whole input box.
/// ´test:integration:lc-native-reciprocal-keeps-whole-box-domain´
#[test]
fn lc_native_reciprocal_keeps_whole_box_domain() -> TestResult {
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = super::reciprocal_statistics::derivative(&mut fixture, &mut budget)?;
    for input in 0..2 {
        let mut values = [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]];
        values[input][1] = values[input][0];
        let mut experiment = prepare_inputs(&fixture, &contract, values, &mut budget)?;
        attach(&mut experiment, true)?;
        let bound = request(&fixture, &experiment, &mut budget)?
            .validate_with_resource_budget(&mut experiment.heap, &budget)?;
        assert!(bound.evaluate(bound.normalized_inputs()?).is_err());
        drop(bound);
        assert_eq!(experiment.heap.provenance_usage().records(), 0);
    }
    Ok(())
}

/// A fixed symbolic direction cannot omit variation from the declared two-source model.
/// ´test:integration:lc-native-reciprocal-refuses-fixed-direction-subsets´
#[test]
fn lc_native_reciprocal_refuses_fixed_direction_subsets() -> TestResult {
    use nrz_bindings::readings::{ReciprocalJacobianCache, ReciprocalJacobianKey};
    use nrz_scalar::LiteralLimits;
    use std::rc::Rc;
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let key =
        ReciprocalJacobianKey::new(graph, fixture.law.operator(), &["C".into()], &mut budget)?;
    let mut cache = ReciprocalJacobianCache::new();
    let exact = cache.get_or_derive(graph, &key, &mut budget)?;
    let contract =
        ReciprocalDerivativeContract::new(graph, exact, LiteralLimits::default(), &mut budget)?;
    let mut experiment = prepare_inputs(
        &fixture,
        &contract,
        [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        &mut budget,
    )?;
    attach(&mut experiment, true)?;
    let result = Binding::from_symbols(
        super::operator(&fixture)?,
        experiment.selected,
        ExecutionLimits::default(),
    )
    .with_reciprocal_statistics(experiment.derivative, &mut budget);
    assert!(matches!(
        result,
        Err(nrz_matrix::ReciprocalDerivativeError::Identity)
    ));
    assert_eq!(experiment.heap.provenance_usage().records(), 0);
    Ok(())
}

/// Equal mathematical coefficients cannot substitute different physical reference identities.
/// ´test:integration:lc-native-reciprocal-refuses-substituted-references´
#[test]
fn lc_native_reciprocal_refuses_substituted_references() -> TestResult {
    let mut budget = super::reciprocal_statistics::policy()?;
    let mut fixture = Fixture::with_store_limits(["5", "1/10", "1/2000"], budget.limits().clone())?;
    let contract = super::reciprocal_statistics::derivative(&mut fixture, &mut budget)?;
    let changed = different_references(&mut fixture, &mut budget)?;
    let original = fixture.law.operator();
    let changed_map = changed.reciprocal_map().ok_or("changed reciprocal map")?;
    assert_eq!(changed_map.reuse_key(), original.reuse_key());
    assert_eq!(changed_map.inputs(), original.inputs());
    assert_ne!(changed_map.frame().entries(), original.frame().entries());
    let experiment = prepare_inputs(
        &fixture,
        &contract,
        [[0.25, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]],
        &mut budget,
    )?;
    let result = Binding::from_symbols(changed, experiment.selected, ExecutionLimits::default())
        .with_reciprocal_statistics(experiment.derivative, &mut budget);
    assert!(matches!(
        result,
        Err(nrz_matrix::ReciprocalDerivativeError::Identity)
    ));
    assert_eq!(experiment.heap.provenance_usage().records(), 0);
    Ok(())
}

/// Only source-free reference names change; all exact magnitudes and native matrix syntax agree.
fn different_references(
    fixture: &mut Fixture,
    budget: &mut Budget,
) -> TestResult<nrz_matrix::CheckedOperation> {
    use nrz_bindings::{
        ReferenceDefinition, Scale,
        readings::{
            CoordinateNormalization, EntryMeaning, PositiveReciprocalMap, ReciprocalMapDefinition,
        },
    };
    use nrz_scalar::LiteralLimits;
    use std::rc::Rc;
    let original = fixture.law.operator();
    let mut entries = Vec::new();
    for (index, entry) in original.frame().entries().iter().enumerate() {
        let reference = ReferenceDefinition::new(
            format!("substituted reference {index}"),
            entry.descriptor(),
            Scale::Exact(
                entry
                    .magnitude()
                    .as_rational()
                    .ok_or("rational reference")?
                    .clone(),
            ),
        )?;
        entries.push(EntryMeaning::new(
            entry.descriptor().clone(),
            reference,
            None,
            budget,
        )?);
    }
    let graph = Rc::get_mut(&mut fixture.graph).ok_or("original graph")?;
    let frame = CoordinateNormalization::new(graph, original.frame().context(), &entries, budget)?;
    let proposed = ReciprocalMapDefinition {
        inputs: original.inputs().to_vec(),
        shape: original.shape(),
        map: original.map().clone(),
        original: original.original().clone(),
    };
    let definition = PositiveReciprocalMap::prove(graph, &frame, proposed, budget)?;
    let (shapes, output) = crate::reciprocal_testing::shapes(&definition)?;
    Ok(nrz_matrix::OperationDraft::positive_reciprocal_map(
        graph,
        definition,
        shapes,
        output,
        LiteralLimits::default(),
        budget,
    )?
    .check(ExecutionLimits::default())?)
}
