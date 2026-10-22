// SPDX-License-Identifier: AGPL-3.0-only
//! Finite selection and ordered affine-map contracts through the public API.

use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::readings::{AffineMap, EntryIndex, EntryOrder, EntrySelection, SelectionSpec};
use nrz_cas::syntax::{AtomicSort, Field, Operator, Sort};
use nrz_cas::{Fault, FaultKind, InvalidCode, Rational};

use crate::testing::common::{TestResult, real};
use crate::testing::readings::{Fixture, ledger, map, matrix, numbers};

fn refuses<T>(result: Result<T, Fault>, expected: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(expected))
    );
}

/// Convenience selectors retain logical column order and their exact result shape.
/// ´test:integration:finite-selection-shapes-and-order´
#[test]
fn finite_selection_shapes_and_order() -> TestResult {
    let source = matrix(3, 2);
    let block = EntrySelection::new(
        &source,
        &SelectionSpec::Block {
            row: 1,
            column: 0,
            rows: 2,
            columns: 2,
        },
        &mut ledger()?,
    )?;
    assert_eq!(
        block.indices(),
        &[
            EntryIndex { row: 1, column: 0 },
            EntryIndex { row: 2, column: 0 },
            EntryIndex { row: 1, column: 1 },
            EntryIndex { row: 2, column: 1 },
        ]
    );
    assert_eq!(block.result_sort(&source, &mut ledger()?)?, matrix(2, 2));
    let row = EntrySelection::new(&source, &SelectionSpec::Row(1), &mut ledger()?)?;
    assert_eq!(row.result_sort(&source, &mut ledger()?)?, matrix(1, 2));
    let column = EntrySelection::new(&source, &SelectionSpec::Column(1), &mut ledger()?)?;
    assert_eq!(column.result_sort(&source, &mut ledger()?)?, matrix(3, 1));
    let entry = EntrySelection::new(
        &source,
        &SelectionSpec::Entry(EntryIndex { row: 2, column: 1 }),
        &mut ledger()?,
    )?;
    assert_eq!(
        entry.result_sort(&source, &mut ledger()?)?,
        Sort::Value(AtomicSort::Scalar(Field::Real))
    );
    let empty = EntrySelection::new(
        &source,
        &SelectionSpec::Entries {
            entries: Vec::new(),
            order: EntryOrder::Given,
        },
        &mut ledger()?,
    )?;
    assert_eq!(empty.indices(), [] as [EntryIndex; 0]);
    Ok(())
}

/// Shape, range, injectivity and ordering failures remain separately named.
/// ´test:integration:finite-selection-named-refusals´
#[test]
fn finite_selection_named_refusals() -> TestResult {
    let source = matrix(2, 2);
    let first = EntryIndex { row: 0, column: 0 };
    let last = EntryIndex { row: 1, column: 1 };
    for (spec, code) in [
        (SelectionSpec::Row(2), InvalidCode::SelectionOutOfRange),
        (SelectionSpec::Column(2), InvalidCode::SelectionOutOfRange),
        (
            SelectionSpec::Entries {
                entries: vec![first, first],
                order: EntryOrder::Given,
            },
            InvalidCode::SelectionDuplicate,
        ),
        (
            SelectionSpec::Entries {
                entries: vec![last, first],
                order: EntryOrder::ColumnMajor,
            },
            InvalidCode::SelectionOrder,
        ),
        (
            SelectionSpec::Block {
                row: u32::MAX,
                column: 0,
                rows: 2,
                columns: 1,
            },
            InvalidCode::SelectionShape,
        ),
    ] {
        refuses(EntrySelection::new(&source, &spec, &mut ledger()?), code);
    }
    let scalar = Sort::Value(AtomicSort::Scalar(Field::Real));
    refuses(
        EntrySelection::new(&scalar, &SelectionSpec::Entry(first), &mut ledger()?),
        InvalidCode::SelectionShape,
    );
    let selection = EntrySelection::new(&source, &SelectionSpec::Entry(first), &mut ledger()?)?;
    refuses(
        selection.result_sort(&matrix(3, 2), &mut ledger()?),
        InvalidCode::SelectionShape,
    );
    Ok(())
}

/// Matrix contraction is noncommutative and includes both ordered translation terms.
/// ´test:integration:affine-map-ordered-composition´
#[test]
fn affine_map_ordered_composition() -> TestResult {
    let outer = map(2, 2, &[1, 2, 0, 1], &[3, 4])?;
    let inner = map(2, 2, &[1, 0, 3, 1], &[5, 6])?;
    let forward = outer.compose(&inner, &mut ledger()?)?;
    assert_eq!(forward, map(2, 2, &[7, 2, 3, 1], &[20, 10])?);
    assert_ne!(forward, inner.compose(&outer, &mut ledger()?)?);
    assert_eq!(
        forward.project(&[1, 0], &mut ledger()?)?,
        map(2, 2, &[3, 1, 7, 2], &[10, 20])?
    );
    let expansion = map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?;
    assert_eq!(
        expansion.project(&[0, 3], &mut ledger()?)?,
        map(2, 2, &[1, 0, 1, 0], &[0, 0])?
    );
    refuses(
        outer.compose(&expansion, &mut ledger()?),
        InvalidCode::ReadingShape,
    );
    refuses(
        outer.project(&[2], &mut ledger()?),
        InvalidCode::SelectionOutOfRange,
    );
    refuses(
        AffineMap::new(2, 2, numbers(&[1])?, numbers(&[0, 0])?, &mut ledger()?),
        InvalidCode::ReadingShape,
    );
    Ok(())
}

