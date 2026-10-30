// SPDX-License-Identifier: AGPL-3.0-only
//! Each nearest owner has a literal monotone Work boundary and atomic N-1 refusal.

use super::{Selection, TestResult, ledger, orthogonal, prepared, record_count};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::{Cas, Fault, FaultKind, Location};

/// Selection, fresh selection replay, full preparation, outputs, fresh identity replay, bytes, records.
const COSTS: [(u64, u64, u64, u64, u64, usize, usize); 3] = [
    (364_787, 905_604, 122_399, 99_607, 161_213, 3_745, 71),
    (361_243, 896_822, 120_735, 96_281, 151_758, 3_700, 70),
    (364_891, 906_028, 122_951, 100_327, 161_916, 3_745, 71),
];

fn fixture(kind: usize) -> TestResult<Selection> {
    match kind {
        0 => prepared(5),
        1 => orthogonal::tie(false),
        _ => orthogonal::tie(true),
    }
}

fn budget(amount: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work: amount,
        ..Limits::default()
    })?;
    budget.work(37, Location::new("prior nearest owner Work"))?;
    Ok(budget)
}

/// Full grid snapshots fit default Work; larger fixtures use the caller's exact count checks.
fn snapshot(cas: &Cas, dimension: usize) -> TestResult<Option<Vec<u8>>> {
    if dimension > 2 {
        return Ok(None);
    }
    Ok(Some(cas.canonical_snapshot(&mut ledger()?)?))
}

fn outcome<T>(
    cas: &Cas,
    result: Result<T, Fault>,
    budget: &Budget,
    succeeds: bool,
    snapshot: Option<&[u8]>,
) -> TestResult<Option<T>> {
    assert!(budget.usage().work > 37);
    if succeeds {
        assert_eq!(budget.usage().work, budget.limits().work);
        Ok(Some(result?))
    } else {
        let fault = result
            .err()
            .ok_or("nearest owner exceeded its Work boundary")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert!(budget.usage().work <= budget.limits().work);
        if let Some(snapshot) = snapshot {
            assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
        Ok(None)
    }
}

pub(super) fn proof(
    source: &mut Selection,
    work: u64,
    succeeds: bool,
) -> TestResult<Option<VerifiedWitness>> {
    let cas = &mut source.source.frame.cas;
    let snapshot = snapshot(cas, source.expected.values.len())?;
    let before = cas.counts();
    let mut budget = budget(work)?;
    let limits = budget.limits().clone();
    let result = cas.prove_lattice_nearest_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut budget,
    );
    assert_eq!(budget.limits(), &limits);
    let result = outcome(cas, result, &budget, succeeds, snapshot.as_deref())?;
    if !succeeds {
        assert_eq!(cas.counts(), before);
    }
    Ok(result)
}

pub(super) fn preparation(source: &mut Selection, work: u64, succeeds: bool) -> TestResult {
    let cas = &mut source.source.frame.cas;
    let snapshot = snapshot(cas, source.expected.values.len())?;
    let before = cas.counts();
    let mut budget = budget(work)?;
    let limits = budget.limits().clone();
    let result = cas.prepare_lattice_nearest_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut budget,
    );
    assert_eq!(budget.limits(), &limits);
    if let Some(attempt) = outcome(cas, result, &budget, succeeds, snapshot.as_deref())? {
        assert_eq!(attempt.spent_work(), work);
    }
    assert_eq!(cas.counts(), before);
    if let Some(snapshot) = snapshot {
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}

pub(super) fn outputs(
    source: &mut Selection,
    witness: &VerifiedWitness,
    work: u64,
    succeeds: bool,
) -> TestResult<Option<Vec<u8>>> {
    let cas = &mut source.source.frame.cas;
    let snapshot = snapshot(cas, source.expected.values.len())?;
    let before = cas.counts();
    let mut budget = budget(work)?;
    let limits = budget.limits().clone();
    let result = cas.lattice_selection_outputs(witness, &mut budget);
    assert_eq!(budget.limits(), &limits);
    let result = outcome(cas, result, &budget, succeeds, snapshot.as_deref())?;
    if !succeeds {
        assert_eq!(cas.counts(), before);
    }
    result
        .map(|output| Ok(cas.export_witness(output.identity(), &mut ledger()?)?))
        .transpose()
}

/// Three fixtures pin proof, fresh replay, full preparation, outputs and fresh identity owners.
/// ´test:integration:lattice-nearest-work´
#[test]
fn lattice_nearest_work() -> TestResult {
    for (kind, (selection, replay, prepare, output, identity, bytes, records)) in
        COSTS.into_iter().enumerate()
    {
        let mut source = fixture(kind)?;
        preparation(&mut source, prepare + 36, false)?;
        preparation(&mut source, prepare + 37, true)?;
        proof(&mut source, selection + 36, false)?;
        let witness = proof(&mut source, selection + 37, true)?.ok_or("missing nearest tuple")?;
        let encoded = source
            .source
            .frame
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        assert_eq!(encoded.len(), bytes);
        assert_eq!(record_count(&encoded)?, records);
        super::super::costs::replay_boundary(&encoded, replay + 36, false)?;
        super::super::costs::replay_boundary(&encoded, replay + 37, true)?;
        outputs(&mut source, &witness, output + 36, false)?;
        let encoded =
            outputs(&mut source, &witness, output + 37, true)?.ok_or("missing nearest outputs")?;
        let (identity_bytes, identity_records) = [(1_389, 29), (1_292, 27), (1_389, 29)][kind];
        assert_eq!(encoded.len(), identity_bytes);
        assert_eq!(record_count(&encoded)?, identity_records);
        super::super::costs::replay_boundary(&encoded, identity + 36, false)?;
        super::super::costs::replay_boundary(&encoded, identity + 37, true)?;
    }
    Ok(())
}
