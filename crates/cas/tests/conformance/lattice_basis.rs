// SPDX-License-Identifier: AGPL-3.0-only
//! Exact lattice bases reuse native matrices and ordinary scoped witnesses.

use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{constant_map, determinant_expected, expected, positive};
use crate::testing::readings::{Fixture, ledger, matrix};
use nrz_cas::lattice::{CheckedLatticeBasis, LatticeBasisOutcome, LatticeBasisQuestion};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, Handle, InvalidCode, Rational};

fn literal(case: &mut Fixture, value: i64) -> TestResult<Handle> {
    case.term(
        Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
        real(),
        &[],
    )
}

fn rectangle(
    case: &mut Fixture,
    rows: u32,
    columns: u32,
    entries: &[Handle],
) -> TestResult<Handle> {
    case.term(
        Operator::Matrix { rows, columns },
        matrix(rows, columns),
        entries,
    )
}

fn question(
    case: &mut Fixture,
    count: u32,
    entries: &[Handle],
    offsets: &[Handle],
) -> TestResult<LatticeBasisQuestion> {
    Ok(LatticeBasisQuestion {
        context: case.context.clone(),
        basis: rectangle(case, count, count, entries)?,
        offset: rectangle(case, count, 1, offsets)?,
    })
}

fn rational_case(kind: u32) -> TestResult<(Fixture, LatticeBasisQuestion)> {
    let mut case = Fixture::new(&[])?;
    let values = match kind {
        0 => [1, 1, 1, 1],
        1 => [1, 1, 1, 2],
        _ => [2, 3, 5, 7],
    };
    let mut entries = Vec::new();
    for value in values {
        entries.push(literal(&mut case, value)?);
    }
    let offsets = [literal(&mut case, 3)?, literal(&mut case, 5)?];
    let request = question(&mut case, 2, &entries, &offsets)?;
    Ok((case, request))
}

fn admitted(outcome: LatticeBasisOutcome) -> TestResult<Box<CheckedLatticeBasis>> {
    match outcome {
        LatticeBasisOutcome::Admitted(value) => Ok(value),
        LatticeBasisOutcome::Unresolved => Err("basis unexpectedly unresolved".into()),
    }
}

fn phase_case() -> TestResult<(Fixture, LatticeBasisQuestion)> {
    let mut case = Fixture::new(&[])?;
    let pi = case.term(Operator::Constant(Constant::Pi), real(), &[])?;
    let two = literal(&mut case, 2)?;
    let period = case.term(Operator::Mul, real(), &[two, pi.clone()])?;
    let request = question(&mut case, 1, &[period], &[pi])?;
    Ok((case, request))
}

