// SPDX-License-Identifier: AGPL-3.0-only
//! Reading proofs refuse different families, altered premises and point-only structure.

use crate::testing::builders::admit;
use crate::testing::common::{TestResult, real};
use crate::testing::readings::{
    ComplexChange, Fixture, ReadingCase, complex_case, ledger, map, state_case,
};
use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::proof::Primitive;
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Fault, FaultKind, InvalidCode, Rational, Reference};

fn refuses<T>(result: Result<T, Fault>, expected: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(expected))
    );
}

/// Contradictory signs/order and absent repeated-coordinate constraints cannot prove equality.
/// ´test:integration:reading-proof-named-refusals´
#[test]
fn reading_proof_named_refusals() -> TestResult {
    for (change, code) in [
        (ComplexChange::Sign, InvalidCode::ReadingMismatch),
        (ComplexChange::Order, InvalidCode::ReadingMismatch),
        (ComplexChange::Unstructured, InvalidCode::ReadingMismatch),
        (
            ComplexChange::MissingDefinition,
            InvalidCode::ReadingUnrelated,
        ),
    ] {
        let mut case = complex_case(change)?;
        let before = case.fixture.cas.counts();
        refuses(case.prove(&mut ledger()?), code);
        assert_eq!(case.fixture.cas.counts(), before);
    }
    let mut case = complex_case(ComplexChange::None)?;
    case.left = case.fixture.symbol("r")?;
    refuses(case.prove(&mut ledger()?), InvalidCode::ReadingUnrelated);
    Ok(())
}

/// Every original copy, coefficient, order, extent, declaration and definition is replay identity.
/// ´test:integration:reading-replay-binds-exact-premises´
#[test]
fn reading_replay_binds_exact_premises() -> TestResult {
    let mut source = complex_case(ComplexChange::None)?;
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    for change in [
        ComplexChange::Copy,
        ComplexChange::Sign,
        ComplexChange::Order,
        ComplexChange::Extent,
        ComplexChange::Declaration,
        ComplexChange::Unstructured,
        ComplexChange::MissingDefinition,
    ] {
        let mut receiver = complex_case(change)?;
        let expected = receiver.expected()?;
        let before = receiver.fixture.cas.counts();
        refuses(
            receiver
                .fixture
                .cas
                .replay(&bytes, &expected, &mut ledger()?),
            InvalidCode::EndpointMismatch,
        );
        assert_eq!(receiver.fixture.cas.counts(), before);
    }
    Ok(())
}

/// A caller's changed domain cannot accept the original whole-family claim.
/// ´test:integration:reading-replay-binds-domain´
#[test]
fn reading_replay_binds_domain() -> TestResult {
    let mut source = state_case(0)?;
    let witness = source.prove(&mut ledger()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut receiver = state_case(0)?;
    restrict_domain(&mut receiver)?;
    let expected = receiver.expected()?;
    let before = receiver.fixture.cas.counts();
    refuses(
        receiver
            .fixture
            .cas
            .replay(&bytes, &expected, &mut ledger()?),
        InvalidCode::EndpointMismatch,
    );
    assert_eq!(receiver.fixture.cas.counts(), before);
    Ok(())
}

fn restrict_domain(case: &mut ReadingCase) -> TestResult {
    let fixture = &mut case.fixture;
    let header = fixture.cas.context_header(&fixture.context)?;
    let base = fixture.cas.admissible_region(&fixture.context)?;
    let condition = admit(
        &mut fixture.cas,
        Draft::Condition {
            kind: PredicateKind::NonNegative,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(case.left.clone())],
        },
        &mut ledger()?,
    )?;
    let region = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(base),
                predicate: Reference::Existing(condition),
            },
        }),
        &mut ledger()?,
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(region),
            assumptions: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

