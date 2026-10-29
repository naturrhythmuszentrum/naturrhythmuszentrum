// SPDX-License-Identifier: AGPL-3.0-only
//! Portable named tuple proofs retain original-target boxes and affine cover dependencies.

#[path = "lattice_selection/nearest.rs"]
mod nearest;

#[path = "lattice_selection/attempt.rs"]
mod attempt;
#[path = "lattice_selection/costs.rs"]
mod costs;
#[path = "lattice_selection/direct.rs"]
mod direct;
#[path = "lattice_selection/output_costs.rs"]
mod output_costs;
#[path = "lattice_selection/outputs.rs"]
mod outputs;

use super::{Product, fixture, ledger, matrix, prove, real};
use crate::testing::common::TestResult;
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::lattice::{
    LatticeBasisOutcome, LatticeBasisQuestion, LatticeBoxEnclosure, LatticeEnclosure,
    LatticeSelection, RationalBoxEnclosure,
};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::{Judgment, Statement, VerifiedWitness};
use nrz_cas::syntax::{AtomicSort, Constant, Operator, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Rational};

struct Selection {
    source: Product,
    cover: VerifiedWitness,
    expected: LatticeSelection,
    enclosure: LatticeEnclosure,
}

fn number(
    source: &mut Product,
    numerator: i64,
    denominator: u64,
    integer: bool,
) -> TestResult<Handle> {
    source.frame.term(
        Operator::Rational(Rational::from_ratio(
            numerator,
            denominator,
            &mut ledger()?,
        )?),
        if integer {
            Sort::Value(AtomicSort::Integer)
        } else {
            real()
        },
        &[],
    )
}

fn column(source: &mut Product, entries: &[Handle]) -> TestResult<Handle> {
    let rows = u32::try_from(entries.len())?;
    source.frame.term(
        Operator::Matrix { rows, columns: 1 },
        matrix(rows, 1),
        entries,
    )
}

fn prepared(kind: u32) -> TestResult<Selection> {
    let one_dimensional = matches!(kind, 2..=4);
    let mut source = fixture(if one_dimensional { 2 } else { 0 }, 0, false, false)?;
    let count = if one_dimensional { 1 } else { 2 };
    let pi = source
        .frame
        .term(Operator::Constant(Constant::Pi), real(), &[])?;
    let two = number(&mut source, 2, 1, false)?;
    let period = source
        .frame
        .term(Operator::Mul, real(), &[two, pi.clone()])?;
    admit_fixture_basis(&mut source, kind, count, &period)?;
    let cover = prove(&mut source, &mut ledger()?)?;
    let Statement::LatticeCover(cover_value) = &cover.judgment().statement else {
        return Err("missing cover".into());
    };
    let cover_value = *cover_value.clone();
    let coordinates = originals(&mut source, kind, &pi, &period)?;
    let entries = coordinates
        .iter()
        .map(|coordinate| coordinate.target.clone())
        .collect::<Vec<_>>();
    let target = column(&mut source, &entries)?;
    let original = RationalBoxEnclosure {
        target: target.clone(),
        context: source.question.basis.context.clone(),
        input_family: source
            .frame
            .cas
            .admissible_region(&source.question.basis.context)?,
        coordinates,
    };
    let inverse = source.basis.inverse().map().expression().clone();
    let coefficients = inverse_coefficients(&source, kind, count, &inverse, &original)?;
    let values = match kind {
        0 => vec![(1827, 640, 861, 256), (1827, 1280, 205, 128)],
        1 => vec![(3857, 1280, 205, 64), (1827, 1280, 205, 128)],
        2 => vec![(609, 512, 41, 32)],
        3 => vec![(609, 640, 533, 512)],
        4 => vec![(203, 64, 205, 64)],
        _ => vec![(12, 5, 12, 5), (16, 5, 16, 5)],
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
    let enclosure = LatticeEnclosure::Original(Box::new(LatticeBoxEnclosure {
        basis: source.question.basis.clone(),
        original,
        inverse,
        coefficients,
        bounds,
    }));
    let selected = match kind {
        0 | 1 => vec![3, 1],
        2 | 3 => vec![1],
        4 => vec![3],
        _ => vec![2, 3],
    };
    let mut values = Vec::new();
    for value in selected {
        values.push(number(&mut source, value, 1, true)?);
    }
    let expected = LatticeSelection {
        cover: cover_value,
        target,
        values,
    };
    Ok(Selection {
        source,
        cover,
        expected,
        enclosure,
    })
}

/// Torus and phase use exact full-turn scales; LC and grid retain their rational scales.
fn admit_fixture_basis(source: &mut Product, kind: u32, count: u32, period: &Handle) -> TestResult {
    let zero = number(source, 0, 1, false)?;
    let scales = if kind == 5 {
        vec![number(source, 1, 8, false)?, number(source, 1, 16, false)?]
    } else if kind == 4 {
        vec![number(source, 1, 1, false)?]
    } else {
        vec![period.clone(); usize::try_from(count)?]
    };
    let basis_entries = if count == 1 {
        scales
    } else {
        vec![
            scales[0].clone(),
            zero.clone(),
            zero.clone(),
            scales[1].clone(),
        ]
    };
    let basis = source.frame.term(
        Operator::Matrix {
            rows: count,
            columns: count,
        },
        matrix(count, count),
        &basis_entries,
    )?;
    let offsets = vec![zero; usize::try_from(count)?];
    let question = LatticeBasisQuestion {
        context: source.question.basis.context.clone(),
        basis,
        offset: column(source, &offsets)?,
    };
    let LatticeBasisOutcome::Admitted(basis) = source
        .frame
        .cas
        .admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("selection basis unresolved".into());
    };
    source.basis = basis;
    source.question.basis = question;
    Ok(())
}

