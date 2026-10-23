// SPDX-License-Identifier: AGPL-3.0-only
//! Live coverage fixtures retain actual CAS proofs and exercise complete numerical transactions.

use super::*;
use crate::binding::adapters::fixtures::{
    Case, Fixture, TestResult, boundaries, ledger, matrix, real, state_case,
};
use crate::{EntryTicket, Heap, HeapLimits};
use nrz_bindings::{
    Budget,
    readings::{
        ReadingRelation, Representation, RepresentationDefinition,
        algebra::readings::{AffineMap, EntryIndex, SelectionSpec},
    },
};

/// Exact graph and complete read-only coverage fixture.
type Plan = (Rc<Cas>, ReadingCoverage<usize>);

/// One full state and both disjoint components share a complete two-entry allocation.
fn state() -> TestResult<Plan> {
    plan(state_case()?)
}

/// Construct all three representations and their exact pairwise intersections.
fn plan(mut case: Case) -> TestResult<Plan> {
    let current = case.fixture.define(
        case.whole.family(),
        "i",
        SelectionSpec::Entry(EntryIndex { row: 1, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![case.whole.entries()[1].clone()],
    )?;
    let mut adapters = Vec::new();
    for (reading, symbol, indices) in [
        (&case.whole, "z", vec![0, 1]),
        (&case.component, "v", vec![0]),
        (&current, "i", vec![1]),
    ] {
        let symbol = case.fixture.symbol(symbol)?;
        let representation = Representation::prove(
            &mut case.fixture.cas,
            reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(u32::try_from(indices.len())?, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let entries = indices
            .into_iter()
            .map(|index| EntryTicket { region: 0, index })
            .collect();
        adapters.push(ReadingAdapter::new(
            representation,
            entries,
            &mut ledger()?,
        )?);
    }
    let mut relations = Vec::new();
    for (index, reading) in [(0, &case.component), (1, &current)] {
        let left = case
            .whole
            .project_selected(&mut case.fixture.cas, &[index], &mut ledger()?)?;
        let right = reading.project_selected(&mut case.fixture.cas, &[0], &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut case.fixture.cas,
            left,
            right,
            &mut ledger()?,
        )?);
    }
    let universe = vec![
        EntryTicket {
            region: 0,
            index: 0,
        },
        EntryTicket {
            region: 0,
            index: 1,
        },
    ];
    let coverage = ReadingCoverage::new(universe, adapters, relations, &mut ledger()?)?;
    Ok((Rc::new(case.fixture.cas), coverage))
}

/// Import through the same production allowance used by all exact adapters.
fn imported<P: Scalar + 'static>(policy: DependencyPolicy) -> TestResult<(Heap, ReadingImport<P>)> {
    let (graph, coverage) = state()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let region = ReadingRegion {
        extent: Extent::new(2, 1)?,
        values: vec![P::ONE; 2],
    };
    let imported = heap.import_readings(
        graph,
        coverage,
        Vec::new(),
        &[region],
        policy,
        &mut ledger()?,
    )?;
    Ok((heap, imported))
}

/// Exact cached intersections invalidate the changed component and whole state, preserving its disjoint twin.
/// ´test:unit:live-readings-preserve-disjoint-component´
#[test]
fn live_readings_preserve_disjoint_component() -> TestResult {
    survival::<f32>()?;
    survival::<f64>()
}

/// The same entry-version and proof assertions hold at either supported precision.
fn survival<P: Scalar + 'static>() -> TestResult {
    let (mut heap, imported) = imported::<P>(DependencyPolicy::Entries)?;
    let [whole, voltage, current] = imported.bindings() else {
        return Err("bindings".into());
    };
    assert_eq!(heap.reading_entries(whole)?, vec![P::ONE; 2]);
    heap.replace_reading_entries(current, &[P::ZERO], &mut ledger()?)?;
    assert_eq!(heap.reading_entries(voltage)?, vec![P::ONE]);
    assert_eq!(whole.state(), SymbolState::Unavailable);
    assert_eq!(current.state(), SymbolState::Unavailable);
    assert!(heap.reading_entries(whole).is_err());
    assert!(heap.reading_entries(current).is_err());
    Ok(())
}

/// An explicitly unavailable fine inventory grants no disjointness claim.
/// ´test:unit:live-readings-coarse-fallback-invalidates-family´
#[test]
fn live_readings_coarse_fallback_invalidates_family() -> TestResult {
    let (mut heap, imported) = imported::<f64>(DependencyPolicy::WholeBase)?;
    let [whole, voltage, current] = imported.bindings() else {
        return Err("bindings".into());
    };
    heap.replace_reading_entries(current, &[0.0], &mut ledger()?)?;
    for binding in [whole, voltage, current] {
        assert_eq!(binding.state(), SymbolState::Unavailable);
        assert!(heap.reading_entries(binding).is_err());
    }
    Ok(())
}

/// Wrong actual universes and finite-payload mutations refuse before exposing any allocation.
/// ´test:unit:live-reading-import-refuses-uncovered-payload´
#[test]
fn live_reading_import_refuses_uncovered_payload() -> TestResult {
    for values in [vec![1.0], vec![1.0, 2.0, 3.0], vec![1.0, f64::INFINITY]] {
        let (graph, coverage) = state()?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let region = ReadingRegion {
            extent: Extent::new(values.len(), 1)?,
            values,
        };
        assert!(
            heap.import_readings(
                graph,
                coverage,
                Vec::new(),
                &[region],
                DependencyPolicy::Entries,
                &mut ledger()?
            )
            .is_err()
        );
        assert_eq!(heap.descriptors.iter().count(), 0);
    }
    Ok(())
}

/// Failed writes preserve every entry version and all published reading states.
/// ´test:unit:live-reading-write-refusal-is-atomic´
#[test]
fn live_reading_write_refusal_is_atomic() -> TestResult {
    let (mut heap, imported) = imported::<f64>(DependencyPolicy::Entries)?;
    let whole = imported.bindings().first().ok_or("whole")?;
    assert!(
        heap.replace_reading_entries(whole, &[0.0, f64::NAN], &mut ledger()?)
            .is_err()
    );
    assert_eq!(heap.reading_entries(whole)?, vec![1.0; 2]);
    let foreign = Heap::new(HeapLimits::default())?;
    assert!(foreign.reading_entries(whole).is_err());
    Ok(())
}

/// Live import and write metadata obey exact cumulative Work and staged-payload boundaries.
/// ´test:unit:live-reading-resource-boundaries´
#[test]
fn live_reading_resource_boundaries() -> TestResult {
    for write in [false, true] {
        boundaries(|limits, precharge| {
            let mut budget = Budget::new(limits)?;
            budget.work(precharge, super::admission::AT)?;
            if write {
                let (mut heap, imported) = imported::<f64>(DependencyPolicy::Entries)?;
                heap.replace_reading_entries(
                    imported.bindings().get(2).ok_or("current")?,
                    &[0.0],
                    &mut budget,
                )?;
            } else {
                let (graph, coverage) = state()?;
                let mut heap = Heap::new(HeapLimits::default())?;
                let region = ReadingRegion {
                    extent: Extent::new(2, 1)?,
                    values: vec![1.0_f64; 2],
                };
                heap.import_readings(
                    graph,
                    coverage,
                    Vec::new(),
                    &[region],
                    DependencyPolicy::Entries,
                    &mut budget,
                )?;
            }
            Ok(budget.usage().work)
        })?;
    }
    Ok(())
}

/// A homogeneous result family is declared beside a recipe, before it owns any floats.
fn result_plan() -> TestResult<Plan> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
    ])?;
    let quantity = nrz_bindings::Descriptor::ratio();
    let reference = nrz_bindings::ReferenceDefinition::unit("ratio", &quantity)?;
    let meaning =
        nrz_bindings::readings::EntryMeaning::new(quantity, reference, None, &mut ledger()?)?;
    let family = nrz_bindings::readings::CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![meaning.clone(); 2],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        vec![meaning.clone(); 2],
    )?;
    let component = fixture.define(
        &family,
        "v",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![meaning],
    )?;
    plan(Case {
        fixture,
        whole,
        component,
    })
}

