// SPDX-License-Identifier: AGPL-3.0-only
//! Exact inverse laws and named whole-family refusals through the public API.

use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{constant_map, fixture, ledger, parameter_case, rational_case};
use crate::testing::readings::map;
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, InvalidCode, Rational};

fn refuses<T>(result: Result<T, Fault>, expected: InvalidCode) {
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(expected))
    );
}

/// Diagonal, triangular, general, expansion-restriction and empty maps match independent inverses.
/// ´test:integration:general-map-inverse-identity´
#[test]
fn general_map_inverse_identity() -> TestResult {
    for kind in 0..5 {
        let (mut case, original, expected) = rational_case(kind)?;
        let inverse = case
            .cas
            .invert_reading_map(&case.context, &original, &mut ledger()?)?;
        assert!(inverse.witness().is_discharged());
        case.cas.prove_reading_equality(
            &case.context,
            inverse.map().expression(),
            expected.expression(),
            &mut ledger()?,
        )?;
    }
    Ok(())
}

/// The rational convenience API uses the same inverse producer and preserves existing composition.
/// ´test:integration:rational-map-inverse-specialization´
#[test]
fn rational_map_inverse_specialization() -> TestResult {
    let original = map(2, 2, &[2, 3, 5, 7], &[11, 13])?;
    let inverse = original.inverse(&mut ledger()?)?;
    assert_eq!(inverse.map(), &map(2, 2, &[-7, 3, 5, -2], &[38, -29])?);
    assert_eq!(
        inverse.determinant(),
        &Rational::from_ratio(-1, 1, &mut ledger()?)?
    );
    assert_eq!(
        original.compose(inverse.map(), &mut ledger()?)?,
        nrz_cas::readings::AffineMap::identity(2, &mut ledger()?)?
    );
    let expansion = map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?;
    let restriction = expansion.project(&[2, 3], &mut ledger()?)?;
    assert_eq!(
        restriction.inverse(&mut ledger()?)?.map(),
        &map(2, 2, &[0, 1, -1, 0], &[0, 0])?
    );
    Ok(())
}

/// Exact singularity and unproved family nonsingularity are distinct atomic refusals.
/// ´test:integration:map-inverse-family-refusals´
#[test]
fn map_inverse_family_refusals() -> TestResult {
    let mut case = fixture(&[])?;
    let singular = constant_map(&mut case, 2, &["1", "2", "2", "4"], &["0", "0"])?;
    let before = case.cas.counts();
    refuses(
        case.cas
            .invert_reading_map(&case.context, &singular, &mut ledger()?),
        InvalidCode::ReadingSingular,
    );
    assert_eq!(case.cas.counts(), before);
    let (mut case, varying, _) = parameter_case(false)?;
    let before = case.cas.counts();
    refuses(
        case.cas
            .invert_reading_map(&case.context, &varying, &mut ledger()?),
        InvalidCode::ReadingNonsingularity,
    );
    assert_eq!(case.cas.counts(), before);
    let (mut guarded, varying, _) = parameter_case(true)?;
    let inverse = guarded
        .cas
        .invert_reading_map(&guarded.context, &varying, &mut ledger()?)?;
    assert_eq!(inverse.witness().hypotheses().len(), 1);
    refuses(
        case.cas
            .invert_reading_map(&case.context, &varying, &mut ledger()?),
        InvalidCode::ForeignHandle,
    );
    Ok(())
}