/// Each new value operation has an exact cumulative Work boundary.
/// ´test:integration:reading-value-work-boundaries´
#[test]
fn reading_value_work_boundaries() -> TestResult {
    let input = map(2, 2, &[1, 2, 3, 4], &[5, 6])?;
    for operation in 0..6 {
        let run = |budget: &mut Budget| -> Result<(), Fault> {
            match operation {
                0 => {
                    AffineMap::identity(2, budget)?;
                }
                1 => {
                    input.compose(&input, budget)?;
                }
                2 => {
                    input.project(&[1, 0], budget)?;
                }
                3 => {
                    EntrySelection::new(&matrix(2, 2), &SelectionSpec::Row(0), budget)?;
                }
                4 => {
                    let selection =
                        EntrySelection::new(&matrix(2, 1), &SelectionSpec::Row(0), &mut ledger()?)?;
                    selection.compose(&input, budget)?;
                }
                _ => {
                    AffineMap::new(1, 1, vec![Rational::one()], vec![Rational::zero()], budget)?;
                }
            }
            Ok(())
        };
        let mut measured = ledger()?;
        run(&mut measured)?;
        let needed = measured.usage().work;
        for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
            let result = run(&mut Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(matches!(
                    result,
                    Err(Fault {
                        kind: FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            ..
                        },
                        ..
                    })
                ));
            }
        }
    }
    Ok(())
}

/// Graph definitions retain the selected copy and refuse changed source extents atomically.
/// ´test:integration:reading-syntax-retains-selected-identity´
#[test]
fn reading_syntax_retains_selected_identity() -> TestResult {
    let mut fixture = Fixture::new(&[("block", matrix(2, 2)), ("r", real())])?;
    let source = fixture.symbol("block")?;
    let result = fixture.symbol("r")?;
    let first = fixture.cas.select_entries(
        &source,
        &SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        &mut ledger()?,
    )?;
    let repeated = fixture.cas.select_entries(
        &source,
        &SelectionSpec::Entry(EntryIndex { row: 1, column: 1 }),
        &mut ledger()?,
    )?;
    assert_ne!(first, repeated);
    let identity = AffineMap::identity(1, &mut ledger()?)?;
    let reading = fixture
        .cas
        .define_reading(&result, &first, &identity, &mut ledger()?)?;
    assert_ne!(reading, result);
    let before = fixture.cas.counts();
    refuses(
        fixture
            .cas
            .define_reading(&result, &source, &identity, &mut ledger()?),
        InvalidCode::ReadingShape,
    );
    assert_eq!(fixture.cas.counts(), before);
    let zero = fixture.term(Operator::Rational(Rational::zero()), real(), &[])?;
    refuses(
        fixture
            .cas
            .define_reading(&zero, &first, &identity, &mut ledger()?),
        InvalidCode::ReadingDefinition,
    );
    Ok(())
}

/// Both graph constructors retain spent Work and publish nothing below their exact limit.
/// ´test:integration:reading-admission-work-and-payload-boundaries´
#[test]
fn reading_admission_work_and_payload_boundaries() -> TestResult {
    for definition in [false, true] {
        let setup = || Fixture::new(&[("source", matrix(2, 1)), ("result", matrix(2, 1))]);
        let operation = |fixture: &mut Fixture, budget: &mut Budget| -> TestResult {
            let source = fixture.symbol("source")?;
            if definition {
                let result = fixture.symbol("result")?;
                fixture.cas.define_reading(
                    &result,
                    &source,
                    &AffineMap::identity(2, &mut ledger()?)?,
                    budget,
                )?;
            } else {
                fixture
                    .cas
                    .select_entries(&source, &SelectionSpec::Column(0), budget)?;
            }
            Ok(())
        };
        let mut measured_fixture = setup()?;
        let mut measured = ledger()?;
        operation(&mut measured_fixture, &mut measured)?;
        for (kind, needed) in [
            (BudgetKind::Work, measured.usage().work),
            (
                BudgetKind::PayloadBytes,
                measured_fixture.cas.counts().payload_bytes,
            ),
        ] {
            for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
                let mut fixture = setup()?;
                let before = fixture.cas.counts();
                let limits = if kind == BudgetKind::Work {
                    Limits {
                        work: limit,
                        ..Limits::default()
                    }
                } else {
                    Limits {
                        payload_bytes: limit,
                        ..Limits::default()
                    }
                };
                let result = operation(&mut fixture, &mut Budget::new(limits)?);
                assert_eq!(result.is_ok(), succeeds);
                if !succeeds {
                    assert_eq!(fixture.cas.counts(), before);
                }
            }
        }
    }
    Ok(())
}

