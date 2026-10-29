// SPDX-License-Identifier: AGPL-3.0-only
//! Rational skew and orthogonal fixtures retain the original target and index order.

use super::{
    EnclosureOrigin, LatticeBasisOutcome, LatticeBasisQuestion, LatticeBoxEnclosure,
    LatticeEnclosure, LatticeSelection, Operator, Rational, RationalBoxEnclosure,
    RationalEnclosure, Selection, TestResult, column, fixture, ledger, matrix, number, prepared,
    prove,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{FaultKind, Handle, Location};

fn exact(source: &Selection, target: Handle, ratio: (i64, u64)) -> TestResult<RationalEnclosure> {
    let value = Rational::from_ratio(ratio.0, ratio.1, &mut ledger()?)?;
    Ok(RationalEnclosure {
        target: target.clone(),
        context: source.expected.cover.index.basis.context.clone(),
        input_family: source
            .source
            .frame
            .cas
            .admissible_region(&source.expected.cover.index.basis.context)?,
        lower: value.clone(),
        upper: value.clone(),
        origin: EnclosureOrigin::ExactRational {
            expression: target,
            value,
        },
    })
}

fn skew() -> TestResult<Selection> {
    let mut source = fixture(3, 0, false, false)?;
    let mut entries = Vec::new();
    for value in [1, 1, 0, 0, 1, 0, 0, 0, 1] {
        entries.push(number(&mut source, value, 1, false)?);
    }
    let question = LatticeBasisQuestion {
        context: source.question.basis.context.clone(),
        basis: source.frame.term(
            Operator::Matrix {
                rows: 3,
                columns: 3,
            },
            matrix(3, 3),
            &entries,
        )?,
        offset: source.question.basis.offset.clone(),
    };
    let LatticeBasisOutcome::Admitted(basis) = source
        .frame
        .cas
        .admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("skew basis unresolved".into());
    };
    source.basis = basis;
    source.question.basis = question;
    let cover = prove(&mut source, &mut ledger()?)?;
    let nrz_cas::proof::Statement::LatticeCover(value) = &cover.judgment().statement else {
        return Err("missing skew cover".into());
    };
    let cover_value = *value.clone();
    let mut targets = Vec::new();
    for (n, d) in [(3, 5), (2, 5), (1, 10)] {
        targets.push(number(&mut source, n, d, false)?);
    }
    let target = column(&mut source, &targets)?;
    let zero = number(&mut source, 0, 1, true)?;
    let expected = LatticeSelection {
        cover: cover_value,
        target,
        values: vec![zero; 3],
    };
    let original = RationalBoxEnclosure {
        target: expected.target.clone(),
        context: expected.cover.index.basis.context.clone(),
        input_family: source
            .frame
            .cas
            .admissible_region(&expected.cover.index.basis.context)?,
        coordinates: Vec::new(),
    };
    let enclosure = LatticeEnclosure::Original(Box::new(LatticeBoxEnclosure {
        basis: source.question.basis.clone(),
        inverse: source.basis.inverse().map().expression().clone(),
        original,
        coefficients: Vec::new(),
        bounds: Vec::new(),
    }));
    let mut result = Selection {
        source,
        cover,
        expected,
        enclosure,
    };
    skew_enclosure(&mut result, &targets)?;
    Ok(result)
}

fn skew_enclosure(source: &mut Selection, targets: &[Handle]) -> TestResult {
    let mut coordinates = Vec::new();
    for (target, ratio) in targets.iter().zip([(3, 5), (2, 5), (1, 10)]) {
        coordinates.push(exact(source, target.clone(), ratio)?);
    }
    let entries = source
        .source
        .frame
        .cas
        .operands(&source.expected.cover.inverse, &mut ledger()?)?;
    let mut coefficients = Vec::new();
    for (target, value) in entries
        .into_iter()
        .zip([1, -1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0])
    {
        coefficients.push(exact(source, target, (value, 1))?);
    }
    let mut bounds = Vec::new();
    for (n, d) in [(1, 5), (2, 5), (1, 10)] {
        let value = Rational::from_ratio(n, d, &mut ledger()?)?;
        bounds.push((value.clone(), value));
    }
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing skew enclosure".into());
    };
    enclosure.original.coordinates = coordinates;
    enclosure.coefficients = coefficients;
    enclosure.bounds = bounds;
    Ok(())
}

/// Floor baseline pins both owners without adding nearest checks to either convention's setup.
/// ´test:integration:lattice-nearest-floor-work´
#[test]
fn lattice_nearest_floor_work() -> TestResult {
    for (mut source, proof, replay, bytes, records) in [
        (prepared(5)?, 335_639, 876_456, 3_745, 71),
        (skew()?, 502_480, 1_428_761, 4_638, 82),
    ] {
        baseline_proof(&mut source, proof + 36, false)?;
        let witness =
            baseline_proof(&mut source, proof + 37, true)?.ok_or("missing floor baseline")?;
        assert_eq!(witness.len(), bytes);
        assert_eq!(record_count(&witness)?, records);
        super::costs::replay_boundary(&witness, replay + 36, false)?;
        super::costs::replay_boundary(&witness, replay + 37, true)?;
    }
    Ok(())
}

/// The three-dimensional fixture's unrelated setup makes a full snapshot exceed default Work.
/// Keep full snapshots for grid; skew checks exact graph counts and retained cover bytes.
fn baseline_proof(
    source: &mut Selection,
    amount: u64,
    succeeds: bool,
) -> TestResult<Option<Vec<u8>>> {
    if source.expected.values.len() == 2 {
        return super::costs::proof_boundary(source, amount, succeeds);
    }
    let cas = &mut source.source.frame.cas;
    let before = cas.counts();
    let cover = cas.export_witness(&source.cover, &mut ledger()?)?;
    let mut budget = Budget::new(Limits {
        work: amount,
        ..Limits::default()
    })?;
    let limits = budget.limits().clone();
    budget.work(37, Location::new("prior skew floor Work"))?;
    let result = cas.prove_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut budget,
    );
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > 37);
    if succeeds {
        let witness = result?;
        assert_eq!(budget.usage().work, amount);
        Ok(Some(cas.export_witness(&witness, &mut ledger()?)?))
    } else {
        let fault = result
            .err()
            .ok_or("skew floor exceeded its Work boundary")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.export_witness(&source.cover, &mut ledger()?)?, cover);
        Ok(None)
    }
}

/// Read only the fixed witness envelope's declared record count.
fn record_count(bytes: &[u8]) -> TestResult<usize> {
    assert_eq!(bytes.get(..8), Some(b"NRZCAS05".as_slice()));
    let mut offset = 8;
    let manifest = index(bytes, &mut offset)?;
    for _ in 0..manifest {
        let length = usize::try_from(index(bytes, &mut offset)?)?;
        offset = offset.checked_add(length).ok_or("manifest length")?;
        index(bytes, &mut offset)?;
    }
    Ok(usize::try_from(index(bytes, &mut offset)?)?)
}

fn index(bytes: &[u8], offset: &mut usize) -> TestResult<u32> {
    let end = offset.checked_add(4).ok_or("index end")?;
    let value = u32::from_le_bytes(bytes.get(*offset..end).ok_or("index bytes")?.try_into()?);
    *offset = end;
    Ok(value)
}