fn inverse_coefficients(
    source: &Product,
    kind: u32,
    count: u32,
    inverse: &Handle,
    original: &RationalBoxEnclosure,
) -> TestResult<Vec<RationalEnclosure>> {
    let inverse_entries = source.frame.cas.operands(inverse, &mut ledger()?)?;
    let mut coefficients = Vec::new();
    let width = usize::try_from(count)? + 1;
    for (position, target) in inverse_entries
        .into_iter()
        .take(usize::try_from(count)? * width)
        .enumerate()
    {
        let diagonal = position / width == position % width;
        let pi_entry = diagonal && kind < 4;
        let (lower, upper, origin) = if pi_entry {
            (
                Rational::from_ratio(203, 1280, &mut ledger()?)?,
                Rational::from_ratio(205, 1280, &mut ledger()?)?,
                EnclosureOrigin::PiBound {
                    expression: target.clone(),
                    certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
                },
            )
        } else {
            let selected = if !diagonal {
                0
            } else if kind == 5 {
                if position == 0 { 8 } else { 16 }
            } else {
                1
            };
            let value = Rational::from_ratio(selected, 1, &mut ledger()?)?;
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
            context: original.context.clone(),
            input_family: original.input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    Ok(coefficients)
}

fn originals(
    source: &mut Product,
    kind: u32,
    pi: &Handle,
    period: &Handle,
) -> TestResult<Vec<RationalEnclosure>> {
    let mut result = Vec::new();
    let values = if kind < 2 {
        vec![(20, 1), (10, 1)]
    } else if kind == 5 {
        vec![(3, 10), (1, 5)]
    } else {
        vec![(0, 1)]
    };
    for (row, (numerator, denominator)) in values.into_iter().enumerate() {
        let (target, rational) = match kind {
            2 => {
                let factor = number(source, 5, 2, false)?;
                (
                    source
                        .frame
                        .term(Operator::Mul, real(), &[factor, pi.clone()])?,
                    None,
                )
            }
            3 => (period.clone(), None),
            4 => {
                let ten = number(source, 10, 1, false)?;
                (
                    source
                        .frame
                        .term(Operator::Div, real(), &[ten, pi.clone()])?,
                    None,
                )
            }
            _ => (
                number(source, numerator, denominator, false)?,
                Some(Rational::from_ratio(
                    numerator,
                    denominator,
                    &mut ledger()?,
                )?),
            ),
        };
        let bounds = match kind {
            0 => {
                if row == 0 {
                    (18, 1, 21, 1)
                } else {
                    (9, 1, 10, 1)
                }
            }
            1 => {
                if row == 0 {
                    (19, 1, 20, 1)
                } else {
                    (9, 1, 10, 1)
                }
            }
            2 => (15, 2, 8, 1),
            3 => (6, 1, 13, 2),
            4 => (203, 64, 205, 64),
            _ => (numerator, denominator, numerator, denominator),
        };
        let origin = if let Some(value) = rational {
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value,
            }
        } else {
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
            }
        };
        result.push(RationalEnclosure {
            target,
            context: source.question.basis.context.clone(),
            input_family: source
                .frame
                .cas
                .admissible_region(&source.question.basis.context)?,
            lower: Rational::from_ratio(bounds.0, bounds.1, &mut ledger()?)?,
            upper: Rational::from_ratio(bounds.2, bounds.3, &mut ledger()?)?,
            origin,
        });
    }
    Ok(result)
}

fn selected(source: &mut Selection, budget: &mut Budget) -> TestResult<VerifiedWitness> {
    Ok(source.source.frame.cas.prove_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        budget,
    )?)
}

