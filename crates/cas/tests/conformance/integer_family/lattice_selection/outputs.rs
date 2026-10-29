// SPDX-License-Identifier: AGPL-3.0-only
//! Exact representatives and separate capacities preserve the selected tuple.

use super::{Selection, TestResult, column, ledger, number, prepared, real, selected};
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::lattice::{LatticeBasisOutcome, LatticeEnclosure};
use nrz_cas::limits::Limits;
use nrz_cas::proof::Statement;
use nrz_cas::syntax::{AtomicSort, Constant, Operator, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Rational};

fn representative(source: &mut Selection, kind: u32) -> TestResult<Handle> {
    let mut entries = Vec::new();
    if kind == 3 {
        entries.push(number(&mut source.source, 0, 1, false)?);
    } else if kind == 5 || kind == 6 {
        for (numerator, denominator) in if kind == 5 {
            [(1, 20), (1, 80)]
        } else {
            [(7, 40), (3, 40)]
        } {
            entries.push(number(&mut source.source, numerator, denominator, false)?);
        }
    } else if kind == 4 {
        let original = source
            .source
            .frame
            .cas
            .operands(&source.expected.target, &mut ledger()?)?[0]
            .clone();
        let three = number(&mut source.source, 3, 1, false)?;
        entries.push(
            source
                .source
                .frame
                .term(Operator::Sub, real(), &[original, three])?,
        );
    } else {
        let pi = source
            .source
            .frame
            .term(Operator::Constant(Constant::Pi), real(), &[])?;
        if kind == 2 {
            let two = number(&mut source.source, 2, 1, false)?;
            entries.push(
                source
                    .source
                    .frame
                    .term(Operator::Div, real(), &[pi, two])?,
            );
        } else {
            for (original, turns) in [(20, 6), (10, 2)] {
                let factor = number(&mut source.source, turns, 1, false)?;
                let period =
                    source
                        .source
                        .frame
                        .term(Operator::Mul, real(), &[factor, pi.clone()])?;
                let original = number(&mut source.source, original, 1, false)?;
                entries.push(source.source.frame.term(
                    Operator::Sub,
                    real(),
                    &[original, period],
                )?);
            }
        }
    }
    column(&mut source.source, &entries)
}

/// Torus, full-turn phase, LC, grid and translated outputs have exact checked representatives.
/// ´test:integration:lattice-exact-outputs´
#[test]
fn lattice_exact_outputs() -> TestResult {
    for kind in [1, 2, 3, 4, 5, 6] {
        let mut source = if kind == 6 {
            translated()?
        } else if kind == 3 {
            super::direct::direct(kind)?
        } else {
            prepared(kind)?
        };
        let witness = selected(&mut source, &mut ledger()?)?;
        let outputs = source
            .source
            .frame
            .cas
            .lattice_selection_outputs(&witness, &mut ledger()?)?;
        assert_eq!(outputs.tuple(), source.expected.values);
        assert_eq!(outputs.question(), &source.expected);
        assert_eq!(outputs.selection().proof(), witness.proof());
        let Statement::Equality { left, .. } = &outputs.identity().judgment().statement else {
            return Err("missing reconstruction identity".into());
        };
        assert_eq!(left, &source.expected.target);
        assert!(outputs.identity().is_discharged());
        let expected = representative(&mut source, kind)?;
        let identity = source.source.frame.cas.prove_reading_equality(
            &source.expected.cover.index.basis.context,
            outputs.representative(),
            &expected,
            &mut ledger()?,
        )?;
        assert!(identity.is_discharged());
        let bytes = source
            .source
            .frame
            .cas
            .export_witness(outputs.identity(), &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
        let selection = source
            .source
            .frame
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        let replayed = receiver.inspect(&selection, &mut ledger()?)?;
        super::citations(&receiver, &replayed, matches!(kind, 1 | 2 | 4))?;
    }
    Ok(())
}

fn translated() -> TestResult<Selection> {
    let mut source = prepared(5)?;
    let offsets = [
        number(&mut source.source, 1, 10, false)?,
        number(&mut source.source, 1, 20, false)?,
    ];
    source.source.question.basis.offset = column(&mut source.source, &offsets)?;
    let LatticeBasisOutcome::Admitted(basis) = source
        .source
        .frame
        .cas
        .admit_lattice_basis(&source.source.question.basis, &mut ledger()?)?
    else {
        return Err("translated grid not admitted".into());
    };
    source.source.basis = basis;
    source.cover = super::prove(&mut source.source, &mut ledger()?)?;
    let Statement::LatticeCover(cover) = &source.cover.judgment().statement else {
        return Err("missing translated cover".into());
    };
    source.expected.cover = *cover.clone();
    source.expected.values = vec![
        number(&mut source.source, 1, 1, true)?,
        number(&mut source.source, 2, 1, true)?,
    ];
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing original box".into());
    };
    enclosure.basis = source.source.question.basis.clone();
    enclosure.inverse = source.source.basis.inverse().map().expression().clone();
    let entries = source
        .source
        .frame
        .cas
        .operands(&enclosure.inverse, &mut ledger()?)?;
    enclosure.coefficients.clear();
    for (target, (numerator, denominator)) in
        entries
            .into_iter()
            .zip([(8, 1), (0, 1), (-4, 5), (0, 1), (16, 1), (-4, 5)])
    {
        let value = Rational::from_ratio(numerator, denominator, &mut ledger()?)?;
        enclosure.coefficients.push(RationalEnclosure {
            target: target.clone(),
            context: enclosure.original.context.clone(),
            input_family: enclosure.original.input_family.clone(),
            lower: value.clone(),
            upper: value.clone(),
            origin: EnclosureOrigin::ExactRational {
                expression: target,
                value,
            },
        });
    }
    enclosure.bounds = [8, 12]
        .into_iter()
        .map(|numerator| {
            let value = Rational::from_ratio(numerator, 5, &mut ledger()?)?;
            Ok((value.clone(), value))
        })
        .collect::<TestResult<_>>()?;
    Ok(source)
}

