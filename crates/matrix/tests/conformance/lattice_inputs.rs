// SPDX-License-Identifier: AGPL-3.0-only
//! Lattice originals reuse the scalar capture and complete native version relation.

use super::{decision_fixture::ledger, lattice_fixture, lattice_physical};
use crate::binding_testing::TestResult;
use nrz_bindings::{LatticeDecisionCase, LatticeInputs};
use nrz_matrix::{Coverage, DecisionSnapshotError, Heap, HeapError, HeapLimits, ValueBinding};

/// Retain a full physical phase, declared two-angle torus or rational-grid question.
pub(super) fn association(kind: u8) -> TestResult<LatticeInputs> {
    let mut fixture = lattice_fixture::lattice(kind)?;
    let owner = lattice_physical::source(kind, "1", "live lattice source")?;
    let spec = lattice_physical::specification(&mut fixture, kind, &owner)?;
    let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?)?;
    Ok(case.associate_inputs(&mut ledger()?)?)
}

/// Numerically import each original against its designated value and companion symbols.
pub(super) fn inputs(
    heap: &mut Heap,
    association: &LatticeInputs,
) -> TestResult<Vec<ValueBinding<f64>>> {
    association
        .operands()
        .iter()
        .map(|operand| {
            let center = match operand.name() {
                "axis_a" => 20.0,
                "axis_b" => 10.0,
                "period0" | "period1" => 6.25,
                "zero" => 0.0,
                _ => return Err("unexpected original input".into()),
            };
            Ok(heap.import_symbols(
                operand.symbols(),
                &[center],
                Some(&[0.5]),
                Some(&[0.125]),
                Coverage::DeclaredBounds,
            )?)
        })
        .collect()
}

/// Measured native capture, current-check and storage owners for one fixture.
struct Costs {
    /// Phase, torus or rational grid.
    kind: u8,
    /// Work for the complete native capture.
    capture: u64,
    /// Work for one current lease validation.
    check: u64,
    /// Newly owned snapshot metadata.
    bytes: u64,
    /// Distinct original source count.
    operands: usize,
}

/// Capture and authentication retain all original sources without publishing heap records.
/// ´test:integration:lattice-inputs-costs´
/// ´claim:matrix:lattice-inputs-costs´
#[test]
fn lattice_inputs_costs() -> TestResult {
    for expected in [
        Costs {
            kind: 0,
            capture: 4107,
            check: 97,
            bytes: 4000,
            operands: 3,
        },
        Costs {
            kind: 1,
            capture: 6417,
            check: 161,
            bytes: 6240,
            operands: 5,
        },
        Costs {
            kind: 2,
            capture: 6417,
            check: 161,
            bytes: 6240,
            operands: 5,
        },
    ] {
        let association = association(expected.kind)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let bindings = inputs(&mut heap, &association)?;
        let before = heap.provenance_usage();
        let mut budget = ledger()?;
        let frozen = heap.freeze_lattice_inputs(&association, &bindings, &mut budget)?;
        let capture = budget.usage().work;
        heap.check_lattice_inputs(&frozen, &mut budget)?;
        assert_eq!(heap.provenance_usage(), before);
        assert_eq!(capture, expected.capture);
        assert_eq!(budget.usage().work - capture, expected.check);
        assert_eq!(frozen.retained_payload_bytes(), expected.bytes);
        assert_eq!(frozen.operands().len(), expected.operands);
        assert_eq!(before.records(), 0);
    }
    Ok(())
}

/// Radius-only writes retire the complete vector snapshot despite unchanged center bytes.
/// ´test:integration:lattice-inputs-radius-lifetime´
/// ´claim:matrix:lattice-inputs-radius-lifetime´
#[test]
fn lattice_inputs_radius_lifetime() -> TestResult {
    let association = association(1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let frozen = heap.freeze_lattice_inputs(&association, &bindings, &mut ledger()?)?;
    let foreign = Heap::new(HeapLimits::default())?;
    let Err(fault) = foreign.check_lattice_inputs(&frozen, &mut ledger()?) else {
        panic!("foreign heap must refuse");
    };
    assert!(matches!(
        fault,
        DecisionSnapshotError::Heap(HeapError::ForeignHandle)
    ));
    heap.check_lattice_inputs(&frozen, &mut ledger()?)?;
    assert_eq!(frozen.operands().len(), 5);
    for (original, captured) in association.operands().iter().zip(frozen.operands()) {
        assert_eq!(
            heap.symbol_binding(captured.input().handle())?
                .symbol()
                .node(),
            original.symbols().value().node()
        );
        assert_eq!(captured.coordinates()[1..], [0.5, 0.125]);
    }
    let operand = &frozen.operands()[0];
    let handle = operand.input().handle();
    let stamp = operand.input().stamp();
    let center = operand.coordinates()[0];
    let alias = heap.alias(handle, operand.input().layout())?;
    let before = heap.provenance_usage();
    heap.replace_declared_bounds(alias, stamp.version(), &[center], &[1.5])?;
    assert_eq!(
        heap.value(handle)?.region().get(0, 0)?.to_bits(),
        center.to_bits()
    );
    let Err(fault) = heap.check_lattice_inputs(&frozen, &mut ledger()?) else {
        panic!("changed total radius must refuse");
    };
    assert!(matches!(
        fault,
        DecisionSnapshotError::Heap(HeapError::StaleEvidence)
    ));
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}
