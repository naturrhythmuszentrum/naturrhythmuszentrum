// SPDX-License-Identifier: AGPL-3.0-only
//! Separate live lattice owners spend on one caller ledger without graph publication.

use super::{decision_fixture::ledger, lattice_live, lattice_selection};
use crate::binding_testing::TestResult;
use nrz_matrix::{Heap, HeapLimits};
use nrz_scalar::ArithmeticContext;

/// Measured figures identify each receiving owner and its retained storage.
#[derive(Debug, Eq, PartialEq)]
struct Costs {
    /// Phase or two-angle original question.
    kind: u8,
    /// Complete original native capture.
    capture: u64,
    /// CAS original-box replay and live coordinate containment.
    enclosure: u64,
    /// Bind the checked box to the immutable complete selection.
    preparation: u64,
    /// Final native lease validation and shared authority allocation.
    publication: u64,
    /// Borrow under a current native lease.
    current: u64,
    /// Authenticate the requested physical association and native lease.
    current_for: u64,
    /// Construct one dependent physical representative.
    representative: u64,
    /// Construct one separately capacity-checked executable index.
    index: u64,
    /// Original capture storage.
    capture_bytes: u64,
    /// Complete checked enclosure storage.
    enclosure_bytes: u64,
    /// Complete live attachment storage.
    publication_bytes: u64,
}

/// Measure each live owner independently of the existing mathematical prerequisite producers.
/// ´test:integration:lattice-owner-costs´
/// ´claim:matrix:lattice-owner-costs´
#[test]
fn lattice_owner_costs() -> TestResult {
    let mut measurements = Vec::new();
    for kind in [0, 1] {
        let mut source = lattice_selection::prepared(kind, false)?;
        let association = source.case.associate_inputs(&mut ledger()?)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = lattice_live::inputs(&mut heap, &association, kind)?;
        let before = heap.provenance_usage();
        let mut budget = ledger()?;
        let frozen = heap.freeze_lattice_inputs(&association, &inputs, &mut budget)?;
        let capture = budget.usage().work;
        let capture_bytes = frozen.retained_payload_bytes();
        assert!(frozen.operands()[0].coordinates()[2] > 0.0);
        let contract = lattice_selection::original_box(&source)?.clone();
        let enclosed = heap.enclose_lattice_inputs(
            frozen,
            &mut source.fixture.frame.cas,
            &contract,
            &ArithmeticContext::new()?,
            lattice_live::literals(),
            &mut budget,
        )?;
        let enclosure = budget.usage().work;
        let enclosure_bytes = enclosed.retained_payload_bytes();
        let selected = lattice_selection::select(source)?;
        let publication_bytes = enclosure_bytes + selected.retained_payload_bytes() + 128;
        let graph = selected.cas()?.counts();
        let archived = selected.clone();
        let prepared = heap.prepare_lattice_decision(enclosed, selected, &mut budget)?;
        let preparation = budget.usage().work;
        let published = heap.publish_lattice_decision(prepared, &mut budget)?;
        let publication = budget.usage().work;
        published.current(&heap, &mut budget)?;
        let current = budget.usage().work;
        published.current_for(&heap, &association, &mut budget)?;
        let current_for = budget.usage().work;
        published.representative(&heap, 0, &mut budget)?;
        let representative = budget.usage().work;
        published.index(&heap, 0, (0, 3), &mut budget)?;
        measurements.push(Costs {
            kind,
            capture,
            enclosure: enclosure - capture,
            preparation: preparation - enclosure,
            publication: publication - preparation,
            current: current - publication,
            current_for: current_for - current,
            representative: representative - current_for,
            index: budget.usage().work - representative,
            capture_bytes,
            enclosure_bytes,
            publication_bytes,
        });
        assert_eq!(archived.cas()?.counts(), graph);
        assert_eq!(heap.provenance_usage(), before);
    }
    assert_eq!(measurements, expected());
    Ok(())
}

/// Literal measured owners from the intentionally failing baseline probes.
const fn expected() -> [Costs; 2] {
    [
        Costs {
            kind: 0,
            capture: 4107,
            enclosure: 59_464,
            preparation: 1694,
            publication: 225,
            current: 98,
            current_for: 112,
            representative: 98,
            index: 135,
            capture_bytes: 4000,
            enclosure_bytes: 5848,
            publication_bytes: 41_628,
        },
        Costs {
            kind: 1,
            capture: 6417,
            enclosure: 21_282,
            preparation: 2022,
            publication: 289,
            current: 162,
            current_for: 184,
            representative: 162,
            index: 199,
            capture_bytes: 6240,
            enclosure_bytes: 8480,
            publication_bytes: 65_790,
        },
    ]
}
