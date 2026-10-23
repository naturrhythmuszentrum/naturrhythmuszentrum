// SPDX-License-Identifier: AGPL-3.0-only
//! Every budgeted physical adapter shares cumulative Work and explicit payload ceilings.

use crate::testing::readings::{TestResult, ledger, map, state_case};
use nrz_bindings::readings::{
    CoordinateFamily, EntryMeaning, PhysicalReading, ReadingAxis, ReadingDefinition,
    ReadingRelation, ReadingShape,
};
use nrz_bindings::{Budget, Limits};
use nrz_cas::readings::{AffineMap, EntryIndex, SelectionSpec};

/// Independent operation whose setup uses unchanged production limits.
#[derive(Clone, Copy, Debug)]
enum Operation {
    /// Exact per-entry reference and origin checks.
    Meaning,
    /// Complete original coordinate family admission.
    Family,
    /// Complete affine expansion and physical checks.
    Expansion,
    /// Defined physical reading and selected source.
    Reading,
    /// Exact ordered component projection.
    Projection,
    /// Whole-family physical equality proof.
    Proof,
    /// Complete portable proof export.
    Export,
    /// Independently reconstructed expected-judgment replay.
    Replay,
    /// Ordered index and partition admission.
    Axis,
    /// Complete matrix block coverage.
    Shape,
}

/// Operations covered by both resource-boundary tests.
const OPERATIONS: [Operation; 10] = [
    Operation::Meaning,
    Operation::Family,
    Operation::Expansion,
    Operation::Reading,
    Operation::Projection,
    Operation::Proof,
    Operation::Export,
    Operation::Replay,
    Operation::Axis,
    Operation::Shape,
];

/// Spend only the tested operation's work on the constrained cumulative ledger.
fn attempt(operation: Operation, limits: Limits, precharge: u64) -> TestResult<u64> {
    let mut case = state_case(0)?;
    let mut budget = Budget::new(limits)?;
    budget.work(
        precharge,
        nrz_cas::Location::new("physical reading boundary precharge"),
    )?;
    match operation {
        Operation::Meaning => {
            let entry = &case.component.entries()[0];
            EntryMeaning::new(
                entry.descriptor().clone(),
                entry.reference().clone(),
                None,
                &mut budget,
            )?;
        }
        Operation::Family => {
            CoordinateFamily::new(
                &case.fixture.cas,
                &case.fixture.context,
                case.whole.family().root(),
                case.whole.family().entries().to_vec(),
                &mut budget,
            )?;
        }
        Operation::Expansion => {
            let symbol = case.fixture.symbol("x")?;
            case.whole.family().expand(
                &mut case.fixture.cas,
                &symbol,
                &AffineMap::identity(2, &mut ledger()?)?,
                case.whole.entries().to_vec(),
                &mut budget,
            )?;
        }
        Operation::Reading => {
            let definition = ReadingDefinition {
                symbol: case.fixture.symbol("i")?,
                selection: SelectionSpec::Entry(EntryIndex { row: 1, column: 0 }),
                map: map(1, 1, &[2], &[0])?,
                entries: vec![case.whole.entries()[1].clone()],
            };
            PhysicalReading::define(
                &mut case.fixture.cas,
                case.whole.family(),
                definition,
                &mut budget,
            )?;
        }
        Operation::Projection => {
            case.whole
                .project(&mut case.fixture.cas, &[1], &mut budget)?;
        }
        Operation::Proof | Operation::Export | Operation::Replay => {
            relation_attempt(operation, &mut case, &mut budget)?;
        }
        Operation::Axis | Operation::Shape => {
            shape_attempt(operation, &case, &mut budget)?;
        }
    }
    Ok(budget.usage().work)
}

/// Separate setup evidence from the measured proof, export or fresh replay.
fn relation_attempt(
    operation: Operation,
    case: &mut crate::testing::readings::Case,
    budget: &mut Budget,
) -> TestResult {
    let left = case
        .component
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = case
        .whole
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    if matches!(operation, Operation::Proof) {
        ReadingRelation::prove(&mut case.fixture.cas, left, right, budget)?;
        return Ok(());
    }
    let relation = ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?)?;
    if matches!(operation, Operation::Export) {
        relation.export(&case.fixture.cas, budget)?;
    } else {
        let bytes = relation.export(&case.fixture.cas, &mut ledger()?)?;
        let mut receiver = state_case(0)?;
        let left = receiver
            .component
            .project(&mut receiver.fixture.cas, &[0], &mut ledger()?)?;
        let right = receiver
            .whole
            .project(&mut receiver.fixture.cas, &[0], &mut ledger()?)?;
        ReadingRelation::replay(&mut receiver.fixture.cas, left, right, &bytes, budget)?;
    }
    Ok(())
}

