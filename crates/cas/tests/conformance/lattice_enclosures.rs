// SPDX-License-Identifier: AGPL-3.0-only
//! Vector boxes preserve original operand order and reuse scalar certificate authority.

mod affine;

use crate::testing::common::{TestResult, real};
use crate::testing::readings::{Fixture, ledger, matrix};
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::lattice::RationalBoxEnclosure;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, Handle, InvalidCode, Location, Rational};

fn number(case: &mut Fixture, value: i64) -> TestResult<Handle> {
    case.term(
        Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
        real(),
        &[],
    )
}

fn column(case: &mut Fixture, entries: &[Handle]) -> TestResult<Handle> {
    let rows = u32::try_from(entries.len())?;
    case.term(
        Operator::Matrix { rows, columns: 1 },
        matrix(rows, 1),
        entries,
    )
}

fn fixture(kind: u32) -> TestResult<(Fixture, RationalBoxEnclosure)> {
    let mut case = Fixture::new(&[])?;
    let mut coordinates = Vec::new();
    if kind < 2 {
        for (value, lower, upper) in [
            (
                20,
                if kind == 0 { 18 } else { 19 },
                if kind == 0 { 21 } else { 20 },
            ),
            (10, 9, 10),
        ] {
            let target = number(&mut case, value)?;
            coordinates.push(RationalEnclosure {
                origin: EnclosureOrigin::ExactRational {
                    expression: target.clone(),
                    value: Rational::from_ratio(value, 1, &mut ledger()?)?,
                },
                target,
                context: case.context.clone(),
                input_family: case.cas.admissible_region(&case.context)?,
                lower: Rational::from_ratio(lower, 1, &mut ledger()?)?,
                upper: Rational::from_ratio(upper, 1, &mut ledger()?)?,
            });
        }
    } else {
        let pi = case.term(Operator::Constant(Constant::Pi), real(), &[])?;
        let (target, lower, upper) = match kind {
            2 => {
                let factor = case.term(
                    Operator::Rational(Rational::from_ratio(5, 2, &mut ledger()?)?),
                    real(),
                    &[],
                )?;
                (
                    case.term(Operator::Mul, real(), &[factor, pi])?,
                    (15, 2),
                    (8, 1),
                )
            }
            3 => {
                let two = number(&mut case, 2)?;
                (
                    case.term(Operator::Mul, real(), &[two, pi])?,
                    (6, 1),
                    (13, 2),
                )
            }
            _ => {
                let ten = number(&mut case, 10)?;
                (
                    case.term(Operator::Div, real(), &[ten, pi])?,
                    (203, 64),
                    (205, 64),
                )
            }
        };
        coordinates.push(RationalEnclosure {
            origin: EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(super::pi_bounds::cycle()?),
            },
            target,
            context: case.context.clone(),
            input_family: case.cas.admissible_region(&case.context)?,
            lower: Rational::from_ratio(lower.0, lower.1, &mut ledger()?)?,
            upper: Rational::from_ratio(upper.0, upper.1, &mut ledger()?)?,
        });
    }
    let entries = coordinates
        .iter()
        .map(|coordinate| coordinate.target.clone())
        .collect::<Vec<_>>();
    let target = column(&mut case, &entries)?;
    let request = RationalBoxEnclosure {
        target,
        context: case.context.clone(),
        input_family: case.cas.admissible_region(&case.context)?,
        coordinates,
    };
    Ok((case, request))
}

