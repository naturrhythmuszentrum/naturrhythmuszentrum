// SPDX-License-Identifier: AGPL-3.0-only
//! Source-free mixed-state restoration, named norms and surviving views use public bindings.

use super::state_fixture;
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use nrz_accuracy::NormKind;
use nrz_matrix::{Coverage, DependencyPolicy, Heap, HeapLimits, ReadingCoverage};
use nrz_scalar::ArithmeticContext;

/// Shared exact references restore both coordinates and outward radii before a named physical norm.
/// ´test:integration:lc-acceptance-mixed-state-norm-and-lifetime´
#[test]
fn lc_acceptance_mixed_state_norm_and_lifetime() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = state_fixture::plan(spectral.law.state().entries())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(
        &imported.bindings()[0],
        &plan.contract,
        &context,
        &mut ledger()?,
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    restored_oracle(&restored)?;
    norms(
        &heap,
        &restored,
        &plan.contract,
        spectral.law.state().entries(),
        &context,
    )?;
    let voltage = imported.bindings()[1].clone();
    let current = imported.bindings()[2].clone();
    drop(restored);
    drop(normalized);
    drop(imported);
    heap.compact()?;
    assert_eq!(
        heap.reading_entries(&voltage)?[0].to_bits(),
        3.0_f64.to_bits()
    );
    heap.replace_reading_entries(&current, &[0.25], &mut ledger()?)?;
    assert!(heap.reading_entries(&current).is_err());
    assert!(heap.reading_entries(&voltage).is_ok());
    Ok(())
}

/// Fine entry dependencies preserve disjoint voltage; whole-base fallback grants no such claim.
/// ´test:integration:lc-acceptance-overlap-and-coarse-invalidation´
#[test]
fn lc_acceptance_overlap_and_coarse_invalidation() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = state_fixture::plan(spectral.law.state().entries())?;
    for policy in [DependencyPolicy::Entries, DependencyPolicy::WholeBase] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let imported = state_fixture::import(&plan, &mut heap, policy)?;
        let context = ArithmeticContext::new()?;
        let normalized = heap.normalize_reading(
            &imported.bindings()[0],
            &plan.contract,
            &context,
            &mut ledger()?,
        )?;
        heap.replace_reading_entries(&imported.bindings()[2], &[0.25], &mut ledger()?)?;
        assert!(
            heap.restore_reading(&normalized, &context, &mut ledger()?)
                .is_err()
        );
        assert!(heap.reading_entries(&imported.bindings()[0]).is_err());
        assert_eq!(
            heap.reading_entries(&imported.bindings()[1]).is_ok(),
            policy == DependencyPolicy::Entries
        );
    }
    Ok(())
}

/// Missing or substituted selected-coordinate proofs cannot license the mixed-state overlaps.
/// ´test:integration:lc-acceptance-rejects-unrelated-overlap-inventory´
#[test]
fn lc_acceptance_rejects_unrelated_overlap_inventory() -> TestResult {
    let spectral = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let plan = state_fixture::plan(spectral.law.state().entries())?;
    assert!(
        ReadingCoverage::new(
            plan.coverage.entries().to_vec(),
            plan.coverage.readings().to_vec(),
            Vec::new(),
            &mut ledger()?
        )
        .is_err()
    );
    let wrong = vec![plan.coverage.overlaps()[0].clone(); 2];
    assert!(
        ReadingCoverage::new(
            plan.coverage.entries().to_vec(),
            plan.coverage.readings().to_vec(),
            wrong,
            &mut ledger()?
        )
        .is_err()
    );
    let mut incomplete = plan.coverage.readings().to_vec();
    incomplete.truncate(2);
    assert!(
        ReadingCoverage::new(
            plan.coverage.entries().to_vec(),
            incomplete[1..].to_vec(),
            Vec::new(),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

pub(super) fn restored_oracle(restored: &nrz_matrix::CoordinateReading<f64>) -> TestResult {
    for (index, endpoints) in [[(5, 2), (7, 2)], [(3, 16), (5, 16)]]
        .into_iter()
        .enumerate()
    {
        for (top, bottom) in endpoints {
            assert!(
                Rational::new(top, bottom)?
                    .covered(restored.centers()[index], restored.totals()[index])?
            );
        }
        assert!(restored.errors()[index] < restored.totals()[index]);
    }
    Ok(())
}

pub(super) fn norms(
    heap: &Heap,
    restored: &nrz_matrix::CoordinateReading<f64>,
    contract: &nrz_matrix::CoordinateContract,
    entries: &[nrz_bindings::readings::EntryMeaning],
    context: &ArithmeticContext,
) -> TestResult {
    for (kind, numerator, denominator) in [
        (NormKind::One, 29, 40),
        (NormKind::Euclidean, 641, 1600),
        (NormKind::Infinity, 5, 8),
    ] {
        let norm = heap.coordinate_norm(restored, contract, kind, context, &mut ledger()?)?;
        let upper = Rational::binary(norm.total().upper().get())?;
        let upper = if kind == NormKind::Euclidean {
            upper.multiply(&upper)?
        } else {
            upper
        };
        assert!(Rational::new(numerator, denominator)?.at_most(&upper)?);
        assert_eq!(norm.coverage(), Coverage::DeclaredBounds);
        assert_eq!(norm.references().entries(), entries);
    }
    Ok(())
}
