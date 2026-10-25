// SPDX-License-Identifier: AGPL-3.0-only
//! Original uncertain source-free energy composes native operations without floating re-imports.

mod recipe;

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Binding, CheckedOperation, Coverage, DependencyPolicy, ExecutionLimits, Heap, Outcome,
    PublishedBundle, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

type Evaluation = Outcome<Vec<PublishedBundle<f64>>>;

const WORK: [usize; 6] = [12, 12, 32, 32, 8, 25];
const ADMISSION: [usize; 6] = [12, 12, 32, 32, 12, 25];

/// All sixteen independent exact box corners are covered by the original source-free energy.
/// ´test:integration:lc-energy-restored-native-box-oracle´
#[test]
fn lc_energy_restored_native_box_oracle() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operations = recipe::operations(&fixture)?;
    let (mut heap, sources) =
        fixture.import([[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let state = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    let components = select(&mut heap, &plan, state.bindings())?;
    let elements = super::invariants::selections(&mut heap, &sources)?;
    let evaluated = evaluate(&mut heap, &operations, &components, &elements)?;
    let output = crate::kernel_testing::output(evaluated.last().ok_or("energy result")?)?;
    oracle(&heap, output)?;
    drop(state);
    heap.compact()?;
    for result in evaluated {
        assert!(heap.operation_record(result.record()).is_ok());
    }
    for component in components {
        assert!(heap.coordinate_origin(&component).is_ok());
    }
    Ok(())
}

/// Original state and both source or radius writes invalidate every dependent energy operation.
/// ´test:integration:lc-energy-invalidates-original-inputs´
#[test]
fn lc_energy_invalidates_original_inputs() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operations = recipe::operations(&fixture)?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    for change in 0..9 {
        let (mut heap, sources) =
            fixture.import([[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]])?;
        let state = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
        let components = select(&mut heap, &plan, state.bindings())?;
        let elements = super::invariants::selections(&mut heap, &sources)?;
        let evaluated = evaluate(&mut heap, &operations, &components, &elements)?;
        match change.cmp(&6) {
            std::cmp::Ordering::Less => {
                super::invariants::mutate(&mut heap, &sources, change)?;
            }
            std::cmp::Ordering::Equal => {
                heap.replace_reading_entries(&state.bindings()[0], &[3.0, 0.25], &mut ledger()?)?;
            }
            std::cmp::Ordering::Greater => {
                let index = change - 7;
                let values = heap.reading_companion_entries(&state.bindings()[0], index)?;
                heap.replace_reading_companion_entries(
                    &state.bindings()[0],
                    index,
                    &values,
                    &mut ledger()?,
                )?;
            }
        }
        let result = evaluated.last().ok_or("energy result")?;
        assert!(heap.operation_record(result.record()).is_err());
        assert!(
            heap.value(crate::kernel_testing::output(result)?.value())
                .is_err()
        );
        for component in &components {
            assert_eq!(heap.coordinate_origin(component).is_ok(), change < 6);
        }
    }
    Ok(())
}

/// Source-free energy and the separately declared driven equation have disjoint experiment inputs.
/// ´test:integration:lc-energy-is-independent-of-driven-experiment´
#[test]
fn lc_energy_is_independent_of_driven_experiment() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operations = recipe::operations(&fixture)?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    for state_write in [false, true] {
        let (mut heap, sources) =
            fixture.import([[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]])?;
        let state = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
        let components = select(&mut heap, &plan, state.bindings())?;
        let elements = super::invariants::selections(&mut heap, &sources)?;
        let evaluated = evaluate(&mut heap, &operations, &components, &elements)?;
        let mut resources = crate::phasor_solve_testing::budget()?;
        let mut driven =
            crate::phasor_solve_testing::case_in_heap(-75.0_f64, 0.0, false, heap, &mut resources)?;
        let publication = driven.publication(&mut resources)?;
        let current = driven.heap.publish_phasor_solve(
            &publication,
            &ArithmeticContext::new()?,
            &mut resources,
        )?;
        let target = if state_write {
            &state.bindings()[0]
        } else {
            &driven.bindings[5]
        };
        let values = driven.heap.reading_entries(target)?;
        driven
            .heap
            .replace_reading_entries(target, &values, &mut ledger()?)?;
        for result in &evaluated {
            assert_eq!(
                driven.heap.operation_record(result.record()).is_ok(),
                !state_write
            );
        }
        assert_eq!(
            driven
                .heap
                .reading_entries(&current.readings().bindings()[0])
                .is_ok(),
            state_write
        );
        for component in &components {
            assert_eq!(
                driven.heap.coordinate_origin(component).is_ok(),
                !state_write
            );
        }
    }
    Ok(())
}

/// Select the restored original state projections, including both radii and fixed references.
fn select(
    heap: &mut Heap,
    plan: &super::state_fixture::Plan,
    state: &[nrz_matrix::ReadingBinding<f64>],
) -> TestResult<Vec<ValueBinding<f64>>> {
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    let normalized = heap.normalize_reading(&state[0], &plan.contract, &context, &mut budget)?;
    let restored = heap.restore_reading(&normalized, &context, &mut budget)?;
    state[1..]
        .iter()
        .map(|component| {
            Ok(heap.select_restored_reading(&restored, component, &context, &mut budget)?)
        })
        .collect()
}

/// Every intermediate is consumed as its actual native result binding.
fn evaluate(
    heap: &mut Heap,
    operations: &[CheckedOperation],
    components: &[ValueBinding<f64>],
    elements: &[ValueBinding<f64>],
) -> TestResult<Vec<Evaluation>> {
    execute(heap, operations, components, elements, false)
}

