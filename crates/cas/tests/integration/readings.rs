// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-family reading proofs exported and accepted in independently rebuilt contexts.

use crate::testing::common::TestResult;
use crate::testing::readings::{ComplexChange, ReadingCase, complex_case, ledger, state_case};

fn portable(source: &mut ReadingCase, receiver: &mut ReadingCase) -> TestResult {
    let witness = source.prove(&mut ledger()?)?;
    assert!(witness.is_discharged());
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let before = receiver.fixture.cas.counts();
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        receiver
            .fixture
            .cas
            .export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    assert_ne!(receiver.fixture.cas.counts(), before);
    Ok(())
}

/// Both copies of `a` agree with `Re(c)` only through the retained whole-block expansion.
/// ´test:integration:complex-block-reading-proof-replays´
#[test]
fn complex_block_reading_proof_replays() -> TestResult {
    portable(
        &mut complex_case(ComplexChange::None)?,
        &mut complex_case(ComplexChange::None)?,
    )?;
    portable(
        &mut complex_case(ComplexChange::Copy)?,
        &mut complex_case(ComplexChange::Copy)?,
    )
}

/// Both state components are the exact ordered projections of the same family.
/// ´test:integration:state-component-reading-proofs-replay´
#[test]
fn state_component_reading_proofs_replay() -> TestResult {
    for component in 0..2 {
        portable(&mut state_case(component)?, &mut state_case(component)?)?;
    }
    Ok(())
}

/// Partial overlaps compare only the shared component; the complete readings still disagree.
/// ´test:integration:partial-reading-overlap-projection´
#[test]
fn partial_reading_overlap_projection() -> TestResult {
    let (mut source, whole) = overlap_case()?;
    let refusal = source.fixture.cas.prove_reading_equality(
        &source.fixture.context,
        &whole[0],
        &whole[1],
        &mut ledger()?,
    );
    assert!(matches!(
        refusal,
        Err(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::ReadingMismatch),
            ..
        })
    ));
    let (mut receiver, _) = overlap_case()?;
    portable(&mut source, &mut receiver)
}

fn overlap_case() -> TestResult<(ReadingCase, [nrz_cas::Handle; 2])> {
    use crate::testing::readings::{Fixture, matrix};
    use nrz_cas::readings::{AffineMap, EntryIndex, SelectionSpec};

    let mut fixture = Fixture::new(&[
        ("z", matrix(3, 1)),
        ("x", matrix(2, 1)),
        ("y", matrix(2, 1)),
    ])?;
    let original = fixture.symbol("z")?;
    let mut whole = Vec::new();
    for (key, row) in [("x", 0), ("y", 1)] {
        let selected = fixture.cas.select_entries(
            &original,
            &SelectionSpec::Block {
                row,
                column: 0,
                rows: 2,
                columns: 1,
            },
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol(key)?;
        whole.push(fixture.cas.define_reading(
            &symbol,
            &selected,
            &AffineMap::identity(2, &mut ledger()?)?,
            &mut ledger()?,
        )?);
    }
    let [first, second]: [nrz_cas::Handle; 2] = whole.try_into().map_err(|_| "overlap pair")?;
    let left = fixture.cas.select_entries(
        &first,
        &SelectionSpec::Entry(EntryIndex { row: 1, column: 0 }),
        &mut ledger()?,
    )?;
    let right = fixture.cas.select_entries(
        &second,
        &SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        &mut ledger()?,
    )?;
    Ok((
        ReadingCase {
            fixture,
            left,
            right,
        },
        [first, second],
    ))
}

/// Three independently defined readings agree pairwise through the same exact source family.
/// ´test:integration:three-readings-compose-through-common-family´
#[test]
fn three_readings_compose_through_common_family() -> TestResult {
    for (left, right) in [(0, 1), (1, 2), (0, 2)] {
        let (fixture, values) = triple_case()?;
        let mut source = ReadingCase {
            fixture,
            left: values.get(left).ok_or("left")?.clone(),
            right: values.get(right).ok_or("right")?.clone(),
        };
        let (fixture, values) = triple_case()?;
        let mut receiver = ReadingCase {
            fixture,
            left: values.get(left).ok_or("left")?.clone(),
            right: values.get(right).ok_or("right")?.clone(),
        };
        portable(&mut source, &mut receiver)?;
    }
    Ok(())
}

fn triple_case() -> TestResult<(crate::testing::readings::Fixture, Vec<nrz_cas::Handle>)> {
    use crate::testing::readings::{Fixture, map, matrix};
    use nrz_cas::readings::AffineMap;

    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("y", matrix(2, 1)),
        ("b", matrix(2, 1)),
        ("c", matrix(2, 1)),
    ])?;
    let source = fixture.symbol("z")?;
    let first = fixture.symbol("x")?;
    let transformed = fixture.symbol("y")?;
    let second = fixture.symbol("b")?;
    let third = fixture.symbol("c")?;
    let identity = AffineMap::identity(2, &mut ledger()?)?;
    let first = fixture
        .cas
        .define_reading(&first, &source, &identity, &mut ledger()?)?;
    let changed = fixture.cas.define_reading(
        &transformed,
        &source,
        &map(2, 2, &[1, 1, 0, 1], &[2, 3])?,
        &mut ledger()?,
    )?;
    let second = fixture.cas.define_reading(
        &second,
        &changed,
        &map(2, 2, &[1, -1, 0, 1], &[1, -3])?,
        &mut ledger()?,
    )?;
    let third = fixture
        .cas
        .define_reading(&third, &first, &identity, &mut ledger()?)?;
    Ok((fixture, vec![first, second, third]))
}