/// Axis and block construction keep their own complete finite metadata checks.
fn shape_attempt(
    operation: Operation,
    case: &crate::testing::readings::Case,
    budget: &mut Budget,
) -> TestResult {
    let rows = case.fixture.symbol("rows")?;
    if matches!(operation, Operation::Axis) {
        ReadingAxis::new(
            &case.fixture.cas,
            &case.fixture.context,
            &rows,
            vec![0, 1],
            vec![1, 2],
            budget,
        )?;
    } else {
        let axes = [
            ReadingAxis::new(
                &case.fixture.cas,
                &case.fixture.context,
                &rows,
                vec![0, 1],
                vec![1, 2],
                &mut ledger()?,
            )?,
            ReadingAxis::new(
                &case.fixture.cas,
                &case.fixture.context,
                &case.fixture.symbol("columns")?,
                vec![0],
                vec![1],
                &mut ledger()?,
            )?,
        ];
        ReadingShape::new(
            &case.whole,
            axes,
            &[SelectionSpec::Block {
                row: 0,
                column: 0,
                rows: 2,
                columns: 1,
            }],
            budget,
        )?;
    }
    Ok(())
}

/// Exact N succeeds and N minus one refuses, including previously spent caller work.
/// ´test:integration:physical-reading-work-boundaries´
#[test]
fn physical_reading_work_boundaries() -> TestResult {
    for operation in OPERATIONS {
        let needed = attempt(operation, Limits::default(), 0)?;
        assert!(needed > 0);
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            assert_eq!(
                attempt(
                    operation,
                    Limits {
                        work: limit,
                        ..Limits::default()
                    },
                    0
                )
                .is_ok(),
                succeeds
            );
        }
        assert!(
            attempt(
                operation,
                Limits {
                    work: needed,
                    ..Limits::default()
                },
                1
            )
            .is_err()
        );
    }
    Ok(())
}

/// The minimum admitted staged payload succeeds exactly and refuses one byte below it.
/// ´test:integration:physical-reading-payload-boundaries´
#[test]
fn physical_reading_payload_boundaries() -> TestResult {
    for operation in OPERATIONS {
        if matches!(operation, Operation::Export) {
            continue;
        }
        attempt(operation, Limits::default(), 0)?;
        assert!(
            attempt(
                operation,
                Limits {
                    staged_payload_bytes: 0,
                    ..Limits::default()
                },
                0
            )
            .is_err(),
            "{operation:?}"
        );
        let mut lower = 0;
        let mut upper = Limits::default().staged_payload_bytes;
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            let limits = Limits {
                staged_payload_bytes: middle,
                ..Limits::default()
            };
            if attempt(operation, limits, 0).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        assert!(
            attempt(
                operation,
                Limits {
                    staged_payload_bytes: upper,
                    ..Limits::default()
                },
                0
            )
            .is_ok()
        );
        assert!(
            attempt(
                operation,
                Limits {
                    staged_payload_bytes: upper - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
    }
    Ok(())
}

/// Export copies the already sealed proof bytes and is bounded by their actual wire length.
/// ´test:integration:physical-reading-export-witness-boundaries´
#[test]
fn physical_reading_export_witness_boundaries() -> TestResult {
    let mut case = state_case(0)?;
    let left = case
        .component
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let right = case
        .whole
        .project(&mut case.fixture.cas, &[0], &mut ledger()?)?;
    let relation = ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?)?;
    let needed = u64::try_from(relation.export(&case.fixture.cas, &mut ledger()?)?.len())?;
    for kind in [
        nrz_cas::limits::BudgetKind::Work,
        nrz_cas::limits::BudgetKind::StagedPayloadBytes,
        nrz_cas::limits::BudgetKind::WitnessBytes,
    ] {
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let mut limits = Limits::default();
            match kind {
                nrz_cas::limits::BudgetKind::Work => limits.work = limit,
                nrz_cas::limits::BudgetKind::StagedPayloadBytes => {
                    limits.staged_payload_bytes = limit;
                }
                _ => limits.witness_bytes = limit,
            }
            let mut budget = Budget::new(limits)?;
            assert_eq!(
                relation.export(&case.fixture.cas, &mut budget).is_ok(),
                succeeds
            );
            assert_eq!(budget.usage().work, if succeeds { needed } else { 0 });
        }
    }
    assert_eq!(needed, 1_144);
    Ok(())
}