/// Resource checks use the actual restored input closure before every native result is published.
fn execute(
    heap: &mut Heap,
    operations: &[CheckedOperation],
    components: &[ValueBinding<f64>],
    elements: &[ValueBinding<f64>],
    boundaries: bool,
) -> TestResult<Vec<Evaluation>> {
    let mut results = Vec::new();
    let mut values: Vec<ValueBinding<f64>> = Vec::new();
    for (index, operation) in operations.iter().enumerate() {
        let inputs = operands(index, components, elements, &values);
        if boundaries {
            native_boundaries(heap, operation, &inputs, ADMISSION[index])?;
            if index == 4 {
                addition_kernel_boundaries(operation)?;
            }
        }
        let limits = ExecutionLimits::new(4096, ADMISSION[index], ADMISSION[index]);
        let bound = Binding::from_symbols(operation.clone(), inputs, limits).validate(heap)?;
        let planned = bound.result_bindings()[0].clone();
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        results.push(bound.publish(candidate)?);
        values.push(planned);
    }
    Ok(results)
}

/// The oracle uses exact original dyadic endpoints and never the executor's restored floats.
fn oracle(heap: &Heap, output: PublishedBundle<f64>) -> TestResult {
    let (center, total, error) = crate::kernel_testing::entry(heap, output, 0)?;
    assert_eq!(
        heap.value(output.value())?.coverage(),
        Coverage::DeclaredBounds
    );
    assert!(Rational::new(37, 128)?.covered(center, error)?);
    for inductance in [15, 17] {
        for capacitance in [15, 17] {
            for voltage in [5, 7] {
                for current in [3, 5] {
                    let voltage = Rational::new(voltage, 2)?;
                    let current = Rational::new(current, 16)?;
                    let electric =
                        Rational::new(capacitance, 256)?.multiply(&voltage.multiply(&voltage)?)?;
                    let magnetic =
                        Rational::new(inductance, 64)?.multiply(&current.multiply(&current)?)?;
                    let target = electric.add(&magnetic)?.divide(&Rational::new(2, 1)?)?;
                    assert!(target.covered(center, total)?);
                }
            }
        }
    }
    Ok(())
}

/// Native Work and registry metadata have literal boundaries, separate from uncovered kernel arrays.
/// ´test:integration:lc-energy-native-owner-costs-and-boundaries´
#[test]
fn lc_energy_native_owner_costs_and_boundaries() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let operations = recipe::operations(&fixture)?;
    let (mut heap, sources) =
        fixture.import([[0.25_f64, 0.015_625, 0.0], [0.0625, 0.003_906_25, 0.0]])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let state = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    let components = select(&mut heap, &plan, state.bindings())?;
    let elements = super::invariants::selections(&mut heap, &sources)?;
    let results = execute(&mut heap, &operations, &components, &elements, true)?;
    let mut profiles = Vec::new();
    for result in results {
        let record = heap.operation_record(result.record())?;
        let Some(nrz_matrix::RecipeEvidence::Kernel(evidence)) = record.evidence().first() else {
            return Err("kernel evidence".into());
        };
        let inventory = heap.execution_inventory(result.record())?;
        profiles.push([
            evidence.work(),
            inventory.observed_allocations(),
            inventory.cumulative_payload_bytes(),
            inventory.peak_payload_bytes(),
            inventory.metadata_bytes(),
            inventory.backend_allowance_bytes(),
        ]);
    }
    assert_eq!(profiles, WORK.map(|work| [work, 0, 0, 0, 168, 0]));
    Ok(())
}

/// Metadata and both numerical limits refuse independently before an incomplete result escapes.
fn native_boundaries(
    heap: &mut Heap,
    operation: &CheckedOperation,
    inputs: &[ValueBinding<f64>],
    work: usize,
) -> TestResult {
    for limits in [
        ExecutionLimits::new(4096, work - 1, work),
        ExecutionLimits::new(4096, work, work - 1),
    ] {
        assert!(
            Binding::from_symbols(operation.clone(), inputs.to_vec(), limits)
                .validate(heap)
                .is_err()
        );
    }
    for kind in [1, 2] {
        for (limit, success) in [(168, true), (167, false)] {
            let resources = super::costs::policy(kind, limit, 0)?;
            assert_eq!(
                Binding::from_symbols(
                    operation.clone(),
                    inputs.to_vec(),
                    ExecutionLimits::default()
                )
                .validate_with_resource_budget(heap, &resources)
                .is_ok(),
                success
            );
        }
    }
    Ok(())
}

/// Preserve the exact result bindings between successive native recipe operations.
fn operands(
    index: usize,
    components: &[ValueBinding<f64>],
    elements: &[ValueBinding<f64>],
    values: &[ValueBinding<f64>],
) -> Vec<ValueBinding<f64>> {
    match index {
        0 | 1 => vec![components[index].clone()],
        2 => vec![elements[1].clone(), values[0].clone()],
        3 => vec![elements[0].clone(), values[1].clone()],
        4 => vec![values[2].clone(), values[3].clone()],
        _ => vec![values[4].clone()],
    }
}

/// Addition's eight kernel units are independent of its twelve carried input coordinates.
fn addition_kernel_boundaries(operation: &CheckedOperation) -> TestResult {
    for (work, scratch, success) in [(8, 8, true), (7, 8, false), (8, 7, false)] {
        let draft = nrz_matrix::OperationDraft::add(
            operation.inputs()[0].shape().clone(),
            operation.inputs()[1].shape().clone(),
            nrz_matrix::ShapeLimits::default(),
        )?;
        assert_eq!(
            draft
                .check(ExecutionLimits::new(4096, work, scratch))
                .is_ok(),
            success
        );
    }
    Ok(())
}
