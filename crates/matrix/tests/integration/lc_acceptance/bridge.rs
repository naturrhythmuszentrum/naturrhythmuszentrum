// SPDX-License-Identifier: AGPL-3.0-only
//! Restored selection validates its original state, both covariance targets and every parameter.

use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use crate::spectral_testing::Fixture;
use nrz_matrix::{
    Coverage, DependencyPolicy, Extent, Heap, HeapLimits, ReadingImport, ReadingRegion,
};
use nrz_scalar::ArithmeticContext;
use std::rc::Rc;

/// Self covariance transforms both factors, while cross covariance transforms its matching ordered axis.
/// ´test:integration:restored-selection-preserves-both-covariance-axes´
#[test]
fn restored_selection_preserves_both_covariance_axes() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::covariance_plan(fixture.law.state().entries())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let state = covariance_import(&plan, &mut heap)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &state.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let expected = [
        [(1, 200), (-1, 32), (-1, 32), (25, 4)],
        [(1, 40), (-5, 32), (-1, 320), (5, 8)],
        [(1, 40), (-1, 320), (-5, 32), (5, 8)],
    ];
    for (index, entries) in expected.into_iter().enumerate() {
        let centers = normalized
            .covariance_centers(index)
            .ok_or("covariance centers")?;
        let errors = normalized
            .covariance_errors(index)
            .ok_or("covariance errors")?;
        for (position, (top, bottom)) in entries.into_iter().enumerate() {
            assert!(Rational::new(top, bottom)?.covered(centers[position], errors[position])?);
        }
    }
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    let selected =
        heap.select_restored_reading(&restored, &state.bindings()[1], &context, &mut ledger()?)?;
    drop(restored);
    drop(normalized);
    heap.compact()?;
    let origin = heap.coordinate_origin(&selected)?;
    assert_eq!(origin.covariance_count(), 3);
    for index in 0..3 {
        assert_eq!(
            origin
                .covariance_definition(index)
                .ok_or("covariance")?
                .companion()
                .node(),
            plan.companions[index + 2].companion().node()
        );
        for (position, (top, bottom)) in [(1, 8), (-1, 64), (-1, 64), (1, 16)]
            .into_iter()
            .enumerate()
        {
            assert!(Rational::new(top, bottom)?.covered(
                origin.covariance_centers(index).ok_or("centers")?[position],
                origin.covariance_errors(index).ok_or("errors")?[position]
            )?);
        }
    }
    Ok(())
}