/// Original torus, phase and LC boxes keep every coordinate and exact constant certificate.
/// ´test:integration:lattice-original-boxes´
#[test]
fn lattice_original_boxes() -> TestResult {
    for kind in 0..5 {
        let (case, request) = fixture(kind)?;
        let before = case.cas.counts();
        let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
        let checked = case
            .cas
            .check_rational_box_enclosure(&request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(case.cas.counts(), before);
        assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        if kind >= 2 {
            assert!(matches!(
                checked.contract().coordinates[0].origin,
                EnclosureOrigin::PiBound { .. }
            ));
        }
    }
    Ok(())
}

fn refuses(
    case: &Fixture,
    request: &RationalBoxEnclosure,
    expected: InvalidCode,
    at: &'static str,
) -> TestResult {
    let before = case.cas.counts();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = case
        .cas
        .check_rational_box_enclosure(request, &mut budget)
        .err()
        .ok_or("false vector box accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fault.location, Location::new(at));
    assert_eq!(case.cas.counts(), before);
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert!(budget.usage().work > 0);
    Ok(())
}

/// Changed vector, order, arity and context are refused by the vector identity checker.
/// ´test:integration:lattice-original-box-identities´
#[test]
fn lattice_original_box_identities() -> TestResult {
    let (mut case, request) = fixture(1)?;
    let at = "original rational box enclosure";
    let mut changed = request.clone();
    changed.coordinates.swap(0, 1);
    refuses(&case, &changed, InvalidCode::EndpointMismatch, at)?;
    changed.clone_from(&request);
    changed.coordinates.pop();
    refuses(&case, &changed, InvalidCode::ReadingShape, at)?;
    changed.clone_from(&request);
    let replacement = number(&mut case, 21)?;
    changed.target = column(
        &mut case,
        &[replacement, request.coordinates[1].target.clone()],
    )?;
    refuses(&case, &changed, InvalidCode::EndpointMismatch, at)?;
    changed.clone_from(&request);
    let foreign = fixture(1)?;
    changed.coordinates[0].context = foreign.1.context;
    refuses(&case, &changed, InvalidCode::EndpointMismatch, at)?;
    changed.clone_from(&request);
    changed.coordinates[0].input_family = foreign.1.input_family;
    refuses(&case, &changed, InvalidCode::EndpointMismatch, at)
}

/// A false final coordinate or missing constant proof never returns a partial box.
/// ´test:integration:lattice-original-box-refusals´
#[test]
fn lattice_original_box_refusals() -> TestResult {
    let (case, mut request) = fixture(1)?;
    request.coordinates[1].lower = Rational::from_ratio(11, 1, &mut ledger()?)?;
    request.coordinates[1].upper = Rational::from_ratio(12, 1, &mut ledger()?)?;
    refuses(
        &case,
        &request,
        InvalidCode::DecisionBox,
        "exact rational enclosure",
    )?;
    let (case, mut request) = fixture(2)?;
    request.coordinates[0].origin = EnclosureOrigin::Unverified;
    refuses(
        &case,
        &request,
        InvalidCode::UnsupportedCertificate,
        "exact rational enclosure",
    )
}

/// Complete original-box costs include scalar replay and the retained vector contract.
/// ´test:integration:lattice-original-box-costs´
#[test]
fn lattice_original_box_costs() -> TestResult {
    let mut rows = Vec::new();
    for kind in 0..5 {
        let (case, request) = fixture(kind)?;
        let mut budget = ledger()?;
        let checked = case
            .cas
            .check_rational_box_enclosure(&request, &mut budget)?;
        rows.push((kind, budget.usage().work, checked.retained_bytes()));
    }
    assert_eq!(
        rows,
        [
            (0, 16_236, 1_840),
            (1, 16_300, 1_840),
            (2, 56_902, 1_592),
            (3, 55_878, 1_592),
            (4, 58_049, 1_592),
        ]
    );
    Ok(())
}

/// Literal N/N−1 Work limits include prior usage and return no partial vector.
/// ´test:integration:lattice-original-box-work´
/// ´claim:cas:lattice-original-box-work´
#[test]
fn lattice_original_box_work() -> TestResult {
    for (kind, work) in [
        (0, 16_236),
        (1, 16_300),
        (2, 56_902),
        (3, 55_878),
        (4, 58_049),
    ] {
        for short in [0, 1] {
            let (case, request) = fixture(kind)?;
            let before = case.cas.counts();
            let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
            let limits = Limits {
                work: work + 37 - short,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior original vector replay"))?;
            let result = case.cas.check_rational_box_enclosure(&request, &mut budget);
            if short == 0 {
                assert_eq!(result?.contract(), &request);
                assert_eq!(budget.usage().work, work + 37);
            } else {
                let fault = result.err().ok_or("vector replay exceeded Work")?;
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
            assert_eq!(case.cas.counts(), before);
            assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
    }
    Ok(())
}

/// Returned vector ownership has a literal output boundary after complete replay.
/// ´test:integration:lattice-original-box-output´
#[test]
fn lattice_original_box_output() -> TestResult {
    for (kind, bytes) in [(1, 1_840), (2, 1_592)] {
        let (case, request) = fixture(kind)?;
        let before = case.cas.counts();
        for short in [0, 1] {
            let mut budget = Budget::new(Limits {
                output_bytes: bytes - short,
                ..Limits::default()
            })?;
            let result = case.cas.check_rational_box_enclosure(&request, &mut budget);
            if short == 0 {
                assert_eq!(result?.retained_bytes(), bytes);
            } else {
                let fault = result
                    .err()
                    .ok_or("vector receipt exceeded output capacity")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::OutputBytes,
                        ..
                    }
                ));
                assert_eq!(fault.location.operation, "original rational box enclosure");
            }
            assert_eq!(case.cas.counts(), before);
        }
    }
    Ok(())
}
