// SPDX-License-Identifier: AGPL-3.0-only
//! Direct affine-expression replay remains distinct from transporting an original source box.

use super::{
    CheckedLatticeBasis, EnclosureOrigin, Fixture, Operator, Rational, RationalEnclosure,
    TestResult, column, ledger, prepared, real,
};
use nrz_cas::lattice::{LatticeCoordinateEnclosure, RationalBoxEnclosure};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{FaultKind, InvalidCode, Location};

fn direct(
    kind: u32,
) -> TestResult<(
    Fixture,
    Box<CheckedLatticeBasis>,
    LatticeCoordinateEnclosure,
)> {
    let (mut case, basis, affine) = prepared(kind)?;
    let entries = case.cas.operands(&affine.inverse, &mut ledger()?)?;
    let count = affine.original.coordinates.len();
    let mut coordinates = Vec::new();
    for row in 0..count {
        let start = row * (count + 1);
        let mut target = entries[start + count].clone();
        for (column, original) in affine.original.coordinates.iter().enumerate() {
            let product = case.term(
                Operator::Mul,
                real(),
                &[entries[start + column].clone(), original.target.clone()],
            )?;
            target = case.term(Operator::Add, real(), &[target, product])?;
        }
        let (lower, upper, origin) = if kind == 2 || kind == 3 {
            let value = Rational::from_ratio(
                if kind == 2 { 5 } else { 1 },
                if kind == 2 { 4 } else { 1 },
                &mut ledger()?,
            )?;
            (
                value.clone(),
                value.clone(),
                EnclosureOrigin::ExactRational {
                    expression: target.clone(),
                    value,
                },
            )
        } else {
            let denominator = if row == 1 { 128 } else { 64 };
            (
                Rational::from_ratio(203, denominator, &mut ledger()?)?,
                Rational::from_ratio(205, denominator, &mut ledger()?)?,
                EnclosureOrigin::PiBound {
                    expression: target.clone(),
                    certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
                },
            )
        };
        coordinates.push(RationalEnclosure {
            target,
            context: case.context.clone(),
            input_family: affine.original.input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    let terms = coordinates
        .iter()
        .map(|coordinate| coordinate.target.clone())
        .collect::<Vec<_>>();
    let transformed = RationalBoxEnclosure {
        target: column(&mut case, &terms)?,
        context: case.context.clone(),
        input_family: affine.original.input_family,
        coordinates,
    };
    let request = LatticeCoordinateEnclosure {
        basis: affine.basis,
        target: affine.original.target,
        inverse: affine.inverse,
        coordinates: transformed,
    };
    Ok((case, basis, request))
}

/// The original affine expression can prove exact phase cancellation without claiming E inclusion.
/// ´test:integration:lattice-direct-coordinate-boxes´
#[test]
fn lattice_direct_coordinate_boxes() -> TestResult {
    for kind in 0..5 {
        let (mut case, basis, request) = direct(kind)?;
        let before = case.cas.counts();
        let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
        let checked =
            case.cas
                .check_lattice_coordinate_enclosure(&basis, &request, &mut ledger()?)?;
        assert_eq!(checked.contract(), &request);
        assert_eq!(
            checked.basis_proofs()[0],
            *basis.inverse().nonsingularity().proof()
        );
        assert_eq!(case.cas.counts(), before);
        assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}

/// A true standalone scalar box cannot replace the original affine-expression correspondence.
/// ´test:integration:lattice-direct-coordinate-correspondence´
#[test]
fn lattice_direct_coordinate_correspondence() -> TestResult {
    let (mut case, basis, mut request) = direct(3)?;
    let replacement = super::super::number(&mut case, 1)?;
    request.coordinates.target = column(&mut case, std::slice::from_ref(&replacement))?;
    request.coordinates.coordinates[0].target = replacement.clone();
    request.coordinates.coordinates[0].origin = EnclosureOrigin::ExactRational {
        expression: replacement,
        value: Rational::one(),
    };
    let before = case.cas.counts();
    let fault = case
        .cas
        .check_lattice_coordinate_enclosure(&basis, &request, &mut ledger()?)
        .err()
        .ok_or("surrogate affine expression accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "lattice coordinate box enclosure");
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Direct affine-expression replay keeps its own measured ownership boundary.
/// ´test:integration:lattice-direct-coordinate-costs´
/// ´claim:cas:lattice-direct-coordinate-costs´
#[test]
fn lattice_direct_coordinate_costs() -> TestResult {
    let mut rows = Vec::new();
    for kind in 0..5 {
        let (mut case, basis, request) = direct(kind)?;
        let mut budget = ledger()?;
        let checked = case
            .cas
            .check_lattice_coordinate_enclosure(&basis, &request, &mut budget)?;
        rows.push((kind, budget.usage().work, checked.retained_bytes()));
    }
    assert_eq!(
        rows,
        [
            (0, 201_690, 3_440),
            (1, 201_690, 3_440),
            (2, 60_913, 1_560),
            (3, 59_633, 1_560),
            (4, 83_132, 2_104),
        ]
    );
    Ok(())
}

/// Literal N/N−1 boundaries retain prior Work and refuse without a partial transformed box.
/// ´test:integration:lattice-direct-coordinate-work´
/// ´claim:cas:lattice-direct-coordinate-work´
#[test]
fn lattice_direct_coordinate_work() -> TestResult {
    for (kind, work) in [(0, 201_690), (2, 60_913), (3, 59_633), (4, 83_132)] {
        for short in [0, 1] {
            let (mut case, basis, request) = direct(kind)?;
            let before = case.cas.counts();
            let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
            let limits = Limits {
                work: work + 37 - short,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior direct affine replay"))?;
            let result = case
                .cas
                .check_lattice_coordinate_enclosure(&basis, &request, &mut budget);
            if short == 0 {
                assert_eq!(result?.contract(), &request);
                assert_eq!(budget.usage().work, work + 37);
            } else {
                let fault = result.err().ok_or("direct affine replay exceeded Work")?;
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

/// A changed q cannot reuse the same exact affine expression and proposed H.
/// ´test:integration:lattice-direct-coordinate-original-target´
#[test]
fn lattice_direct_coordinate_original_target() -> TestResult {
    let (mut case, basis, mut request) = direct(3)?;
    let replacement = super::super::number(&mut case, 6)?;
    request.target = column(&mut case, std::slice::from_ref(&replacement))?;
    let before = case.cas.counts();
    let fault = case
        .cas
        .check_lattice_coordinate_enclosure(&basis, &request, &mut ledger()?)
        .err()
        .ok_or("changed original direct target accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "lattice coordinate box enclosure");
    assert_eq!(case.cas.counts(), before);
    Ok(())
}
