// SPDX-License-Identifier: AGPL-3.0-only
//! Caller budgets bound the exact closed spectral law without replacing the original replay policy.

use crate::testing::{
    lc_roots::TestResult,
    spectral::{SpectralFixture, ledger, prepared},
};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{Location, limits::BudgetKind},
};

/// Exact law checking shares prior Work and retains failed retries on the caller ledger.
/// ´test:integration:lc-spectral-exact-work-boundary´
#[test]
fn lc_spectral_exact_work_boundary() -> TestResult {
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    let mut measured = ledger()?;
    fixture.check(&mut measured)?;
    let required = measured.usage().work + 17;
    for work in [required, required - 1] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, Location::new("earlier exact work"))?;
        let result = fixture.check(&mut budget);
        assert_eq!(result.is_ok(), work == required, "{result:?}");
        if result.is_err() {
            let spent = budget.usage();
            assert!(fixture.check(&mut budget).is_err());
            assert!(budget.usage().work >= spent.work);
        }
    }
    Ok(())
}

/// Tight byte limits measure retained output and scratch independently under unchanged defaults.
/// ´test:integration:lc-spectral-exact-payload-boundaries´
#[test]
fn lc_spectral_exact_payload_boundaries() -> TestResult {
    let mut fixture = prepared(["5", "1/10", "1/2000"])?;
    let mut costs = Vec::new();
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        let mut lower = 1;
        let mut upper = Limits::default().output_bytes;
        assert!(attempt(&mut fixture, kind, upper, 0)?);
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(&mut fixture, kind, middle, 0)? {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(attempt(&mut fixture, kind, upper, 0)?);
        assert!(!attempt(&mut fixture, kind, upper - 1, 0)?);
        if kind == BudgetKind::StagedPayloadBytes {
            assert!(attempt(&mut fixture, kind, upper + 23, 23)?);
            assert!(!attempt(&mut fixture, kind, upper + 22, 23)?);
        }
        costs.push(upper);
    }
    // Native ctx.hypothesis admission owns transaction and witness-encoding scratch.
    // The 832-byte law owns its record and two qualified references; prior evidence is shared.
    assert_eq!(costs, vec![17_344, 832]);
    Ok(())
}

fn attempt(
    fixture: &mut SpectralFixture,
    kind: BudgetKind,
    limit: u64,
    retained: u64,
) -> TestResult<bool> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        _ => limits.output_bytes = limit,
    }
    let mut budget = Budget::new(limits)?;
    Ok(budget
        .with_retained_payload(
            retained,
            Location::new("caller retained evidence"),
            |budget| fixture.check(budget),
        )
        .is_ok())
}

/// A solver-specific narrow bit cap cannot silently become the physical store's default policy.
/// ´test:integration:lc-spectral-construction-width-boundary´
#[test]
fn lc_spectral_construction_width_boundary() {
    use crate::testing::spectral::prepared_with_policy;
    let attempt = |component_bits| {
        prepared_with_policy(
            ["5", "1/10", "1/2000"],
            Limits {
                component_bits,
                ..Limits::default()
            },
        )
        .is_ok()
    };
    let mut lower = 1;
    let mut upper = Limits::default().component_bits;
    assert!(attempt(upper));
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if attempt(middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(attempt(upper));
    assert!(!attempt(upper - 1));
    assert_eq!(upper, 66);
}
