// SPDX-License-Identifier: AGPL-3.0-only
//! Original mixed-state and covariance declarations exercise one pi and one rational reference.

use super::{TestResult, state_fixture};
use crate::reading_testing::ledger;
use nrz_bindings::{Descriptor, ReferenceDefinition, Scale, readings::EntryMeaning};
use nrz_matrix::{DependencyPolicy, Extent, Heap, HeapLimits, ReadingBinding, ReadingRegion};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Physical angle and current references retain separate kinds and immutable definition identities.
pub(super) fn plan() -> TestResult<state_fixture::Plan> {
    with_scale(Scale::pi()?)
}

/// Unsupported exact constants remain representable until numerical admission.
pub(super) fn with_scale(scale: Scale) -> TestResult<state_fixture::Plan> {
    let angle = Descriptor::new(
        nrz_bindings::Kind::Angle,
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Difference,
    )?;
    let current = crate::reading_testing::meaning("current", [0, 0, 0, 1, 0, 0, 0])?;
    let reference = ReferenceDefinition::new("half-turn", &angle, scale)?;
    let current_reference = ReferenceDefinition::new(
        "half-current",
        current.descriptor(),
        Scale::Exact(nrz_bindings::readings::algebra::Rational::parse(
            "1/2",
            &mut ledger()?,
        )?),
    )?;
    let entries = [
        EntryMeaning::new(angle, reference, None, &mut ledger()?)?,
        EntryMeaning::new(
            current.descriptor().clone(),
            current_reference,
            None,
            &mut ledger()?,
        )?,
    ];
    state_fixture::covariance_plan(&entries)
}

/// The shared fixture allowance is the measured binary64 restoration ceiling.
pub(super) fn execution() -> nrz_bindings::LimitsOverride {
    nrz_bindings::LimitsOverride {
        staged_payload_bytes: Some(81_296),
        output_bytes: Some(81_192),
        ..nrz_bindings::LimitsOverride::default()
    }
}

/// Every original center, radius and statistical parameter has its own admitted live region.
pub(super) fn imported<P: Scalar + 'static>(
    plan: &state_fixture::Plan,
) -> TestResult<(Heap, ReadingBinding<P>)> {
    let (heap, bindings) = all::<P>(plan)?;
    let first = bindings.first().ok_or("whole state")?.clone();
    Ok((heap, first))
}

/// Complete original selectors expose every covariance target and selected component.
pub(super) fn all<P: Scalar + 'static>(
    plan: &state_fixture::Plan,
) -> TestResult<(Heap, Vec<ReadingBinding<P>>)> {
    let eighth = P::ONE / P::from_u8(8);
    let sixty_fourth = P::ONE / P::from_u8(64);
    let mut regions = [
        [P::from_u8(3), P::ONE / P::from_u8(4)],
        [P::ONE / P::from_u8(2), P::ONE / P::from_u8(16)],
        [eighth, sixty_fourth],
        [P::from_u8(2), P::ONE / P::from_u8(2)],
    ]
    .into_iter()
    .map(|values| {
        Ok(ReadingRegion {
            extent: Extent::new(2, 1)?,
            values: values.to_vec(),
        })
    })
    .collect::<TestResult<Vec<_>>>()?;
    for _ in 0..3 {
        regions.push(ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: vec![
                eighth,
                -sixty_fourth,
                -sixty_fourth,
                P::ONE / P::from_u8(16),
            ],
        });
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let imported = heap.import_readings(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        plan.companions.clone(),
        &regions,
        DependencyPolicy::WholeBase,
        &mut ledger()?,
    )?;
    let bindings = imported.bindings().to_vec();
    drop(imported);
    Ok((heap, bindings))
}