/// A zero map cannot erase its invalid original divisor or manufacture a source family.
/// ´test:integration:reading-zero-retains-invalid-originals´
#[test]
fn reading_zero_retains_invalid_originals() -> TestResult {
    let mut fixture = Fixture::new(&[("z", real()), ("r", real())])?;
    let source = fixture.symbol("z")?;
    let symbol = fixture.symbol("r")?;
    let zero = fixture.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let quotient = fixture.term(Operator::Div, real(), &[source.clone(), zero])?;
    let null = map(1, 1, &[0], &[0])?;
    let invalid = fixture
        .cas
        .define_reading(&symbol, &quotient, &null, &mut ledger()?)?;
    let valid = fixture
        .cas
        .define_reading(&symbol, &source, &null, &mut ledger()?)?;
    let before = fixture.cas.counts();
    refuses(
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &invalid, &valid, &mut ledger()?),
        InvalidCode::ReadingDefinition,
    );
    assert_eq!(fixture.cas.counts(), before);
    let domain = fixture.cas.admissible_region(&fixture.context)?;
    let step = crate::testing::witness::equality(
        &fixture.context,
        &domain,
        Primitive::Reflexivity,
        &invalid,
        &invalid,
    );
    refuses(
        fixture
            .cas
            .check_proof(&fixture.context, &[step], &mut ledger()?),
        InvalidCode::RefutedDomain,
    );
    Ok(())
}

/// Equal offsets at the center say nothing about opposite coefficients on the family.
/// ´test:integration:reading-center-only-equality-refuses´
#[test]
fn reading_center_only_equality_refuses() -> TestResult {
    let mut fixture = Fixture::new(&[("z", real()), ("r", real()), ("s", real())])?;
    let source = fixture.symbol("z")?;
    let first = fixture.symbol("r")?;
    let second = fixture.symbol("s")?;
    let left =
        fixture
            .cas
            .define_reading(&first, &source, &map(1, 1, &[1], &[7])?, &mut ledger()?)?;
    let right =
        fixture
            .cas
            .define_reading(&second, &source, &map(1, 1, &[-1], &[7])?, &mut ledger()?)?;
    refuses(
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &left, &right, &mut ledger()?),
        InvalidCode::ReadingMismatch,
    );
    Ok(())
}

/// Omitting the actual expansion record invalidates the witness closure before publication.
/// ´test:integration:reading-witness-requires-defining-records´
#[test]
fn reading_witness_requires_defining_records() -> TestResult {
    let mut source = complex_case(ComplexChange::None)?;
    let witness = source.prove(&mut ledger()?)?;
    let mut bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut signature = vec![50];
    signature.extend_from_slice(&2_u64.to_le_bytes());
    signature.extend_from_slice(&4_u64.to_le_bytes());
    let (_, record) = crate::testing::answer::records(&bytes)?
        .into_iter()
        .find(|(tag, range)| {
            *tag == 3
                && bytes.get(range.clone()).is_some_and(|payload| {
                    payload
                        .windows(signature.len())
                        .any(|window| window == signature)
                })
        })
        .ok_or("expansion record")?;
    bytes.drain(record.start.checked_sub(5).ok_or("record framing")?..record.end);
    let mut receiver = complex_case(ComplexChange::None)?;
    let expected = receiver.expected()?;
    let before = receiver.fixture.cas.counts();
    let result = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?);
    assert!(matches!(
        result,
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::Encoding | InvalidCode::Reference),
            ..
        })
    ));
    assert_eq!(receiver.fixture.cas.counts(), before);
    Ok(())
}