/// A solve vector and both component readings publish with one complete numerical bundle.
/// ´test:unit:solve-readings-publish-atomically´
#[test]
fn solve_readings_publish_atomically() -> TestResult {
    for refuse in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let operator = heap.import_represented(
            Extent::new(2, 2)?,
            &nrz_bindings::Descriptor::ratio(),
            &[2.0_f64, 0.0, 0.0, 4.0],
        )?;
        let rhs = heap.import_represented(
            Extent::new(2, 1)?,
            &nrz_bindings::Descriptor::ratio(),
            &[2.0_f64, 8.0],
        )?;
        let limits = crate::ExecutionLimits::default();
        let mut bound = crate::Binding::new(
            crate::solve_testing::draft(2, 1)?.check(limits)?,
            vec![operator.value(), rhs.value()],
            limits,
        )
        .validate(&mut heap)?;
        let (graph, coverage) = result_plan()?;
        let readings = bound.predeclare_result_readings(
            graph,
            coverage,
            DependencyPolicy::Entries,
            &mut ledger()?,
        )?;
        for reading in &readings {
            assert_eq!(reading.state(), SymbolState::Pending);
            assert!(bound.reading_entries(reading).is_err());
        }
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        if refuse {
            candidate.outputs[0].centers[0] = f64::NAN;
        }
        let result = bound.publish(candidate);
        if refuse {
            assert!(result.is_err());
            for reading in &readings {
                assert_eq!(reading.state(), SymbolState::Unavailable);
            }
            assert_eq!(heap.descriptors.iter().count(), 2);
        } else {
            result?;
            assert_eq!(heap.reading_entries(&readings[0])?, vec![1.0, 2.0]);
            assert_eq!(heap.reading_entries(&readings[1])?, vec![1.0]);
            assert_eq!(heap.reading_entries(&readings[2])?, vec![2.0]);
            assert_eq!(heap.descriptors.iter().count(), 5);
        }
    }
    Ok(())
}