/// The narrow torus, phase, LC and grid select exact complete tuples and retain their proof closure.
/// ´test:integration:lattice-tuple-selection´
#[test]
fn lattice_tuple_selection() -> TestResult {
    for kind in [1, 2, 4, 5] {
        let mut source = prepared(kind)?;
        let proof = selected(&mut source, &mut ledger()?)?;
        assert_eq!(
            proof.judgment().statement,
            Statement::LatticeSelection(Box::new(source.expected.clone()))
        );
        assert!(proof.is_discharged());
        assert_eq!(
            proof.hypotheses().len(),
            if kind == 2 || kind == 4 { 2 } else { 0 }
        );
        let bytes = source
            .source
            .frame
            .cas
            .export_witness(&proof, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(replayed.hypotheses().len(), proof.hypotheses().len());
        citations(&receiver, &replayed, kind != 5)?;
        assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// Read the retained checker schema and the actual certificate's Machin dependency.
fn citations(cas: &Cas, witness: &VerifiedWitness, pi: bool) -> TestResult {
    let citations = cas.proof_citations(witness.proof(), &mut ledger()?)?;
    let citation = citations.last().ok_or("missing tuple citation")?;
    let schema = citation
        .auxiliary_schema(cas)?
        .ok_or("missing tuple schema")?;
    assert_eq!(
        (schema.name, schema.version),
        ("decision.lattice-floor-selection", 1)
    );
    assert_eq!(citation.premises.len(), 1);
    let constant = citation.constant_schema(cas, &mut ledger()?)?;
    assert_eq!(
        constant.map(|value| (value.name, value.version)),
        pi.then_some(("constant.pi-machin-bounds", 1))
    );
    Ok(())
}

/// Whole-box torus and wrap straddles cannot publish a partial tuple.
/// ´test:integration:lattice-tuple-straddles´
#[test]
fn lattice_tuple_straddles() -> TestResult {
    for kind in [0, 3] {
        let mut source = prepared(kind)?;
        let before = source.source.frame.cas.counts();
        let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
        let fault = source
            .source
            .frame
            .cas
            .prove_lattice_selection(
                &source.cover,
                &source.expected,
                &source.enclosure,
                &mut ledger()?,
            )
            .err()
            .ok_or("straddling tuple selected")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionBoundaryIntersection)
        );
        assert_eq!(fault.location.operation, "lattice tuple selection");
        assert_eq!(source.source.frame.cas.counts(), before);
        assert_eq!(
            source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
    }
    Ok(())
}

/// Independent expected-claim replay matches the original target as well as the same tuple.
/// ´test:integration:lattice-tuple-expected-target´
#[test]
fn lattice_tuple_expected_target() -> TestResult {
    let mut source = prepared(1)?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let bytes = source
        .source
        .frame
        .cas
        .export_witness(&witness, &mut ledger()?)?;
    let mut receiver = prepared(1)?;
    let mut expected = Judgment {
        context: receiver.expected.cover.index.basis.context.clone(),
        layer: witness.judgment().layer,
        space: None,
        residuals: Vec::new(),
        statement: Statement::LatticeSelection(Box::new(receiver.expected.clone())),
    };
    let replayed = receiver
        .source
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(replayed.judgment(), &expected);
    let entries = [
        number(&mut receiver.source, 21, 1, false)?,
        number(&mut receiver.source, 10, 1, false)?,
    ];
    let replacement = column(&mut receiver.source, &entries)?;
    let Statement::LatticeSelection(selection) = &mut expected.statement else {
        return Err("missing expected selection".into());
    };
    selection.target = replacement;
    let before = receiver.source.frame.cas.counts();
    let snapshot = receiver
        .source
        .frame
        .cas
        .canonical_snapshot(&mut ledger()?)?;
    let fault = receiver
        .source
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("changed original selection target replayed")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "typed proof admission");
    assert_eq!(receiver.source.frame.cas.counts(), before);
    assert_eq!(
        receiver
            .source
            .frame
            .cas
            .canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}

fn refuses(source: &mut Selection, expected: InvalidCode, at: &str) -> TestResult {
    let before = source.source.frame.cas.counts();
    let snapshot = source.source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = source
        .source
        .frame
        .cas
        .prove_lattice_selection(
            &source.cover,
            &source.expected,
            &source.enclosure,
            &mut budget,
        )
        .err()
        .ok_or("false named tuple accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fault.location.operation, at);
    assert_eq!(source.source.frame.cas.counts(), before);
    assert_eq!(
        source.source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    assert!(budget.usage().work > 0);
    Ok(())
}

/// Candidate arity, Integer sort, original target and complete parameter range remain exact.
/// ´test:integration:lattice-tuple-refusals´
#[test]
fn lattice_tuple_refusals() -> TestResult {
    let mut source = prepared(1)?;
    source.expected.values.pop();
    refuses(
        &mut source,
        InvalidCode::EndpointMismatch,
        "lattice tuple selection",
    )?;
    let mut source = prepared(1)?;
    source.expected.values[0] = number(&mut source.source, 3, 1, false)?;
    refuses(
        &mut source,
        InvalidCode::EndpointMismatch,
        "lattice tuple selection",
    )?;
    let mut source = prepared(1)?;
    let entries = [
        number(&mut source.source, 21, 1, false)?,
        number(&mut source.source, 10, 1, false)?,
    ];
    source.expected.target = column(&mut source.source, &entries)?;
    refuses(
        &mut source,
        InvalidCode::EndpointMismatch,
        "lattice tuple selection",
    )?;
    let mut source = prepared(2)?;
    source.expected.values[0] = number(&mut source.source, 6, 1, true)?;
    refuses(
        &mut source,
        InvalidCode::DecisionIntegerRange,
        "integer floor family",
    )
}