/// Scalar commutativity cannot turn an ordered matrix product into its reversed product.
/// ´test:integration:reading-matrix-products-remain-ordered´
#[test]
fn reading_matrix_products_remain_ordered() -> TestResult {
    use crate::testing::readings::matrix;

    let mut fixture = Fixture::new(&[("a", matrix(2, 2)), ("b", matrix(2, 2))])?;
    let first = fixture.symbol("a")?;
    let second = fixture.symbol("b")?;
    let left = fixture.term(
        Operator::MatrixMultiply,
        matrix(2, 2),
        &[first.clone(), second.clone()],
    )?;
    let right = fixture.term(Operator::MatrixMultiply, matrix(2, 2), &[second, first])?;
    let domain = fixture.cas.admissible_region(&fixture.context)?;
    let step = crate::testing::witness::equality(
        &fixture.context,
        &domain,
        Primitive::MulCommute,
        &left,
        &right,
    );
    refuses(
        fixture
            .cas
            .check_proof(&fixture.context, &[step], &mut ledger()?),
        InvalidCode::UnsupportedFragment,
    );
    refuses(
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &left, &right, &mut ledger()?),
        InvalidCode::ReadingDefinition,
    );
    Ok(())
}

/// Native ordered products are total after every original factor has its own domain checked.
/// ´test:integration:native-matrix-product-reflexivity-replays´
#[test]
fn native_matrix_product_reflexivity_replays() -> TestResult {
    let mut source = native_product()?;
    let expected = source.expected()?;
    let step = crate::testing::witness::equality(
        &source.fixture.context,
        &source
            .fixture
            .cas
            .admissible_region(&source.fixture.context)?,
        Primitive::Reflexivity,
        &source.left,
        &source.right,
    );
    let proof = source
        .fixture
        .cas
        .check_proof(&source.fixture.context, &[step], &mut ledger()?)?;
    assert!(proof.is_discharged());
    assert_eq!(proof.judgment(), &expected);
    let bytes = source.fixture.cas.export_witness(&proof, &mut ledger()?)?;
    let mut receiver = native_product()?;
    let expected = receiver.expected()?;
    let checked = receiver
        .fixture
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert!(checked.is_discharged());
    Ok(())
}

/// Independent product syntax keeps native order and admits no proof during construction.
fn native_product() -> TestResult<ReadingCase> {
    use crate::testing::readings::matrix;
    let mut fixture = Fixture::new(&[("A", matrix(2, 2)), ("x", matrix(2, 1))])?;
    let node = fixture.term(
        Operator::MatrixMultiply,
        matrix(2, 1),
        &[fixture.symbol("A")?, fixture.symbol("x")?],
    )?;
    Ok(ReadingCase {
        fixture,
        left: node.clone(),
        right: node,
    })
}

/// A structural zero factor cannot erase an invalid original entry in either ordered operand.
/// ´test:integration:native-matrix-product-zero-keeps-invalid-original´
#[test]
fn native_matrix_product_zero_keeps_invalid_original() -> TestResult {
    use crate::testing::readings::matrix;
    let mut fixture = Fixture::new(&[("a", real())])?;
    let zero = fixture.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let invalid = fixture.term(Operator::Div, real(), &[fixture.symbol("a")?, zero.clone()])?;
    let zero = fixture.term(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix(1, 1),
        &[zero],
    )?;
    let invalid = fixture.term(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix(1, 1),
        &[invalid],
    )?;
    for operands in [[zero.clone(), invalid.clone()], [invalid, zero]] {
        let node = fixture.term(Operator::MatrixMultiply, matrix(1, 1), &operands)?;
        let step = crate::testing::witness::equality(
            &fixture.context,
            &fixture.cas.admissible_region(&fixture.context)?,
            Primitive::Reflexivity,
            &node,
            &node,
        );
        let before = fixture.cas.counts();
        refuses(
            fixture
                .cas
                .check_proof(&fixture.context, &[step], &mut ledger()?),
            InvalidCode::RefutedDomain,
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Native product shape admission still refuses a changed contraction extent before any proof.
/// ´test:integration:native-matrix-product-refuses-extent-mismatch´
#[test]
fn native_matrix_product_refuses_extent_mismatch() -> TestResult {
    use crate::testing::readings::matrix;
    let mut fixture = Fixture::new(&[("A", matrix(2, 2)), ("x", matrix(3, 1))])?;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .term(
                Operator::MatrixMultiply,
                matrix(2, 1),
                &[fixture.symbol("A")?, fixture.symbol("x")?]
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
