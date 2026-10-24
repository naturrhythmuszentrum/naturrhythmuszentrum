// SPDX-License-Identifier: AGPL-3.0-only
//! Allocation meanings reuse the original checked graph and retain each physical reference.

use crate::testing::{lc_roots::TestResult, spectral::prepared};
use nrz_bindings::readings::{ReciprocalEntry, ReciprocalStage};

/// Equal-shaped foreign declarations cannot sponsor the original numerical schedule.
/// ´test:integration:reciprocal-allocation-binding-retains-original-definition´
#[test]
fn reciprocal_allocation_binding_retains_original_definition() -> TestResult {
    let first = prepared(["5", "1/10", "1/2000"])
        .map_err(|e| format!("allocation original fixture: {e}"))?;
    let second = prepared(["5", "1/10", "1/2000"])
        .map_err(|e| format!("allocation foreign fixture: {e}"))?;
    for stage in ReciprocalStage::ALL {
        let binding = first.recipe.allocation(stage);
        assert!(binding.same_binding(&first.recipe.clone().allocation(stage)));
        assert!(!binding.same_binding(&second.recipe.allocation(stage)));
        assert!(binding.entry(binding.entries()).is_none());
        for other in ReciprocalStage::ALL {
            assert_eq!(
                binding.same_binding(&first.recipe.allocation(other)),
                stage == other
            );
        }
    }
    Ok(())
}

/// Packing reference magnitudes together preserves both independently typed physical coordinates.
/// ´test:integration:reciprocal-allocation-references-keep-separate-meanings´
#[test]
fn reciprocal_allocation_references_keep_separate_meanings() -> TestResult {
    let fixture = prepared(["5", "1/10", "1/2000"])
        .map_err(|e| format!("allocation references fixture: {e}"))?;
    let binding = fixture.recipe.allocation(ReciprocalStage::References);
    assert_eq!(binding.entries(), 2);
    let Some(ReciprocalEntry::Reference(first)) = binding.entry(0) else {
        return Err("first reference binding".into());
    };
    let Some(ReciprocalEntry::Reference(second)) = binding.entry(1) else {
        return Err("second reference binding".into());
    };
    assert_ne!(first.descriptor(), second.descriptor());
    assert_eq!(first, &fixture.recipe.frame().entries()[0]);
    assert_eq!(second, &fixture.recipe.frame().entries()[1]);
    assert_eq!(ReciprocalStage::References.components().len(), 4);
    assert_eq!(ReciprocalStage::Images.components().len(), 5);
    assert_eq!(ReciprocalStage::Contracted.components().len(), 3);
    Ok(())
}