/// First-coordinate capacity refusal leaves the exact grid tuple and proof bytes unchanged.
/// ´test:integration:lattice-output-capacity´
#[test]
fn lattice_output_capacity() -> TestResult {
    let mut source = prepared(5)?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let cas = &source.source.frame.cas;
    let before = cas.counts();
    let bytes = cas.export_witness(&witness, &mut ledger()?)?;
    let fault = cas
        .materialize_lattice_count(&witness, 0, 1, &mut ledger()?)
        .err()
        .ok_or("first-coordinate capacity exceeded")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
    );
    assert_eq!(fault.location.operation, "Integer materialization capacity");
    assert_eq!(
        cas.materialize_lattice_count(&witness, 0, 2, &mut ledger()?)?,
        2
    );
    assert_eq!(
        cas.materialize_lattice_count(&witness, 1, 3, &mut ledger()?)?,
        3
    );
    let fault = cas
        .materialize_lattice_count(&witness, 2, 10, &mut ledger()?)
        .err()
        .ok_or("missing coordinate materialized")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::SelectionOutOfRange)
    );
    assert_eq!(fault.location.operation, "lattice exact outputs");
    assert_eq!(cas.counts(), before);
    assert_eq!(cas.export_witness(&witness, &mut ledger()?)?, bytes);
    Ok(())
}

fn grid_value(target: &str, selected: &str) -> TestResult<Selection> {
    let mut source = prepared(5)?;
    let value = Rational::parse(target, &mut ledger()?)?;
    let target = source
        .source
        .frame
        .term(Operator::Rational(value.clone()), real(), &[])?;
    let candidate = Rational::parse(selected, &mut ledger()?)?;
    source.expected.values[0] = source.source.frame.term(
        Operator::Rational(candidate),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing original grid".into());
    };
    let coordinate = &mut enclosure.original.coordinates[0];
    coordinate.target = target.clone();
    coordinate.lower = value.clone();
    coordinate.upper = value.clone();
    coordinate.origin = EnclosureOrigin::ExactRational {
        expression: target.clone(),
        value: value.clone(),
    };
    let transformed = value.mul(&Rational::from_ratio(8, 1, &mut ledger()?)?, &mut ledger()?)?;
    enclosure.bounds[0] = (transformed.clone(), transformed);
    let entries = [target, enclosure.original.coordinates[1].target.clone()];
    let target = column(&mut source.source, &entries)?;
    enclosure.original.target = target.clone();
    source.expected.target = target;
    Ok(source)
}

