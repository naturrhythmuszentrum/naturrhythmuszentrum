// SPDX-License-Identifier: AGPL-3.0-only
//! Shared transaction fixtures restricted to the public facade.

use crate::test_api::{
    Axis, AxisEntry, BlockDraft, CheckedOperation, CheckedShape, EntryDraft, ExecutionLimits,
    OperationDraft, Partition, ShapeDraft, ShapeLimits, StructureScope,
};
use nrz_units::{Context, Descriptor, ExponentPolicy, Registry};
use std::rc::Rc;

/// Fallible fixture and test result.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Declare a homogeneous rectangle over ratio columns and physical rows.
#[expect(
    clippy::default_trait_access,
    reason = "CAS types are inferred through the units facade"
)]
pub(crate) fn shape(
    rows: usize,
    columns: usize,
    quantity: &Descriptor,
) -> TestResult<CheckedShape> {
    let owner = Rc::new(
        Context::new(
            Registry::new(ExponentPolicy::Rational),
            Default::default(),
            Default::default(),
        )?
        .freeze()?,
    );
    let domain = axis(columns, &Descriptor::ratio())?;
    let codomain = axis(rows, quantity)?;
    let entries = vec![EntryDraft::Declared(quantity.clone()); rows * columns];
    let blocks = if entries.is_empty() {
        Vec::new()
    } else {
        vec![BlockDraft::new(0..rows, 0..columns, entries)]
    };
    Ok(
        ShapeDraft::new(domain, codomain, blocks, StructureScope::WholeFamily)
            .check(owner, ShapeLimits::default())?,
    )
}

/// One complete partition with distinct coordinate identities.
fn axis(length: usize, quantity: &Descriptor) -> TestResult<Axis> {
    let entries = (0..length)
        .map(|index| AxisEntry::new(format!("coordinate-{index}"), quantity.clone()))
        .collect::<Result<_, _>>()?;
    Ok(Axis::new(
        entries,
        Partition::whole(length, ShapeLimits::default())?,
        ShapeLimits::default(),
    )?)
}

/// One input/output copy contract with the standard execution policy.
pub(crate) fn operation(rows: usize, columns: usize) -> TestResult<CheckedOperation> {
    Ok(
        OperationDraft::copy(vec![shape(rows, columns, &Descriptor::ratio())?])
            .check(ExecutionLimits::default())?,
    )
}

/// A single explicitly typed structural zero, with separately selected family scope.
#[expect(
    clippy::default_trait_access,
    reason = "CAS types are inferred through the units facade"
)]
pub(crate) fn zero_shape(scope: StructureScope) -> TestResult<CheckedShape> {
    let owner = Rc::new(
        Context::new(
            Registry::new(ExponentPolicy::Rational),
            Default::default(),
            Default::default(),
        )?
        .freeze()?,
    );
    let coordinate = axis(1, &Descriptor::ratio())?;
    Ok(ShapeDraft::new(
        coordinate.clone(),
        coordinate,
        vec![BlockDraft::new(
            0..1,
            0..1,
            vec![EntryDraft::Zero(Descriptor::ratio())],
        )],
        scope,
    )
    .check(owner, ShapeLimits::default())?)
}
