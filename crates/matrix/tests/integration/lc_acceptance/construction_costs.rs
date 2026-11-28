// SPDX-License-Identifier: AGPL-3.0-only
//! Proof construction, portable replay and exact coordinate images retain separate measured owners.

use super::{costs::policy, state_fixture};
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::readings::algebra::Cas;
use nrz_bindings::{Budget, Limits};

/// Both overlap proofs, the named frame and its exact images honor cumulative Work and payload ceilings.
/// ´test:integration:lc-acceptance-construction-costs-and-boundaries´
#[test]
fn lc_acceptance_construction_costs_and_boundaries() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let entries = spectral.law.state().entries();
    let plan = state_fixture::plan(entries)?;
    assert_eq!(plan.construction, [25_817, 52_260, 325_993, 496_159]);
    assert_eq!(
        [
            plan.contract.frame().retained_payload_bytes(),
            plan.contract.normalization().retained_payload_bytes(),
            plan.contract.restoration().retained_payload_bytes()
        ],
        [20_464, 31_948, 31_948]
    );
    for (kind, needed) in [496_159, 187_638, 63_896].into_iter().enumerate() {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            let result = state_fixture::plan_with_budget(entries, &mut policy(kind, limit, 0)?);
            assert_eq!(
                result.is_ok(),
                success,
                "kind {kind}, limit {limit}: {:?}",
                result.err()
            );
        }
    }
    assert!(state_fixture::plan_with_budget(entries, &mut policy(0, 496_159, 1)?).is_err());
    Ok(())
}

/// Canonical bytes, export and independent receiving-store replay have distinct exact Work boundaries.
/// ´test:integration:lc-acceptance-overlap-replay-costs-and-boundaries´
#[test]
fn lc_acceptance_overlap_replay_costs_and_boundaries() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = state_fixture::plan(spectral.law.state().entries())?;
    for relation in plan.coverage.overlaps() {
        let mut budget = ledger()?;
        let bytes = relation.export(&plan.graph, &mut budget)?;
        assert_eq!((bytes.len(), budget.usage().work), (710, 710));
        let mut budget = ledger()?;
        replay(&bytes, &mut budget)?;
        assert_eq!(budget.usage().work, 57_495);
        for (limit, prior, success) in [(710, 0, true), (709, 0, false), (710, 1, false)] {
            assert_eq!(
                relation
                    .export(&plan.graph, &mut policy(0, limit, prior)?)
                    .is_ok(),
                success
            );
        }
        for (limit, prior, success) in [(57_495, 0, true), (57_494, 0, false), (57_495, 1, false)] {
            assert_eq!(
                replay(&bytes, &mut policy(0, limit, prior)?).is_ok(),
                success
            );
        }
    }
    Ok(())
}

fn replay(bytes: &[u8], budget: &mut Budget) -> TestResult {
    let mut receiver = Cas::new(Limits::default())?;
    assert!(receiver.inspect(bytes, budget)?.is_unconditional());
    Ok(())
}