/// Dropping an unevaluated reservation makes all its shared-entry result readings unavailable.
/// ´test:unit:result-reading-abandonment-is-total´
#[test]
fn result_reading_abandonment_is_total() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        Extent::new(2, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &[1.0_f64, 2.0],
    )?;
    let shape = crate::solve_testing::shapes(2, 1)?[1].clone();
    let limits = crate::ExecutionLimits::default();
    let mut bound = crate::Binding::new(
        crate::OperationDraft::copy(vec![shape]).check(limits)?,
        vec![source.value()],
        limits,
    )
    .validate(&mut heap)?;
    let (graph, coverage) = result_plan()?;
    let readings = bound.predeclare_result_readings(
        graph,
        coverage,
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    drop(bound);
    for reading in &readings {
        assert_eq!(reading.state(), SymbolState::Unavailable);
        assert!(heap.reading_entries(reading).is_err());
    }
    assert_eq!(heap.descriptors.iter().count(), 1);
    Ok(())
}

/// A selected copy preserves its exact source reading and survives only disjoint entry writes.
/// ´test:unit:selected-reading-copy-retains-transitive-lineage´
#[test]
fn selected_reading_copy_retains_transitive_lineage() -> TestResult {
    let (graph, coverage) = result_plan()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let region = ReadingRegion {
        extent: Extent::new(2, 1)?,
        values: vec![1.0_f64, 2.0],
    };
    let imported = heap.import_readings(
        graph,
        coverage,
        Vec::new(),
        &[region],
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let voltage = imported.bindings().get(1).ok_or("voltage")?;
    let current = imported.bindings().get(2).ok_or("current")?;
    let selected = heap.select_reading(voltage)?;
    assert_eq!(selected.symbol().reading(), Some(voltage.reading()));
    let limits = crate::ExecutionLimits::default();
    let operation = crate::binding_testing::operation(1, 1)?;
    let bound = crate::Binding::from_symbols(operation, vec![selected.clone()], limits)
        .validate(&mut heap)?;
    let planned = bound.result_bindings().first().ok_or("copy")?.clone();
    assert_eq!(planned.symbol().source(), Some(selected.symbol()));
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    drop(outcome);
    heap.replace_reading_entries(current, &[0.0], &mut ledger()?)?;
    assert_eq!(
        heap.value_by_symbol(selected.symbol(), &selected)?
            .region()
            .get(0, 0)?,
        1.0
    );
    assert_eq!(
        heap.value_by_symbol(planned.symbol(), &planned)?
            .region()
            .get(0, 0)?,
        1.0
    );
    heap.replace_reading_entries(voltage, &[0.0], &mut ledger()?)?;
    assert!(heap.value_by_symbol(selected.symbol(), &selected).is_err());
    assert!(heap.value_by_symbol(planned.symbol(), &planned).is_err());
    Ok(())
}

mod checks;
mod domain;
mod overlap;