/// The block of a conjugate is the transpose on every admitted coordinate assignment.
/// ´test:integration:conjugate-reading-block-transpose´
#[test]
fn conjugate_reading_block_transpose() -> TestResult {
    portable(&mut conjugate_case()?, &mut conjugate_case()?)
}

fn conjugate_case() -> TestResult<ReadingCase> {
    use crate::testing::readings::{Fixture, map, matrix};
    use nrz_cas::readings::AffineMap;
    use nrz_cas::syntax::{AtomicSort, Field, Operator, Sort};

    let complex = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("c", complex.clone()),
        ("block", matrix(2, 2)),
        ("conjugated", matrix(2, 2)),
    ])?;
    let source = fixture.symbol("z")?;
    let symbol = fixture.symbol("c")?;
    let value = fixture.cas.define_reading(
        &symbol,
        &source,
        &AffineMap::identity(2, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let conjugate = fixture.term(Operator::Conjugate, complex, std::slice::from_ref(&value))?;
    let expansion = map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?;
    let symbol = fixture.symbol("block")?;
    let block = fixture
        .cas
        .define_reading(&symbol, &value, &expansion, &mut ledger()?)?;
    let symbol = fixture.symbol("conjugated")?;
    let left = fixture
        .cas
        .define_reading(&symbol, &conjugate, &expansion, &mut ledger()?)?;
    let right = fixture.term(Operator::Transpose, matrix(2, 2), &[block])?;
    Ok(ReadingCase {
        fixture,
        left,
        right,
    })
}

/// Ordinary ordered congruence lifts a checked component equation through another reading.
/// ´test:integration:reading-equality-ordered-congruence´
#[test]
fn reading_equality_ordered_congruence() -> TestResult {
    let (mut source, steps) = congruence_case()?;
    let witness =
        source
            .fixture
            .cas
            .check_proof(&source.fixture.context, &steps, &mut ledger()?)?;
    assert!(witness.is_discharged());
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let (mut receiver, _) = congruence_case()?;
    let expected = receiver.expected()?;
    let replayed = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(
        receiver
            .fixture
            .cas
            .export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    Ok(())
}

fn congruence_case() -> TestResult<(ReadingCase, Vec<nrz_cas::proof::ApplicationDraft>)> {
    use crate::testing::readings::map;
    use crate::testing::witness::equality;
    use nrz_cas::proof::{Certificate, Primitive};

    let mut case = state_case(0)?;
    let domain = case.fixture.cas.admissible_region(&case.fixture.context)?;
    let first = equality(
        &case.fixture.context,
        &domain,
        Primitive::Jets,
        &case.left,
        &case.right,
    );
    let symbol = case.fixture.symbol("i")?;
    let affine = map(1, 1, &[2], &[3])?;
    case.left = case
        .fixture
        .cas
        .define_reading(&symbol, &case.left, &affine, &mut ledger()?)?;
    case.right = case
        .fixture
        .cas
        .define_reading(&symbol, &case.right, &affine, &mut ledger()?)?;
    let mut second = equality(
        &case.fixture.context,
        &domain,
        Primitive::Congruence,
        &case.left,
        &case.right,
    );
    second.premises = vec![0];
    second.certificate = Certificate::Congruence(vec![1]);
    Ok((case, vec![first, second]))
}