/// An invalid original divisor beneath a zero coefficient cannot vanish during inversion.
/// ´test:integration:map-inverse-checks-original-zero-operands´
#[test]
fn map_inverse_checks_original_zero_operands() -> TestResult {
    let mut case = fixture(&[])?;
    let zero = case.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let one = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    let undefined = case.term(Operator::Div, real(), &[one.clone(), zero.clone()])?;
    let hidden = case.term(Operator::Mul, real(), &[zero, undefined])?;
    let original =
        case.cas
            .exact_reading_map(&case.context, (1, 1), &[one], &[hidden], &mut ledger()?)?;
    let before = case.cas.counts();
    refuses(
        case.cas
            .invert_reading_map(&case.context, &original, &mut ledger()?),
        InvalidCode::RefutedDomain,
    );
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Empty native products do no work proportional to the unused contraction axis.
/// ´test:integration:empty-matrix-inverse-checker-axis-cost´
#[test]
fn empty_matrix_inverse_checker_axis_cost() -> TestResult {
    let mut costs = Vec::new();
    for inner in [1, u32::MAX] {
        let mut case = fixture(&[])?;
        let left = case.term(
            Operator::Matrix {
                rows: 0,
                columns: inner,
            },
            crate::testing::readings::matrix(0, inner),
            &[],
        )?;
        let right = case.term(
            Operator::Matrix {
                rows: inner,
                columns: 0,
            },
            crate::testing::readings::matrix(inner, 0),
            &[],
        )?;
        let product = case.term(
            Operator::MatrixMultiply,
            crate::testing::readings::matrix(0, 0),
            &[left, right],
        )?;
        let expected = case.term(
            Operator::Matrix {
                rows: 0,
                columns: 0,
            },
            crate::testing::readings::matrix(0, 0),
            &[],
        )?;
        let mut measured = ledger()?;
        case.cas
            .prove_reading_equality(&case.context, &product, &expected, &mut measured)?;
        costs.push(measured.usage().work);
    }
    assert_eq!(costs.first(), costs.get(1));
    Ok(())
}

/// A varying internal elimination pivot must not narrow a matrix with constant nonzero determinant.
/// ´test:integration:determinant-checker-pivots-are-not-family-premises´
#[test]
fn determinant_checker_pivots_are_not_family_premises() -> TestResult {
    let mut case = fixture(&["p"])?;
    let parameter = case.symbol("p")?;
    let zero = case.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let one = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    let negative = case.term(Operator::Neg, real(), std::slice::from_ref(&parameter))?;
    let offsets = [zero.clone(), zero.clone()];
    let original = case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &[parameter, one.clone(), one.clone(), zero.clone()],
        &offsets,
        &mut ledger()?,
    )?;
    let expected = case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &[zero, one.clone(), one, negative],
        &offsets,
        &mut ledger()?,
    )?;
    let inverse = case
        .cas
        .invert_reading_map(&case.context, &original, &mut ledger()?)?;
    assert_eq!(
        inverse.nonsingularity().hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    case.cas.prove_reading_equality(
        &case.context,
        inverse.map().expression(),
        expected.expression(),
        &mut ledger()?,
    )?;
    Ok(())
}

/// A wrong exact determinant proposal fails the independent ordinary rational identity checker.
/// ´test:integration:determinant-certificate-rejects-wrong-value´
#[test]
fn determinant_certificate_rejects_wrong_value() -> TestResult {
    let (mut case, original, _) = rational_case(2)?;
    let wrong = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    let claim = crate::testing::map_inverses::determinant_expected(&mut case, &original, &wrong)?;
    let step = nrz_cas::proof::ApplicationDraft {
        rule: nrz_cas::proof::Primitive::RationalIdentity,
        context: case.context.clone(),
        conclusion: claim.statement,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: nrz_cas::proof::Certificate::None,
    };
    assert!(
        case.cas
            .check_proof(&case.context, &[step], &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Native explicit products prove their independent contraction and refuse reversed matrix order.
/// ´test:integration:explicit-map-products-retain-ordered-coefficients´
#[test]
fn explicit_map_products_retain_ordered_coefficients() -> TestResult {
    let (mut case, first, _) = rational_case(2)?;
    let second = constant_map(&mut case, 2, &["1", "2", "0", "1"], &["3", "4"])?;
    let forward = case
        .cas
        .compose_reading_maps(&first, &second, &mut ledger()?)?;
    let reversed = case
        .cas
        .compose_reading_maps(&second, &first, &mut ledger()?)?;
    refuses(
        case.cas.prove_reading_equality(
            &case.context,
            forward.expression(),
            reversed.expression(),
            &mut ledger()?,
        ),
        InvalidCode::ReadingMismatch,
    );
    let independent = constant_map(&mut case, 2, &["2", "7", "5", "17"], &["29", "56"])?;
    case.cas.prove_reading_equality(
        &case.context,
        forward.expression(),
        independent.expression(),
        &mut ledger()?,
    )?;
    Ok(())
}