/// The singular minimal case refuses atomically; determinant one and negative skew twins admit.
/// ´test:integration:lattice-basis-rank´
#[test]
fn lattice_basis_rank() -> TestResult {
    let (mut case, request) = rational_case(0)?;
    let before = case.cas.counts();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut spent = ledger()?;
    let failure = case
        .cas
        .admit_lattice_basis(&request, &mut spent)
        .err()
        .ok_or("singular admitted")?;
    assert_eq!(
        failure.kind,
        FaultKind::Invalid(InvalidCode::ReadingSingular)
    );
    assert!(spent.usage().work > 0);
    assert_eq!(case.cas.counts(), before);
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    for kind in [1, 2] {
        let (mut case, request) = rational_case(kind)?;
        let basis = admitted(case.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
        assert_eq!(basis.dimension(), 2);
        assert_eq!(basis.question(), &request);
        assert!(basis.inverse().nonsingularity().is_unconditional());
        assert!(basis.inverse().witness().is_unconditional());
        assert!(basis.left_inverse().is_unconditional());
        let (coefficients, offsets): (&[&str], &[&str]) = if kind == 1 {
            (&["2", "-1", "-1", "1"], &["-1", "-2"])
        } else {
            (&["-7", "3", "5", "-2"], &["6", "-5"])
        };
        let oracle = constant_map(&mut case, 2, coefficients, offsets)?;
        case.cas.prove_reading_equality(
            &case.context,
            basis.inverse().map().expression(),
            oracle.expression(),
            &mut ledger()?,
        )?;
    }
    Ok(())
}

/// The one-dimensional phase basis retains exact 2Pi and the translated inverse.
/// ´test:integration:lattice-basis-phase´
#[test]
fn lattice_basis_phase() -> TestResult {
    let (mut case, request) = phase_case()?;
    let basis = admitted(case.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
    assert_eq!(basis.dimension(), 1);
    assert!(basis.inverse().nonsingularity().is_unconditional());
    assert!(basis.left_inverse().is_unconditional());
    let pi = case.term(Operator::Constant(Constant::Pi), real(), &[])?;
    let two = literal(&mut case, 2)?;
    let period = case.term(Operator::Mul, real(), &[two, pi])?;
    let one = literal(&mut case, 1)?;
    let reciprocal = case.term(Operator::Div, real(), &[one, period])?;
    let shift = case.term(
        Operator::Rational(Rational::from_ratio(-1, 2, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let oracle = case.cas.exact_reading_map(
        &case.context,
        (1, 1),
        &[reciprocal],
        &[shift],
        &mut ledger()?,
    )?;
    case.cas.prove_reading_equality(
        &case.context,
        basis.inverse().map().expression(),
        oracle.expression(),
        &mut ledger()?,
    )?;
    Ok(())
}

/// An unproved determinant is unresolved; an explicit parameter hypothesis admits conditionally.
/// ´test:integration:lattice-basis-parameters´
#[test]
fn lattice_basis_parameters() -> TestResult {
    let mut case = Fixture::new(&[("p", real())])?;
    let parameter = case.symbol("p")?;
    let zero = literal(&mut case, 0)?;
    let mut request = question(&mut case, 1, std::slice::from_ref(&parameter), &[zero])?;
    let before = case.cas.counts();
    let mut spent = ledger()?;
    assert!(matches!(
        case.cas.admit_lattice_basis(&request, &mut spent)?,
        LatticeBasisOutcome::Unresolved
    ));
    assert!(spent.usage().work > 0);
    assert_eq!(case.cas.counts(), before);
    positive(&mut case, &parameter, "positive-lattice-scale")?;
    request.context.clone_from(&case.context);
    let basis = admitted(case.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
    for witness in [
        basis.inverse().nonsingularity(),
        basis.inverse().witness(),
        basis.left_inverse(),
    ] {
        assert!(witness.is_discharged());
        assert_eq!(witness.hypotheses().len(), 1);
    }
    let bytes = case
        .cas
        .export_witness(basis.left_inverse(), &mut ledger()?)?;
    let mut receiver = Fixture::new(&[("p", real())])?;
    let parameter = receiver.symbol("p")?;
    let zero = literal(&mut receiver, 0)?;
    let one = literal(&mut receiver, 1)?;
    let reciprocal = receiver.term(Operator::Div, real(), &[one, parameter.clone()])?;
    let forward = receiver.cas.exact_reading_map(
        &receiver.context,
        (1, 1),
        std::slice::from_ref(&parameter),
        std::slice::from_ref(&zero),
        &mut ledger()?,
    )?;
    let inverse = receiver.cas.exact_reading_map(
        &receiver.context,
        (1, 1),
        &[reciprocal],
        &[zero],
        &mut ledger()?,
    )?;
    let missing_scope = expected(&mut receiver, &inverse, &forward)?;
    let before = receiver.cas.counts();
    assert!(
        receiver
            .cas
            .replay(&bytes, &missing_scope, &mut ledger()?)
            .is_err()
    );
    assert_eq!(receiver.cas.counts(), before);
    positive(&mut receiver, &parameter, "positive-lattice-scale")?;
    let retained_scope = expected(&mut receiver, &inverse, &forward)?;
    let replayed = receiver
        .cas
        .replay(&bytes, &retained_scope, &mut ledger()?)?;
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(
        receiver.cas.export_witness(&replayed, &mut ledger()?)?,
        bytes
    );
    Ok(())
}

/// A finite constant with unsupported nonzero evidence remains unresolved without partial admission.
/// ´test:integration:lattice-basis-unresolved-constant´
#[test]
fn lattice_basis_unresolved_constant() -> TestResult {
    let mut case = Fixture::new(&[])?;
    let pi = case.term(Operator::Constant(Constant::Pi), real(), &[])?;
    let three = literal(&mut case, 3)?;
    let scale = case.term(Operator::Sub, real(), &[pi, three])?;
    let zero = literal(&mut case, 0)?;
    let request = question(&mut case, 1, &[scale], &[zero])?;
    let before = case.cas.counts();
    let mut budget = ledger()?;
    assert!(matches!(
        case.cas.admit_lattice_basis(&request, &mut budget)?,
        LatticeBasisOutcome::Unresolved
    ));
    assert!(budget.usage().work > 0);
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Original coefficients and offsets are checked before zero multiplication or singularity can hide them.
/// ´test:integration:lattice-basis-original-domains´
#[test]
fn lattice_basis_original_domains() -> TestResult {
    for position in [0, 1, 2] {
        let mut case = Fixture::new(&[])?;
        let zero = literal(&mut case, 0)?;
        let one = literal(&mut case, 1)?;
        let undefined = case.term(Operator::Div, real(), &[one.clone(), zero.clone()])?;
        let hidden = case.term(Operator::Mul, real(), &[zero.clone(), undefined])?;
        let (entry, offset) = match position {
            0 => (hidden, zero),
            1 => (one, hidden),
            _ => (zero, hidden),
        };
        let request = question(&mut case, 1, &[entry], &[offset])?;
        let before = case.cas.counts();
        let failure = case
            .cas
            .admit_lattice_basis(&request, &mut ledger()?)
            .err()
            .ok_or("undefined admitted")?;
        assert_eq!(failure.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
        assert_eq!(case.cas.counts(), before);
    }
    Ok(())
}

/// Empty, nonsquare and incorrectly shaped offsets cannot declare a full-rank finite frame.
/// ´test:integration:lattice-basis-shapes´
#[test]
fn lattice_basis_shapes() -> TestResult {
    let mut case = Fixture::new(&[])?;
    let zero = literal(&mut case, 0)?;
    let one = literal(&mut case, 1)?;
    let empty = rectangle(&mut case, 0, 0, &[])?;
    let empty_offset = rectangle(&mut case, 0, 1, &[])?;
    let nonsquare = rectangle(&mut case, 1, 2, &[one.clone(), zero.clone()])?;
    let square = rectangle(&mut case, 1, 1, std::slice::from_ref(&one))?;
    let offset = rectangle(&mut case, 1, 1, std::slice::from_ref(&zero))?;
    let wrong_offset = rectangle(&mut case, 2, 1, &[zero.clone(), zero])?;
    for (basis, offset) in [
        (empty, empty_offset),
        (nonsquare, offset),
        (square, wrong_offset),
    ] {
        let request = LatticeBasisQuestion {
            context: case.context.clone(),
            basis,
            offset,
        };
        let before = case.cas.counts();
        let fault = case
            .cas
            .admit_lattice_basis(&request, &mut ledger()?)
            .err()
            .ok_or("shape admitted")?;
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ReadingShape));
        assert_eq!(case.cas.counts(), before);
    }
    Ok(())
}

/// Equivalent shears, Integer offset shifts, changed contexts and foreign stores refuse reuse.
/// ´test:integration:lattice-basis-identities´
#[test]
fn lattice_basis_identities() -> TestResult {
    let (mut case, request) = rational_case(1)?;
    let basis = admitted(case.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
    case.cas
        .validate_lattice_basis(&request, &basis, &mut ledger()?)?;
    let one = literal(&mut case, 1)?;
    let two = literal(&mut case, 2)?;
    let three = literal(&mut case, 3)?;
    let shear = rectangle(&mut case, 2, 2, &[one.clone(), two, one.clone(), three])?;
    let shifted = [literal(&mut case, 4)?, literal(&mut case, 6)?];
    let offset = rectangle(&mut case, 2, 1, &shifted)?;
    positive(&mut case, &one, "different-lattice-context")?;
    let mut changed = request.clone();
    changed.basis = shear;
    let mut shifted = request.clone();
    shifted.offset = offset;
    let mut context = request.clone();
    context.context.clone_from(&case.context);
    for different in [changed, shifted, context] {
        let fault = case
            .cas
            .validate_lattice_basis(&different, &basis, &mut ledger()?)
            .err()
            .ok_or("changed identity accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
    }
    let (foreign, _) = rational_case(1)?;
    let fault = foreign
        .cas
        .validate_lattice_basis(&request, &basis, &mut ledger()?)
        .err()
        .ok_or("foreign admission accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    Ok(())
}

/// Fresh replay reconstructs both inverse orders and determinant against independently built claims.
/// ´test:integration:lattice-basis-replay´
#[test]
fn lattice_basis_replay() -> TestResult {
    let (mut source, request) = rational_case(1)?;
    let basis = admitted(source.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
    let bytes = [
        source
            .cas
            .export_witness(basis.inverse().nonsingularity(), &mut ledger()?)?,
        source
            .cas
            .export_witness(basis.inverse().witness(), &mut ledger()?)?,
        source
            .cas
            .export_witness(basis.left_inverse(), &mut ledger()?)?,
    ];
    let mut receiver = Fixture::new(&[])?;
    let forward = constant_map(&mut receiver, 2, &["1", "1", "1", "2"], &["3", "5"])?;
    let inverse = constant_map(&mut receiver, 2, &["2", "-1", "-1", "1"], &["-1", "-2"])?;
    let determinant = literal(&mut receiver, 1)?;
    let claims = [
        determinant_expected(&mut receiver, &forward, &determinant)?,
        expected(&mut receiver, &forward, &inverse)?,
        expected(&mut receiver, &inverse, &forward)?,
    ];
    for (bytes, claim) in bytes.iter().zip(&claims) {
        let replayed = receiver.cas.replay(bytes, claim, &mut ledger()?)?;
        assert_eq!(replayed.judgment(), claim);
        assert_eq!(
            receiver.cas.export_witness(&replayed, &mut ledger()?)?,
            *bytes
        );
    }
    let shifted = constant_map(&mut receiver, 2, &["1", "1", "1", "2"], &["4", "6"])?;
    let shifted_inverse = constant_map(&mut receiver, 2, &["2", "-1", "-1", "1"], &["-2", "-2"])?;
    let shifted_claim = expected(&mut receiver, &shifted, &shifted_inverse)?;
    let sheared = constant_map(&mut receiver, 2, &["1", "2", "1", "3"], &["3", "5"])?;
    let sheared_inverse = constant_map(&mut receiver, 2, &["3", "-2", "-1", "1"], &["1", "-2"])?;
    let sheared_claim = expected(&mut receiver, &sheared, &sheared_inverse)?;
    positive(&mut receiver, &determinant, "changed-replay-context")?;
    let changed_context = expected(&mut receiver, &forward, &inverse)?;
    for wrong in [shifted_claim, sheared_claim, changed_context] {
        let before = receiver.cas.counts();
        let fault = receiver
            .cas
            .replay(&bytes[1], &wrong, &mut ledger()?)
            .err()
            .ok_or("changed expected question accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(receiver.cas.counts(), before);
    }
    Ok(())
}

/// Literal default-budget Work and witness sizes pin each admitted basis fixture.
/// ´test:integration:lattice-basis-costs´
#[test]
fn lattice_basis_costs() -> TestResult {
    let mut measured = Vec::new();
    for kind in [1, 2, 3] {
        let (mut case, request) = if kind == 3 {
            phase_case()?
        } else {
            rational_case(kind)?
        };
        let before = case.cas.counts();
        let mut budget = ledger()?;
        let basis = admitted(case.cas.admit_lattice_basis(&request, &mut budget)?)?;
        measured.push((
            kind,
            budget.usage().work,
            basis.inverse().nonsingularity().exported_byte_len(),
            basis.inverse().witness().exported_byte_len(),
            basis.left_inverse().exported_byte_len(),
            case.cas.counts().payload_bytes - before.payload_bytes,
        ));
    }
    assert_eq!(
        measured,
        [
            (1, 478_183, 721, 997, 997, 20_963),
            (2, 493_501, 821, 1_147, 1_147, 25_052),
            (3, 281_469, 688, 971, 971, 13_405),
        ]
    );
    Ok(())
}

fn bounded(kind: BudgetKind, amount: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::MatrixEntries => limits.matrix_entries = amount,
        BudgetKind::ComponentBits => limits.component_bits = amount,
        BudgetKind::WitnessBytes => limits.witness_bytes = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::PayloadBytes => limits.payload_bytes = amount,
        _ => return Err("unsupported lattice budget fixture".into()),
    }
    Ok(Budget::new(limits)?)
}

fn resource_attempt(
    fixture_kind: u32,
    owner: BudgetKind,
    amount: u64,
    prior: u64,
    retained: u64,
) -> TestResult<bool> {
    let (mut case, request) = if fixture_kind == 3 {
        phase_case()?
    } else {
        rational_case(fixture_kind)?
    };
    let before = case.cas.counts();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = bounded(owner, amount)?;
    let limits = budget.limits().clone();
    budget.work(prior, nrz_cas::Location::new("earlier lattice work"))?;
    let result = budget.with_retained_payload(
        retained,
        nrz_cas::Location::new("retained lattice caller data"),
        |budget| case.cas.admit_lattice_basis(&request, budget),
    );
    assert!(budget.usage().work >= prior);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(LatticeBasisOutcome::Admitted(_)) => Ok(true),
        Ok(LatticeBasisOutcome::Unresolved) => Err("resource fixture unresolved".into()),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == owner));
            assert_eq!(case.cas.counts(), before);
            assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
            Ok(false)
        }
    }
}

/// Each admission fixture has exact N/N−1 Work twins and retains cumulative prior usage.
/// ´test:integration:lattice-basis-work´
#[test]
fn lattice_basis_work() -> TestResult {
    for (kind, needed) in [(1, 478_183), (2, 493_501), (3, 281_469)] {
        assert!(resource_attempt(kind, BudgetKind::Work, needed, 0, 0)?);
        assert!(!resource_attempt(kind, BudgetKind::Work, needed - 1, 0, 0)?);
        assert!(!resource_attempt(kind, BudgetKind::Work, needed, 37, 0)?);
        assert!(resource_attempt(
            kind,
            BudgetKind::Work,
            needed + 37,
            37,
            0
        )?);
    }
    Ok(())
}

/// Dimension, exact entry size and all proof/output storage owners refuse atomically at N−1.
/// ´test:integration:lattice-basis-capacities´
#[test]
fn lattice_basis_capacities() -> TestResult {
    for (kind, needed) in [
        (BudgetKind::MatrixEntries, 9),
        (BudgetKind::ComponentBits, 3),
        (BudgetKind::WitnessBytes, 2_715),
        (BudgetKind::OutputBytes, 3_227),
        (BudgetKind::StagedPayloadBytes, 83_379),
        (BudgetKind::PayloadBytes, 26_032),
    ] {
        assert!(resource_attempt(1, kind, needed, 0, 0)?);
        assert!(!resource_attempt(1, kind, needed - 1, 0, 0)?);
    }
    assert!(resource_attempt(
        1,
        BudgetKind::StagedPayloadBytes,
        83_636,
        37,
        257
    )?);
    assert!(!resource_attempt(
        1,
        BudgetKind::StagedPayloadBytes,
        83_635,
        37,
        257
    )?);
    Ok(())
}

/// Local identity authentication costs seven Work and preserves all evidence at the boundary.
/// ´test:integration:lattice-basis-validation-work´
#[test]
fn lattice_basis_validation_work() -> TestResult {
    let (mut case, request) = rational_case(1)?;
    let basis = admitted(case.cas.admit_lattice_basis(&request, &mut ledger()?)?)?;
    let before = case.cas.counts();
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    for (allowance, prior, succeeds) in [(7, 0, true), (6, 0, false), (7, 1, false), (8, 1, true)] {
        let mut budget = bounded(BudgetKind::Work, allowance)?;
        budget.work(prior, nrz_cas::Location::new("earlier validation work"))?;
        let result = case
            .cas
            .validate_lattice_basis(&request, &basis, &mut budget);
        assert_eq!(result.is_ok(), succeeds);
        assert!(budget.usage().work >= prior);
        if succeeds {
            assert_eq!(budget.usage().work, 7 + prior);
        } else {
            let fault = result.err().ok_or("missing validation refusal")?;
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
        }
        assert_eq!(case.cas.counts(), before);
        assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}
