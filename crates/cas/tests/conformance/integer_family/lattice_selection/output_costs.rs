// SPDX-License-Identifier: AGPL-3.0-only
//! Exact output construction, fresh identity replay and capacity consumers own their Work.

use super::{TestResult, ledger, prepared, selected};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::{Cas, FaultKind, Location};

/// Kind, output construction Work, fresh identity inspection Work and identity witness bytes.
const COSTS: [(u32, u64, u64, usize); 4] = [
    (1, 117_834, 182_069, 1_426),
    (2, 78_955, 140_033, 1_255),
    (4, 76_255, 135_242, 1_208),
    (5, 99_607, 161_213, 1_389),
];

fn budget(owner: BudgetKind, amount: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match owner {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        _ => return Err("unsupported output boundary fixture".into()),
    }
    let mut budget = Budget::new(limits)?;
    budget.work(37, Location::new("prior lattice output Work"))?;
    Ok(budget)
}

fn construction(
    kind: u32,
    owner: BudgetKind,
    amount: u64,
    succeeds: bool,
) -> TestResult<Option<Vec<u8>>> {
    let mut source = prepared(kind)?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let cas = &mut source.source.frame.cas;
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let original = cas.export_witness(&witness, &mut ledger()?)?;
    let mut budget = budget(owner, amount)?;
    let limits = budget.limits().clone();
    let result = cas.lattice_selection_outputs(&witness, &mut budget);
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > 37);
    assert_eq!(cas.export_witness(&witness, &mut ledger()?)?, original);
    if succeeds {
        let outputs = result?;
        if owner == BudgetKind::Work {
            assert_eq!(budget.usage().work, amount);
        }
        Ok(Some(
            cas.export_witness(outputs.identity(), &mut ledger()?)?,
        ))
    } else {
        let fault = result
            .err()
            .ok_or("exact outputs exceeded resource boundary")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == owner));
        assert!(budget.usage().work <= limits.work);
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        Ok(None)
    }
}

fn replay(bytes: &[u8], amount: u64, succeeds: bool) -> TestResult {
    let mut receiver = Cas::new(Limits::default())?;
    let before = receiver.counts();
    let snapshot = receiver.canonical_snapshot(&mut ledger()?)?;
    let mut budget = budget(BudgetKind::Work, amount)?;
    let limits = budget.limits().clone();
    let result = receiver.inspect(bytes, &mut budget);
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work > 37);
    if succeeds {
        assert_eq!(budget.usage().work, amount);
        assert_eq!(receiver.export_witness(&result?, &mut ledger()?)?, bytes);
    } else {
        let fault = result.err().ok_or("identity replay exceeded Work")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert!(budget.usage().work <= amount);
        assert_eq!(receiver.counts(), before);
        assert_eq!(receiver.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}

/// Output construction and fresh identity replay have atomic literal N/N−1 Work boundaries.
/// ´test:integration:lattice-output-work´
#[test]
fn lattice_output_work() -> TestResult {
    for (kind, work, inspection, length) in COSTS {
        construction(kind, BudgetKind::Work, work + 36, false)?;
        let bytes = construction(kind, BudgetKind::Work, work + 37, true)?
            .ok_or("missing output identity")?;
        assert_eq!(bytes.len(), length);
        replay(&bytes, inspection + 36, false)?;
        replay(&bytes, inspection + 37, true)?;
    }
    Ok(())
}

/// Insufficient returned ownership rolls back all output syntax and proof, preserving the tuple.
/// ´test:integration:lattice-output-storage´
#[test]
fn lattice_output_storage() -> TestResult {
    for (kind, _, _, length) in COSTS {
        let bytes = u64::try_from(length)?;
        construction(kind, BudgetKind::OutputBytes, bytes + 511, false)?;
        let identity = construction(kind, BudgetKind::OutputBytes, bytes + 512, true)?
            .ok_or("missing sized output")?;
        assert_eq!(identity.len(), length);
    }
    Ok(())
}

/// Unsigned and signed coordinate consumers have independent 38/37 and 37/36 Work boundaries.
/// ´test:integration:lattice-output-conversion-work´
#[test]
fn lattice_output_conversion_work() -> TestResult {
    let mut source = prepared(5)?;
    let witness = selected(&mut source, &mut ledger()?)?;
    let cas = &source.source.frame.cas;
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    for signed in [false, true] {
        let work = if signed { 37 } else { 38 };
        for succeeds in [false, true] {
            let amount = work + 36 + u64::from(succeeds);
            let mut budget = budget(BudgetKind::Work, amount)?;
            let limits = budget.limits().clone();
            let result = if signed {
                cas.materialize_lattice_index(&witness, 0, (-2, 2), &mut budget)
                    .map(i128::from)
            } else {
                cas.materialize_lattice_count(&witness, 0, 2, &mut budget)
                    .map(i128::from)
            };
            if succeeds {
                assert_eq!(result?, 2);
                assert_eq!(budget.usage().work, amount);
            } else {
                let fault = result.err().ok_or("coordinate conversion exceeded Work")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
                assert!(budget.usage().work > 37);
                assert!(budget.usage().work <= amount);
            }
            assert_eq!(budget.limits(), &limits);
            assert_eq!(cas.counts(), before);
            assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
    }
    Ok(())
}
