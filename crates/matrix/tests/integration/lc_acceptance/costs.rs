// SPDX-License-Identifier: AGPL-3.0-only
//! Exact owner charges and fine dependency inventories remain distinct from numerical payloads.

use super::{owners::Case, state_fixture::Plan};
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::readings::algebra::readings::DependencyInventory;
use nrz_bindings::{Budget, Limits};

const OWNERS: [[u64; 3]; 6] = [
    [169, 2_952, 0],
    [593_488, 65_170, 13_336],
    [2_574, 616, 0],
    [41_888, 63_178, 63_386],
    [38_815, 63_642, 63_850],
    [38_863, 127_012, 127_188],
];

/// Each operation charges its measured Work and refuses immediately below each positive resource boundary.
/// ´test:integration:lc-acceptance-owner-costs-and-boundaries´
#[test]
fn lc_acceptance_owner_costs_and_boundaries() -> TestResult {
    let case = Case::new()?;
    for (operation, profile) in OWNERS.into_iter().enumerate() {
        let mut resources = ledger()?;
        case.run(operation, &mut resources)?;
        assert_eq!(resources.usage().work, profile[0]);
        for (kind, needed) in profile.into_iter().enumerate() {
            assert!(case.run(operation, &mut policy(kind, needed, 0)?).is_ok());
            if needed > 0 {
                assert!(
                    case.run(operation, &mut policy(kind, needed - 1, 0)?)
                        .is_err()
                );
            }
        }
        assert!(case.run(operation, &mut policy(0, profile[0], 1)?).is_err());
    }
    Ok(())
}

/// `Entries` admission is exactly `WholeBase` admission plus all ten authenticated fine inventories.
/// ´test:integration:lc-acceptance-inventory-costs-reconcile´
#[test]
fn lc_acceptance_inventory_costs_reconcile() -> TestResult {
    let case = Case::new()?;
    let expected = [
        [71_986, 7, 786],
        [137_837, 13, 1_065],
        [46_315, 7, 671],
        [86_616, 13, 895],
        [46_315, 7, 671],
        [86_616, 13, 895],
        [43_288, 10, 667],
        [43_288, 10, 667],
        [48_835, 6, 660],
        [48_835, 6, 660],
    ];
    let shared_costs = [
        71_986, 126_002, 40_000, 77_115, 40_112, 77_115, 35_501, 35_741, 43_671, 43_671,
    ];
    let mut shared = ledger()?;
    let mut total = 0;
    for (index, profile) in expected.into_iter().enumerate() {
        let mut budget = ledger()?;
        let dependency = inventory(&case.plan, index, &mut budget)?;
        let records = &dependency.contents().records;
        let bytes = records.iter().map(Vec::len).sum::<usize>();
        assert_eq!(
            [
                budget.usage().work,
                u64::try_from(records.len())?,
                u64::try_from(bytes)?
            ],
            profile
        );
        let before = shared.usage().work;
        let repeated = inventory(&case.plan, index, &mut shared)?;
        assert_eq!(dependency.contents(), repeated.contents());
        let delta = shared.usage().work - before;
        assert_eq!(delta, shared_costs[index]);
        assert!(delta <= profile[0]);
        total += delta;
        inventory_boundary(&case.plan, index, profile[0])?;
    }
    let mut entries = ledger()?;
    let mut whole = ledger()?;
    case.run(1, &mut entries)?;
    case.run(2, &mut whole)?;
    assert_eq!(total, 590_914);
    assert_eq!(entries.usage().work, whole.usage().work + total);
    Ok(())
}

/// Cold inventory Work succeeds exactly at its measured boundary, including prior-ledger refusal.
fn inventory_boundary(plan: &Plan, index: usize, needed: u64) -> TestResult {
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        assert_eq!(
            inventory(plan, index, &mut policy(0, limit, prior)?).is_ok(),
            success
        );
    }
    Ok(())
}

fn inventory(plan: &Plan, index: usize, budget: &mut Budget) -> TestResult<DependencyInventory> {
    if index < 6 {
        let adapter = &plan.coverage.readings()[index / 2];
        return Ok(if index % 2 == 0 {
            plan.graph
                .reading_dependencies(adapter.reading().node(), budget)?
        } else {
            plan.graph
                .witness_dependencies(adapter.representation().witness(), budget)?
        });
    }
    if index < 8 {
        return Ok(plan
            .graph
            .witness_dependencies(plan.coverage.overlaps()[index - 6].witness(), budget)?);
    }
    Ok(plan
        .graph
        .reading_dependencies(plan.companions[index - 8].companion().node(), budget)?)
}

/// Only the tested ceiling changes; earlier Work remains charged on the same ledger.
pub(super) fn policy(kind: usize, value: u64, prior: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        0 => limits.work = value,
        1 => limits.staged_payload_bytes = value,
        2 => limits.output_bytes = value,
        _ => return Err("unknown resource owner".into()),
    }
    let mut budget = Budget::new(limits)?;
    budget.work(
        prior,
        nrz_bindings::readings::algebra::Location::new("prior LC owner work"),
    )?;
    Ok(budget)
}