/// Signed and nonrepresentable exact Integers retain selection across separate conversion refusals.
/// ´test:integration:lattice-output-signed-capacity´
#[test]
fn lattice_output_signed_capacity() -> TestResult {
    for (target, candidate, negative) in [
        ("-3/10", "-3", true),
        ("2305843009213693952", "18446744073709551616", false),
    ] {
        let mut source = grid_value(target, candidate)?;
        let witness = selected(&mut source, &mut ledger()?)?;
        let outputs = source
            .source
            .frame
            .cas
            .lattice_selection_outputs(&witness, &mut ledger()?)?;
        assert_eq!(outputs.tuple(), source.expected.values);
        assert!(outputs.identity().is_discharged());
        let cas = &source.source.frame.cas;
        let before = cas.counts();
        let bytes = cas.export_witness(&witness, &mut ledger()?)?;
        let fault = cas
            .materialize_lattice_count(&witness, 0, u64::MAX, &mut ledger()?)
            .err()
            .ok_or("invalid count materialized")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
        );
        assert_eq!(fault.location.operation, "Integer materialization capacity");
        if negative {
            assert_eq!(
                cas.materialize_lattice_index(&witness, 0, (-3, 0), &mut ledger()?)?,
                -3
            );
        }
        let fault = cas
            .materialize_lattice_index(&witness, 0, (-2, i64::MAX), &mut ledger()?)
            .err()
            .ok_or("signed capacity exceeded")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
        );
        assert_eq!(fault.location.operation, "lattice exact outputs");
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.export_witness(&witness, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// Borrowed evidence costs exactly four Work and preserves the complete original tuple contract.
/// ´test:integration:lattice-output-evidence-work´
#[test]
fn lattice_output_evidence_work() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind};
    let mut source = prepared(5)?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let cas = &source.source.frame.cas;
    let before = cas.counts();
    for succeeds in [false, true] {
        let limits = Limits {
            work: 40 + u64::from(succeeds),
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        budget.work(37, nrz_cas::Location::new("prior lattice evidence Work"))?;
        let result = cas.lattice_selection_evidence(&witness, &mut budget);
        if succeeds {
            let (question, enclosure) = result?;
            assert_eq!(question, &source.expected);
            assert_eq!(enclosure, &source.enclosure);
            assert_eq!(budget.usage().work, 41);
        } else {
            let fault = result.err().ok_or("evidence exceeded Work")?;
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
            assert_eq!(budget.usage().work, 37);
        }
        assert_eq!(budget.limits(), &limits);
        assert_eq!(cas.counts(), before);
    }
    Ok(())
}

fn unselected(
    cas: &mut Cas,
    witness: &nrz_cas::proof::VerifiedWitness,
    code: InvalidCode,
    owner: &str,
) -> TestResult {
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let fault = cas
        .lattice_selection_outputs(witness, &mut ledger()?)
        .err()
        .ok_or("unselected outputs constructed")?;
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    assert_eq!(fault.location.operation, owner);
    assert_eq!(cas.counts(), before);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    Ok(())
}

/// Other proof roots and foreign selections cannot produce an exact output descriptor.
/// ´test:integration:lattice-output-refusals´
#[test]
fn lattice_output_refusals() -> TestResult {
    let mut source = prepared(5)?;
    unselected(
        &mut source.source.frame.cas,
        &source.cover,
        InvalidCode::EndpointMismatch,
        "lattice exact outputs",
    )?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let mut foreign = Cas::new(Limits::default())?;
    unselected(
        &mut foreign,
        &witness,
        InvalidCode::ForeignHandle,
        "handle lookup",
    )?;
    let fault = source
        .source
        .frame
        .cas
        .materialize_lattice_index(&witness, 0, (1, -1), &mut ledger()?)
        .err()
        .ok_or("reversed capacity admitted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Policy));
    assert_eq!(fault.location.operation, "lattice exact outputs");
    Ok(())
}