/// Both operator payloads round-trip with their full source and declaration closure.
/// ´test:integration:reading-syntax-portable-closure´
#[test]
fn reading_syntax_portable_closure() -> TestResult {
    use crate::testing::witness::equality;
    use nrz_cas::domain::Layer;
    use nrz_cas::proof::{Judgment, Primitive};

    let setup = || -> TestResult<(Fixture, nrz_cas::Handle)> {
        let mut fixture = Fixture::new(&[("source", matrix(2, 1)), ("r", real())])?;
        let source = fixture.symbol("source")?;
        let symbol = fixture.symbol("r")?;
        let selected = fixture.cas.select_entries(
            &source,
            &SelectionSpec::Entry(EntryIndex { row: 1, column: 0 }),
            &mut ledger()?,
        )?;
        let reading = fixture.cas.define_reading(
            &symbol,
            &selected,
            &map(1, 1, &[-2], &[3])?,
            &mut ledger()?,
        )?;
        Ok((fixture, reading))
    };
    let (mut source, original) = setup()?;
    let domain = source.cas.admissible_region(&source.context)?;
    let step = equality(
        &source.context,
        &domain,
        Primitive::Reflexivity,
        &original,
        &original,
    );
    let witness = source
        .cas
        .check_proof(&source.context, &[step], &mut ledger()?)?;
    assert!(witness.is_discharged());
    let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
    let (mut receiver, reading) = setup()?;
    let domain = receiver.cas.admissible_region(&receiver.context)?;
    let expected = Judgment {
        context: receiver.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: equality(
            &receiver.context,
            &domain,
            Primitive::Reflexivity,
            &reading,
            &reading,
        )
        .conclusion,
        residuals: Vec::new(),
    };
    let replayed = receiver.cas.replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    Ok(())
}

/// The smallest accepted temporary-payload ceiling succeeds and one byte less refuses.
/// ´test:integration:reading-value-payload-boundaries´
#[test]
fn reading_value_payload_boundaries() -> TestResult {
    let input = map(2, 2, &[1, 2, 3, 4], &[5, 6])?;
    let selected = EntrySelection::new(&matrix(2, 1), &SelectionSpec::Row(0), &mut ledger()?)?;
    for operation in 0..6 {
        let run = |limit| -> Result<(), Fault> {
            let mut budget = Budget::new(Limits {
                staged_payload_bytes: limit,
                ..Limits::default()
            })?;
            match operation {
                0 => {
                    AffineMap::identity(2, &mut budget)?;
                }
                1 => {
                    input.compose(&input, &mut budget)?;
                }
                2 => {
                    input.project(&[1, 0], &mut budget)?;
                }
                3 => {
                    EntrySelection::new(&matrix(2, 2), &SelectionSpec::Row(0), &mut budget)?;
                }
                4 => {
                    selected.compose(&input, &mut budget)?;
                }
                _ => {
                    AffineMap::new(
                        1,
                        1,
                        vec![Rational::one()],
                        vec![Rational::zero()],
                        &mut budget,
                    )?;
                }
            }
            Ok(())
        };
        let mut low = 0;
        let mut high = Limits::default().staged_payload_bytes;
        run(high)?;
        while low < high {
            let middle = low + (high - low) / 2;
            if run(middle).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        run(high)?;
        assert!(matches!(
            run(high - 1),
            Err(Fault {
                kind: FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    ..
                },
                ..
            })
        ));
    }
    Ok(())
}

/// The selected sort's declared field is measured before copying its owned name.
/// ´test:integration:selection-result-sort-work-boundary´
#[test]
fn selection_result_sort_work_boundary() -> TestResult {
    let source = Sort::Value(AtomicSort::Matrix {
        field: Field::Named("declared-entry-field".into()),
        rows: nrz_cas::syntax::Extent::Literal(2),
        columns: nrz_cas::syntax::Extent::Literal(1),
    });
    let selection = EntrySelection::new(&source, &SelectionSpec::Row(0), &mut ledger()?)?;
    let mut measured = ledger()?;
    selection.result_sort(&source, &mut measured)?;
    let needed = measured.usage().work;
    for (limit, succeeds) in [(needed, true), (needed - 1, false)] {
        let result = selection.result_sort(
            &source,
            &mut Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?,
        );
        assert_eq!(result.is_ok(), succeeds);
    }
    Ok(())
}

/// The selected sort copy respects its exact temporary-payload boundary.
/// ´test:integration:selection-result-sort-payload-boundary´
#[test]
fn selection_result_sort_payload_boundary() -> TestResult {
    let source = matrix(2, 1);
    let selection = EntrySelection::new(&source, &SelectionSpec::Column(0), &mut ledger()?)?;
    let run = |limit| {
        selection.result_sort(
            &source,
            &mut Budget::new(Limits {
                staged_payload_bytes: limit,
                ..Limits::default()
            })?,
        )
    };
    let mut low = 0;
    let mut high = Limits::default().staged_payload_bytes;
    while low < high {
        let middle = low + (high - low) / 2;
        if run(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    run(high)?;
    assert!(matches!(
        run(high - 1),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::StagedPayloadBytes,
                ..
            },
            ..
        })
    ));
    Ok(())
}
