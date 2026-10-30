// SPDX-License-Identifier: AGPL-3.0-only
//! Closed live vector containment, atomic tuple publication and dependent retirement.

use super::{
    decision_fixture::ledger,
    lattice_selection::{self, Selection},
};
use crate::binding_testing::TestResult;
use nrz_bindings::{Budget, LatticeInputs};
use nrz_matrix::{
    Coverage, Handle, Heap, HeapLimits, LiveDecisionError, PreparedLatticeDecision, Value,
    ValueBinding,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits};

/// Explicit existing endpoint decode policy, prepaid on each attempt.
pub(super) const fn literals() -> LiteralLimits {
    LiteralLimits::new(32, 32, 1024)
}

/// Import the actual phase or torus inputs, retaining separate total and numerical radii.
pub(super) fn inputs(
    heap: &mut Heap,
    association: &LatticeInputs,
    kind: u8,
) -> TestResult<Vec<ValueBinding<f64>>> {
    association
        .operands()
        .iter()
        .map(|operand| {
            let (center, total) = match operand.name() {
                "axis_a" => {
                    if kind == 0 {
                        (7.75, 0.125)
                    } else {
                        (19.5, 0.5)
                    }
                }
                "axis_b" => (9.5, 0.5),
                "period0" | "period1" => (6.25, 0.125),
                "zero" => (0.0, 0.0),
                _ => return Err("unexpected original source".into()),
            };
            Ok(heap.import_symbols(
                operand.symbols(),
                &[center],
                Some(&[total]),
                Some(&[total / 2.0]),
                Coverage::DeclaredBounds,
            )?)
        })
        .collect()
}

/// Prepare all live containment before the complete independent mathematical/physical selection.
pub(super) fn candidate(
    heap: &Heap,
    source: Selection,
    association: &LatticeInputs,
    inputs: &[ValueBinding<f64>],
    budget: &mut Budget,
) -> TestResult<(PreparedLatticeDecision<f64>, Handle<Value, f64>)> {
    let frozen = heap.freeze_lattice_inputs(association, inputs, budget)?;
    let original = frozen.operands()[0].input().handle();
    let enclosed = heap.enclose_lattice_inputs(
        frozen,
        &source.fixture.frame.cas,
        lattice_selection::original_box(&source)?,
        &ArithmeticContext::new()?,
        literals(),
        budget,
    )?;
    let selected = lattice_selection::select(source)?;
    Ok((
        heap.prepare_lattice_decision(enclosed, selected, budget)?,
        original,
    ))
}

/// The phase and declared torus select complete tuples under their original live input leases.
/// ´test:integration:lattice-live-costs´
/// ´claim:matrix:lattice-live-costs´
#[test]
fn lattice_live_costs() -> TestResult {
    for (kind, work_limits) in [(0, [65_971, 65_970]), (1, [30_747, 30_746])] {
        for (position, work) in work_limits.into_iter().enumerate() {
            lifecycle(kind, work, position == 0)?;
        }
    }
    Ok(())
}

/// Exercise the complete measured caller ledger, with the final retired read at its boundary.
fn lifecycle(kind: u8, work: u64, accepted: bool) -> TestResult {
    let source = lattice_selection::prepared(kind, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association, kind)?;
    let before = heap.provenance_usage();
    let mut budget = Budget::new(nrz_bindings::Limits {
        work,
        ..nrz_bindings::Limits::default()
    })?;
    budget.work(
        17,
        nrz_bindings::readings::algebra::Location::new("prior lattice lifecycle"),
    )?;
    let (prepared, original) = candidate(&heap, source, &association, &inputs, &mut budget)?;
    let preparation = budget.usage().work;
    let published = heap.publish_lattice_decision(prepared, &mut budget)?;
    let publication = budget.usage().work;
    let representative = published.representative(&heap, 0, &mut budget)?;
    representative.current(&heap, &mut budget)?;
    let index = published.index(&heap, 0, (0, 3), &mut budget)?;
    assert_eq!(
        index.current(&heap, &mut budget)?,
        if kind == 0 { 1 } else { 3 }
    );
    let current = budget.usage().work;
    let value = heap.value(original)?;
    let stamp = value.stamp();
    let center = value.region().get(0, 0)?;
    heap.replace_declared_bounds(original, stamp.version(), &[center], &[1.5])?;
    let Err(fault) = representative.current(&heap, &mut budget) else {
        panic!("changed radius must retire the representative");
    };
    assert!(matches!(fault, LiveDecisionError::Snapshot(_)));
    let Err(fault) = index.current(&heap, &mut budget) else {
        panic!("parent retirement must retire the lowering");
    };
    if accepted {
        assert!(matches!(fault, LiveDecisionError::Retired));
        assert_eq!(budget.usage().work - current, 35);
    } else {
        let LiveDecisionError::Algebra(fault) = fault else {
            panic!("one-below complete lifecycle Work must refuse at the retired read");
        };
        assert!(matches!(
            fault.kind,
            nrz_bindings::readings::algebra::FaultKind::Exhausted { .. }
        ));
        assert_eq!(budget.usage().work - current, 34);
    }
    assert_eq!(budget.usage().work, work);
    assert_eq!(preparation - 17, if kind == 0 { 65_265 } else { 29_721 });
    assert_eq!(publication - preparation, if kind == 0 { 225 } else { 289 });
    assert_eq!(current - publication, if kind == 0 { 429 } else { 685 });
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}

