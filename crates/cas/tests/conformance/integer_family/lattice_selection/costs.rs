// SPDX-License-Identifier: AGPL-3.0-only
//! Literal proof and fresh inspection boundaries share the caller's monotone Work ledger.

use super::{Selection, TestResult, ledger, prepared};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::{Cas, Fault, FaultKind, Location};

/// Kind, proof Work, fresh inspection Work and portable witness bytes.
const COSTS: [(u32, u64, u64, usize); 4] = [
    (1, 404_211, 1_071_050, 4_004),
    (2, 381_163, 961_180, 4_120),
    (4, 313_199, 758_686, 3_575),
    (5, 335_639, 876_456, 3_745),
];

fn budget(amount: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work: amount,
        ..Limits::default()
    })?;
    budget.work(37, Location::new("prior lattice tuple Work"))?;
    Ok(budget)
}

/// A final refusal preserves counts and canonical graph bytes, including failed publication.
fn outcome(
    cas: &Cas,
    result: Result<VerifiedWitness, Fault>,
    budget: &Budget,
    succeeds: bool,
    snapshot: &[u8],
) -> TestResult<Option<VerifiedWitness>> {
    assert!(budget.usage().work > 37);
    if succeeds {
        assert_eq!(budget.usage().work, budget.limits().work);
        Ok(Some(result?))
    } else {
        let fault = result
            .err()
            .ok_or("tuple operation exceeded its Work boundary")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert!(budget.usage().work <= budget.limits().work);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        Ok(None)
    }
}

pub(super) fn proof_boundary(
    source: &mut Selection,
    amount: u64,
    succeeds: bool,
) -> TestResult<Option<Vec<u8>>> {
    let cas = &mut source.source.frame.cas;
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = budget(amount)?;
    let limits = budget.limits().clone();
    let result = cas.prove_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut budget,
    );
    assert_eq!(budget.limits(), &limits);
    let witness = outcome(cas, result, &budget, succeeds, &snapshot)?;
    if !succeeds {
        assert_eq!(cas.counts(), before);
    }
    witness
        .map(|witness| Ok(cas.export_witness(&witness, &mut ledger()?)?))
        .transpose()
}

pub(super) fn replay_boundary(bytes: &[u8], amount: u64, succeeds: bool) -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = budget(amount)?;
    let limits = budget.limits().clone();
    let result = cas.inspect(bytes, &mut budget);
    assert_eq!(budget.limits(), &limits);
    if let Some(witness) = outcome(&cas, result, &budget, succeeds, &snapshot)? {
        assert_eq!(cas.export_witness(&witness, &mut ledger()?)?, bytes);
    } else {
        assert_eq!(cas.counts(), before);
    }
    Ok(())
}

/// Proof and fresh replay each succeed at N and refuse atomically at N−1 after 37 prior Work.
/// ´test:integration:lattice-tuple-work´
/// ´claim:cas:lattice-tuple-work´
#[test]
fn lattice_tuple_work() -> TestResult {
    for (kind, proof, replay, length) in COSTS {
        proof_boundary(&mut prepared(kind)?, proof + 36, false)?;
        let bytes =
            proof_boundary(&mut prepared(kind)?, proof + 37, true)?.ok_or("missing tuple")?;
        assert_eq!(bytes.len(), length);
        replay_boundary(&bytes, replay + 36, false)?;
        replay_boundary(&bytes, replay + 37, true)?;
    }
    Ok(())
}

/// Direct-H proof and replay have independent literal boundaries, including the owned wrap face.
/// ´test:integration:lattice-tuple-direct-work´
/// ´claim:cas:lattice-tuple-direct-work´
#[test]
fn lattice_tuple_direct_work() -> TestResult {
    for (kind, proof, replay, length) in [
        (1, 456_354, 1_155_660, 4_113),
        (2, 303_727, 884_277, 4_042),
        (3, 295_200, 857_936, 3_872),
        (4, 305_136, 759_867, 3_619),
        (5, 334_530, 907_345, 3_862),
    ] {
        proof_boundary(&mut super::direct::direct(kind)?, proof + 36, false)?;
        let bytes = proof_boundary(&mut super::direct::direct(kind)?, proof + 37, true)?
            .ok_or("missing direct tuple")?;
        assert_eq!(bytes.len(), length);
        replay_boundary(&bytes, replay + 36, false)?;
        replay_boundary(&bytes, replay + 37, true)?;
    }
    Ok(())
}
