// SPDX-License-Identifier: AGPL-3.0-only
//! Exact count origins remain shared across numerical operations and bounded before publication.

use super::ExactCountRatio;
use nrz_bindings::Budget;
use nrz_bindings::readings::algebra::{Fault, Location, limits::BudgetKind};
use std::rc::Rc;

/// Optional shared source inventory; ordinary quantities allocate no count-origin array.
type Sources = Option<Rc<[ExactCountRatio]>>;

/// Prepay a source list and its conservative complete retained exact owners.
fn prepare(sources: &[&[ExactCountRatio]], budget: &mut Budget) -> Result<(), Fault> {
    let at = Location::new("quantity count origins");
    let mut bytes = u64::try_from(2 * size_of::<usize>()).unwrap_or(u64::MAX);
    for source in sources.iter().flat_map(|sources| sources.iter()) {
        budget.work(1, at)?;
        bytes = bytes
            .saturating_add(source.retained_payload_bytes())
            .saturating_add(u64::try_from(size_of::<ExactCountRatio>()).unwrap_or(u64::MAX));
    }
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        budget.check(kind, bytes, at)?;
    }
    Ok(())
}

/// Retain the exact import before any numerical publication can occur.
pub(super) fn single(source: &ExactCountRatio, budget: &mut Budget) -> Result<Sources, Fault> {
    let sources = std::slice::from_ref(source);
    prepare(&[sources], budget)?;
    Ok(Some(Rc::from(sources)))
}

/// Combine source inventories without merging distinct physical origins or losing either operand.
pub(super) fn merge(
    left: &Sources,
    right: &Sources,
    budget: &mut Budget,
) -> Result<Sources, Fault> {
    match (left, right) {
        (Some(left), Some(right)) => {
            prepare(&[left, right], budget)?;
            Ok(Some(left.iter().chain(right.iter()).cloned().collect()))
        }
        (Some(_), None) => Ok(left.clone()),
        (None, _) => Ok(right.clone()),
    }
}
