// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-box affine transport retains the exact inverse and Machin coefficient evidence.

mod direct;

use super::{Fixture, TestResult, column, fixture, ledger, matrix, number, real};
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::lattice::{
    CheckedLatticeBasis, LatticeBasisOutcome, LatticeBasisQuestion, LatticeBoxEnclosure,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, InvalidCode, Location, Rational};

fn prepared(kind: u32) -> TestResult<(Fixture, Box<CheckedLatticeBasis>, LatticeBoxEnclosure)> {
    if kind == 5 {
        return skew();
    }
    let (mut case, original) = fixture(kind)?;
    let count = u32::try_from(original.coordinates.len())?;
    let zero = number(&mut case, 0)?;
    let scale = if kind == 4 {
        number(&mut case, 1)?
    } else {
        let pi = case.term(Operator::Constant(Constant::Pi), real(), &[])?;
        let two = number(&mut case, 2)?;
        case.term(Operator::Mul, real(), &[two, pi])?
    };
    let entries = if count == 1 {
        vec![scale]
    } else {
        vec![scale.clone(), zero.clone(), zero.clone(), scale]
    };
    let basis = case.term(
        Operator::Matrix {
            rows: count,
            columns: count,
        },
        matrix(count, count),
        &entries,
    )?;
    let offsets = vec![zero; usize::try_from(count)?];
    let question = LatticeBasisQuestion {
        context: case.context.clone(),
        basis,
        offset: column(&mut case, &offsets)?,
    };
    let LatticeBasisOutcome::Admitted(basis) =
        case.cas.admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("affine enclosure basis unresolved".into());
    };
    let inverse = basis.inverse().map().expression().clone();
    let inverse_entries = case.cas.operands(&inverse, &mut ledger()?)?;
    let mut coefficients = Vec::new();
    let width = usize::try_from(count)? + 1;
    for (position, target) in inverse_entries
        .into_iter()
        .take(usize::try_from(count)? * width)
        .enumerate()
    {
        let diagonal = position / width == position % width;
        let (lower, upper, origin) = if diagonal && kind != 4 {
            (
                Rational::from_ratio(203, 1280, &mut ledger()?)?,
                Rational::from_ratio(205, 1280, &mut ledger()?)?,
                EnclosureOrigin::PiBound {
                    expression: target.clone(),
                    certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
                },
            )
        } else {
            let value = Rational::from_ratio(i64::from(diagonal), 1, &mut ledger()?)?;
            (
                value.clone(),
                value.clone(),
                EnclosureOrigin::ExactRational {
                    expression: target.clone(),
                    value,
                },
            )
        };
        coefficients.push(RationalEnclosure {
            target,
            context: case.context.clone(),
            input_family: original.input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    let values = match kind {
        0 => vec![(1827, 640, 861, 256), (1827, 1280, 205, 128)],
        1 => vec![(3857, 1280, 205, 64), (1827, 1280, 205, 128)],
        2 => vec![(609, 512, 41, 32)],
        3 => vec![(609, 640, 533, 512)],
        _ => vec![(203, 64, 205, 64)],
    };
    let bounds = values
        .into_iter()
        .map(|(ln, ld, un, ud)| {
            Ok((
                Rational::from_ratio(ln, ld, &mut ledger()?)?,
                Rational::from_ratio(un, ud, &mut ledger()?)?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    let request = LatticeBoxEnclosure {
        basis: question,
        original,
        inverse,
        coefficients,
        bounds,
    };
    Ok((case, basis, request))
}

/// Exact inverse-entry certificates transport every original box without a decimal surrogate.
/// ´test:integration:lattice-affine-boxes´
#[test]
fn lattice_affine_boxes() -> TestResult {
    for kind in 0..6 {
        let (mut case, basis, request) = prepared(kind)?;
        let before = case.cas.counts();
        let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
        let checked = case
            .cas
            .check_lattice_box_enclosure(&basis, &request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(
            checked.basis_proofs(),
            &[
                basis.inverse().nonsingularity().proof().clone(),
                basis.inverse().witness().proof().clone(),
                basis.left_inverse().proof().clone(),
            ]
        );
        let published = kind < 5;
        crate::conformance::pi_bounds::graph_after_citation(&case.cas, before, published);
        if !published {
            assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
    }
    Ok(())
}

/// A wide E cannot be replaced by the same target's sharper point enclosure during transport.
/// ´test:integration:lattice-affine-box-whole-source´
#[test]
fn lattice_affine_box_whole_source() -> TestResult {
    let (mut case, basis, mut request) = prepared(0)?;
    request.bounds[0].0 = Rational::from_ratio(3, 1, &mut ledger()?)?;
    let before = case.cas.counts();
    let fault = case
        .cas
        .check_lattice_box_enclosure(&basis, &request, &mut ledger()?)
        .err()
        .ok_or("wide source box was sharpened")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::DecisionBox));
    assert_eq!(fault.location.operation, "lattice affine box enclosure");
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Complete transport costs include original replay and full-contract coefficient reuse.
/// ´test:integration:lattice-affine-box-costs´
/// ´claim:cas:lattice-affine-box-costs´
#[test]
fn lattice_affine_box_costs() -> TestResult {
    let mut rows = Vec::new();
    for kind in 0..6 {
        let (mut case, basis, request) = prepared(kind)?;
        let mut budget = ledger()?;
        let checked = case
            .cas
            .check_lattice_box_enclosure(&basis, &request, &mut budget)?;
        rows.push((kind, budget.usage().work, checked.retained_bytes()));
    }
    assert_eq!(
        rows,
        [
            (0, 119_257, 8_728),
            (1, 119_025, 8_720),
            (2, 114_216, 4_508),
            (3, 113_232, 4_516),
            (4, 80_471, 3_948),
            (5, 74_753, 7_648),
        ]
    );
    Ok(())
}

fn refusal(
    case: &mut Fixture,
    basis: &CheckedLatticeBasis,
    request: &LatticeBoxEnclosure,
    expected: InvalidCode,
    at: &str,
) -> TestResult {
    let before = case.cas.counts();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = case
        .cas
        .check_lattice_box_enclosure(basis, request, &mut budget)
        .err()
        .ok_or("false affine box accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fault.location.operation, at);
    assert_eq!(case.cas.counts(), before);
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert!(budget.usage().work > 0);
    Ok(())
}

/// Inverse identity, coefficient order and original target mutations name their refusing checker.
/// ´test:integration:lattice-affine-box-identities´
#[test]
fn lattice_affine_box_identities() -> TestResult {
    let (mut case, basis, request) = prepared(1)?;
    let mut changed = request.clone();
    changed.inverse = basis.forward().expression().clone();
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::EndpointMismatch,
        "lattice affine box enclosure",
    )?;
    changed.clone_from(&request);
    changed.coefficients.swap(0, 1);
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::EndpointMismatch,
        "lattice affine box enclosure",
    )?;
    changed.clone_from(&request);
    changed.coefficients.pop();
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::ReadingShape,
        "lattice affine box enclosure",
    )?;
    changed.clone_from(&request);
    let replacement = number(&mut case, 21)?;
    changed.original.target = column(
        &mut case,
        &[replacement, request.original.coordinates[1].target.clone()],
    )?;
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::EndpointMismatch,
        "original rational box enclosure",
    )?;
    changed.clone_from(&request);
    let one = number(&mut case, 1)?;
    changed.basis.offset = column(&mut case, &[one.clone(), one])?;
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::EndpointMismatch,
        "lattice basis admission",
    )
}

/// Exact symbolic nonsingularity does not authorize an interval reciprocal meeting zero.
/// ´test:integration:lattice-affine-box-reciprocal´
#[test]
fn lattice_affine_box_reciprocal() -> TestResult {
    let (mut case, basis, mut request) = prepared(2)?;
    let EnclosureOrigin::PiBound { certificate, .. } = &mut request.coefficients[0].origin else {
        return Err("missing inverse Machin certificate".into());
    };
    certificate.lower = Rational::from_ratio(-4, 1, &mut ledger()?)?;
    certificate.upper = Rational::from_ratio(4, 1, &mut ledger()?)?;
    refusal(
        &mut case,
        &basis,
        &request,
        InvalidCode::UnsupportedCertificate,
        "closed pi enclosure",
    )?;
    request.coefficients[0].origin = EnclosureOrigin::Unverified;
    refusal(
        &mut case,
        &basis,
        &request,
        InvalidCode::UnsupportedCertificate,
        "exact rational enclosure",
    )
}

fn skew() -> TestResult<(Fixture, Box<CheckedLatticeBasis>, LatticeBoxEnclosure)> {
    let (mut case, original) = fixture(1)?;
    let mut entries = Vec::new();
    for value in [1, 1, 1, 2] {
        entries.push(number(&mut case, value)?);
    }
    let basis = case.term(
        Operator::Matrix {
            rows: 2,
            columns: 2,
        },
        matrix(2, 2),
        &entries,
    )?;
    let offsets = [number(&mut case, 3)?, number(&mut case, 5)?];
    let question = LatticeBasisQuestion {
        context: case.context.clone(),
        basis,
        offset: column(&mut case, &offsets)?,
    };
    let LatticeBasisOutcome::Admitted(basis) =
        case.cas.admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("skew basis unresolved".into());
    };
    let inverse = basis.inverse().map().expression().clone();
    let entries = case.cas.operands(&inverse, &mut ledger()?)?;
    let mut coefficients = Vec::new();
    for (target, number) in entries.into_iter().zip([2, -1, -1, -1, 1, -2]) {
        let value = Rational::from_ratio(number, 1, &mut ledger()?)?;
        coefficients.push(RationalEnclosure {
            origin: EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value: value.clone(),
            },
            target,
            context: case.context.clone(),
            input_family: original.input_family.clone(),
            lower: value.clone(),
            upper: value,
        });
    }
    let bounds = [(27, 30), (-13, -11)]
        .into_iter()
        .map(|(lower, upper)| {
            Ok((
                Rational::from_ratio(lower, 1, &mut ledger()?)?,
                Rational::from_ratio(upper, 1, &mut ledger()?)?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    Ok((
        case,
        basis,
        LatticeBoxEnclosure {
            basis: question,
            original,
            inverse,
            coefficients,
            bounds,
        },
    ))
}

/// Negative inverse coefficients and a translated origin use both extrema of every source row.
/// ´test:integration:lattice-affine-box-skew´
#[test]
fn lattice_affine_box_skew() -> TestResult {
    let (mut case, basis, mut request) = skew()?;
    case.cas
        .check_lattice_box_enclosure(&basis, &request, &mut ledger()?)?;
    request.bounds[1].0 = Rational::from_ratio(-12, 1, &mut ledger()?)?;
    refusal(
        &mut case,
        &basis,
        &request,
        InvalidCode::DecisionBox,
        "lattice affine box enclosure",
    )
}

/// Reused coefficient evidence is keyed by the complete contract, including bounds and Machin data.
/// ´test:integration:lattice-affine-box-reuse-identity´
#[test]
fn lattice_affine_box_reuse_identity() -> TestResult {
    let (mut case, basis, request) = prepared(1)?;
    assert_eq!(request.coefficients[0], request.coefficients[4]);
    let mut changed = request.clone();
    changed.coefficients[4].upper = Rational::one();
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::DecisionBox,
        "lattice affine box enclosure",
    )?;
    changed.clone_from(&request);
    let EnclosureOrigin::PiBound { certificate, .. } = &mut changed.coefficients[4].origin else {
        return Err("missing repeated Machin evidence".into());
    };
    certificate.sums[0] = Rational::zero();
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::EndpointMismatch,
        "Machin pi bounds",
    )?;
    changed.clone_from(&request);
    changed.coefficients[4].origin = EnclosureOrigin::Unverified;
    refusal(
        &mut case,
        &basis,
        &changed,
        InvalidCode::UnsupportedCertificate,
        "exact rational enclosure",
    )
}

/// Literal N/N−1 transport ceilings retain prior Work without exposing a partial H.
/// ´test:integration:lattice-affine-box-work´
/// ´claim:cas:lattice-affine-box-work´
#[test]
fn lattice_affine_box_work() -> TestResult {
    for (kind, work) in [
        (0, 119_257),
        (1, 119_025),
        (2, 114_216),
        (3, 113_232),
        (4, 80_471),
        (5, 74_753),
    ] {
        for short in [0, 1] {
            let (mut case, basis, request) = prepared(kind)?;
            let before = case.cas.counts();
            let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
            let limits = Limits {
                work: work + 37 - short,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior affine box transport"))?;
            let result = case
                .cas
                .check_lattice_box_enclosure(&basis, &request, &mut budget);
            if short == 0 {
                assert_eq!(result?.contract(), &request);
                assert_eq!(budget.usage().work, work + 37);
            } else {
                let fault = result.err().ok_or("affine transport exceeded Work")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
            }
            assert!(budget.usage().work > 37);
            assert_eq!(budget.limits(), &limits);
            let published = kind < 5 && short == 0;
            crate::conformance::pi_bounds::graph_after_citation(&case.cas, before, published);
            if !published {
                assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
            }
        }
    }
    Ok(())
}