/// Late radius changes and foreign leases cannot publish any part of a selected tuple.
/// ´test:integration:lattice-live-publication-refusals´
/// ´claim:matrix:lattice-live-publication-refusals´
#[test]
fn lattice_live_publication_refusals() -> TestResult {
    for foreign in [false, true] {
        let source = lattice_selection::prepared(1, false)?;
        let association = source.case.associate_inputs(&mut ledger()?)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = inputs(&mut heap, &association, 1)?;
        let (prepared, handle) = candidate(&heap, source, &association, &inputs, &mut ledger()?)?;
        let other = Heap::new(HeapLimits::default())?;
        if !foreign {
            let value = heap.value(handle)?;
            let stamp = value.stamp();
            let center = value.region().get(0, 0)?;
            heap.replace_declared_bounds(handle, stamp.version(), &[center], &[1.5])?;
        }
        let before = heap.provenance_usage();
        let receiver = if foreign { &other } else { &heap };
        let Err(fault) = receiver.publish_lattice_decision(prepared, &mut ledger()?) else {
            panic!("invalid lease must refuse the complete publication");
        };
        let LiveDecisionError::Snapshot(nrz_matrix::DecisionSnapshotError::Heap(reason)) = fault
        else {
            panic!("publication must refuse at the native lease checker");
        };
        assert_eq!(
            reason,
            if foreign {
                nrz_matrix::HeapError::ForeignHandle
            } else {
                nrz_matrix::HeapError::StaleEvidence
            }
        );
        assert_eq!(heap.provenance_usage(), before);
    }
    Ok(())
}

/// Same-cell widening requires fresh validation and cannot revive retired descendants.
/// ´test:integration:lattice-live-fresh-equal-tuple´
/// ´claim:matrix:lattice-live-fresh-equal-tuple´
#[test]
fn lattice_live_fresh_equal_tuple() -> TestResult {
    let source = lattice_selection::prepared(1, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let contract = lattice_selection::original_box(&source)?.clone();
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut inputs = inputs(&mut heap, &association, 1)?;
    let (prepared, handle) = candidate(&heap, source, &association, &inputs, &mut ledger()?)?;
    let published = heap.publish_lattice_decision(prepared, &mut ledger()?)?;
    let selected = published.current(&heap, &mut ledger()?)?.clone();
    let index = published.index(&heap, 0, (0, 3), &mut ledger()?)?;
    let value = heap.value(handle)?;
    let stamp = value.stamp();
    let center = value.region().get(0, 0)?;
    heap.replace_declared_bounds(handle, stamp.version(), &[center], &[513.0 / 1024.0])?;
    let Err(fault) = index.current(&heap, &mut ledger()?) else {
        panic!("same-cell source change must retire the old index");
    };
    assert!(matches!(fault, LiveDecisionError::Snapshot(_)));
    inputs[0] = heap.import_symbols(
        association.operands()[0].symbols(),
        &[center],
        Some(&[513.0 / 1024.0]),
        Some(&[0.0]),
        Coverage::DeclaredBounds,
    )?;
    let frozen = heap.freeze_lattice_inputs(&association, &inputs, &mut ledger()?)?;
    let enclosed = heap.enclose_lattice_inputs(
        frozen,
        selected.cas(),
        &contract,
        &ArithmeticContext::new()?,
        literals(),
        &mut ledger()?,
    )?;
    let prepared = heap.prepare_lattice_decision(enclosed, selected, &mut ledger()?)?;
    let replacement = heap.publish_lattice_decision(prepared, &mut ledger()?)?;
    assert_eq!(
        replacement
            .index(&heap, 0, (0, 3), &mut ledger()?)?
            .current(&heap, &mut ledger()?)?,
        3
    );
    let Err(fault) = published.current(&heap, &mut ledger()?) else {
        panic!("equal fresh tuple must never revive the old authority");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    let Err(fault) = index.current(&heap, &mut ledger()?) else {
        panic!("old lowering must remain retired");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    Ok(())
}

/// Unsupported kernel evidence refuses at the independent original-coordinate checker.
/// ´test:integration:lattice-live-kernel-origins-refuse´
/// ´claim:matrix:lattice-live-kernel-origins-refuse´
#[test]
fn lattice_live_kernel_origins_refuse() -> TestResult {
    use nrz_bindings::readings::algebra::{
        FaultKind, InvalidCode, Location, decision::EnclosureOrigin,
    };
    let source = lattice_selection::prepared(1, false)?;
    let association = source.case.associate_inputs(&mut ledger()?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association, 1)?;
    let records = source.fixture.frame.cas.counts();
    let heap_records = heap.provenance_usage();
    for origin in [EnclosureOrigin::ScalarKernel, EnclosureOrigin::MatrixKernel] {
        let frozen = heap.freeze_lattice_inputs(&association, &inputs, &mut ledger()?)?;
        let mut contract = lattice_selection::original_box(&source)?.clone();
        contract.coordinates[1].origin = origin;
        let Err(fault) = heap.enclose_lattice_inputs(
            frozen,
            &source.fixture.frame.cas,
            &contract,
            &ArithmeticContext::new()?,
            literals(),
            &mut ledger()?,
        ) else {
            panic!("unsupported later coordinate must refuse without a prefix");
        };
        let LiveDecisionError::Algebra(fault) = fault else {
            panic!("kernel origin must reach the independent CAS checker");
        };
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
        );
        assert_eq!(fault.location, Location::new("exact rational enclosure"));
        assert_eq!(source.fixture.frame.cas.counts(), records);
        assert_eq!(heap.provenance_usage(), heap_records);
    }
    Ok(())
}