/// Writing either state, either radius or a covariance parameter refuses every old selected result.
/// ´test:integration:restored-selection-invalidates-every-covariance-source´
#[test]
fn restored_selection_invalidates_every_covariance_source() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::covariance_plan(fixture.law.state().entries())?;
    let context = ArithmeticContext::new()?;
    for change in 0..7 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let state = covariance_import(&plan, &mut heap)?;
        let normalized = heap.normalize_reading(
            &state.bindings()[0],
            &plan.contract,
            &context,
            &mut ledger()?,
        )?;
        let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
        let selected = heap.select_restored_reading(
            &restored,
            &state.bindings()[1],
            &context,
            &mut ledger()?,
        )?;
        if change < 2 {
            let target = &state.bindings()[if change == 0 { 0 } else { 3 }];
            let values = heap.reading_entries(target)?;
            heap.replace_reading_entries(target, &values, &mut ledger()?)?;
        } else {
            let index = change - 2;
            let values = heap.reading_companion_entries(&state.bindings()[0], index)?;
            heap.replace_reading_companion_entries(
                &state.bindings()[0],
                index,
                &values,
                &mut ledger()?,
            )?;
        }
        assert!(heap.coordinate_origin(&selected).is_err());
        assert!(heap.value_by_symbol(selected.symbol(), &selected).is_err());
        assert!(
            heap.select_restored_reading(&restored, &state.bindings()[1], &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Generated numerical radii preserve represented-input assurance and reject foreign or normalized inputs.
/// ´test:integration:restored-selection-preserves-assurance-and-refuses-substitution´
#[test]
fn restored_selection_preserves_assurance_and_refuses_substitution() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let represented = heap.import_readings(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        Vec::new(),
        &[ReadingRegion {
            extent: Extent::new(2, 1)?,
            values: vec![3.0_f64, 0.25],
        }],
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &represented.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    assert!(
        heap.select_restored_reading(
            &normalized,
            &represented.bindings()[1],
            &context,
            &mut ledger()?
        )
        .is_err()
    );
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    assert!(
        heap.select_restored_reading(
            &restored,
            &represented.bindings()[0],
            &context,
            &mut ledger()?
        )
        .is_err()
    );
    let unrelated = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    assert!(
        heap.select_restored_reading(
            &restored,
            &unrelated.bindings()[1],
            &context,
            &mut ledger()?
        )
        .is_err()
    );
    let mut foreign = Heap::new(HeapLimits::default())?;
    assert!(
        foreign
            .select_restored_reading(
                &restored,
                &represented.bindings()[1],
                &context,
                &mut ledger()?
            )
            .is_err()
    );
    let selected = heap.select_restored_reading(
        &restored,
        &represented.bindings()[1],
        &context,
        &mut ledger()?,
    )?;
    assert_eq!(
        heap.value_by_symbol(selected.symbol(), &selected)?
            .coverage(),
        Coverage::RepresentedInputs
    );
    assert_eq!(
        heap.coordinate_origin(&selected)?.coverage(),
        Coverage::RepresentedInputs
    );
    Ok(())
}

fn covariance_import(
    plan: &super::state_fixture::Plan,
    heap: &mut Heap,
) -> TestResult<ReadingImport<f64>> {
    let mut regions = [[3.0, 0.25], [0.5, 0.0625], [0.125, 0.015_625], [2.0, 0.5]]
        .into_iter()
        .map(|values| {
            Ok(ReadingRegion {
                extent: Extent::new(2, 1)?,
                values: values.to_vec(),
            })
        })
        .collect::<TestResult<Vec<_>>>()?;
    for _ in 0..3 {
        regions.push(ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![0.125, -0.015_625, -0.015_625, 0.0625],
        });
    }
    Ok(heap.import_readings(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        plan.companions.clone(),
        &regions,
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?)
}

/// Native publications preserve both floating formats and survive dropped snapshots and compaction.
/// ´test:integration:restored-selection-retains-precision-and-storage-lifetime´
#[test]
fn restored_selection_retains_precision_and_storage_lifetime() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    precision::<f32>(&plan)?;
    precision::<f64>(&plan)
}

fn precision<P: nrz_scalar::Scalar + 'static>(plan: &super::state_fixture::Plan) -> TestResult {
    let regions = [
        [P::from_u8(3), P::ONE / P::from_u8(4)],
        [P::ONE / P::from_u8(2), P::ONE / P::from_u8(16)],
        [P::ONE / P::from_u8(8), P::ONE / P::from_u8(64)],
    ]
    .into_iter()
    .map(|values| {
        Ok(ReadingRegion {
            extent: Extent::new(2, 1)?,
            values: values.to_vec(),
        })
    })
    .collect::<TestResult<Vec<_>>>()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let state = heap.import_readings(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        plan.companions.clone(),
        &regions,
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &state.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    let selected =
        heap.select_restored_reading(&restored, &state.bindings()[2], &context, &mut ledger()?)?;
    let expected = [
        restored.centers()[1],
        restored.totals()[1],
        restored.errors()[1],
    ]
    .map(|value| value.to_f64().to_bits());
    drop(restored);
    drop(normalized);
    drop(state);
    heap.compact()?;
    let value = heap.value_by_symbol(selected.symbol(), &selected)?;
    let total = heap.companion(value.bound().ok_or("total")?)?;
    let error = heap.companion(value.evaluation().ok_or("error")?)?;
    let actual = [
        value.region().get(0, 0)?,
        total.region().get(0, 0)?,
        error.region().get(0, 0)?,
    ]
    .map(|value| value.to_f64().to_bits());
    assert_eq!(actual, expected);
    assert_eq!(
        heap.coordinate_origin(&selected)?
            .contract()
            .frame()
            .entries(),
        plan.contract.frame().entries()
    );
    Ok(())
}

/// A failed output reservation consumes no descriptor slot and leaves the original bundle current.
/// ´test:integration:restored-selection-capacity-refusal-is-atomic´
#[test]
fn restored_selection_capacity_refusal_is_atomic() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = super::state_fixture::plan(fixture.law.state().entries())?;
    let mut heap = Heap::new(HeapLimits::new(5, 1_048_576, 4096))?;
    let state = super::state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &state.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    assert!(
        heap.select_restored_reading(&restored, &state.bindings()[1], &context, &mut ledger()?)
            .is_err()
    );
    for _ in 0..2 {
        heap.import_represented(
            Extent::new(1, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[1.0_f64],
        )?;
    }
    assert!(
        heap.import_represented(
            Extent::new(1, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[1.0_f64]
        )
        .is_err()
    );
    for binding in state.bindings() {
        assert!(heap.reading_entries(binding).is_ok());
    }
    Ok(())
}

/// Each owner admits its literal Work and byte ceilings and refuses one below or prior Work.
/// ´test:integration:restored-coordinate-owner-costs-and-boundaries´
#[test]
fn restored_coordinate_owner_costs_and_boundaries() -> TestResult {
    let fixture = Fixture::new(["5", "1/10", "1/2000"])?;
    let ordinary = super::state_fixture::plan(fixture.law.state().entries())?;
    let statistical = super::state_fixture::covariance_plan(fixture.law.state().entries())?;
    for (plan, covariance, operation, profile) in [
        (&ordinary, false, 2, [13_455, 83_367, 75_978]),
        (&statistical, true, 0, [50_102, 80_698, 80_546]),
        (&statistical, true, 1, [47_023, 81_474, 81_370]),
        (&statistical, true, 2, [13_455, 103_575, 96_186]),
    ] {
        owner_boundaries(plan, covariance, operation, profile)?;
    }
    Ok(())
}

fn owner_boundaries(
    plan: &super::state_fixture::Plan,
    covariance: bool,
    operation: usize,
    profile: [u64; 3],
) -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let state = if covariance {
        covariance_import(plan, &mut heap)?
    } else {
        super::state_fixture::import(plan, &mut heap, DependencyPolicy::Entries)?
    };
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &state.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    let mut attempt = |budget: &mut nrz_bindings::Budget| -> TestResult {
        match operation {
            0 => {
                heap.normalize_reading(&state.bindings()[0], &plan.contract, &context, budget)?;
            }
            1 => {
                heap.restore_reading(&normalized, &context, budget)?;
            }
            _ => {
                heap.select_restored_reading(&restored, &state.bindings()[1], &context, budget)?;
            }
        }
        Ok(())
    };
    let mut resources = ledger()?;
    attempt(&mut resources)?;
    assert_eq!(resources.usage().work, profile[0]);
    for (kind, needed) in profile.into_iter().enumerate() {
        attempt(&mut super::costs::policy(kind, needed, 0)?)?;
        assert!(attempt(&mut super::costs::policy(kind, needed - 1, 0)?).is_err());
    }
    assert!(attempt(&mut super::costs::policy(0, profile[0], 1)?).is_err());
    Ok(())
}
