// SPDX-License-Identifier: AGPL-3.0-only
//! Public kernel fixtures retain one declaration owner and explicitly shared coordinate axes.

pub(crate) use crate::conversion_testing::{TestResult, owner};
use crate::test_api::{
    Axis, AxisEntry, Binding, BlockDraft, CheckedShape, EntryDraft, ExecutionLimits, Handle, Heap,
    OperationDraft, Outcome, Partition, PublishedBundle, ShapeDraft, ShapeLimits, StructureScope,
    Value,
};
use nrz_scalar::Scalar;
use nrz_units::{CheckedDeclarations, Descriptor};
use std::rc::Rc;

/// Declare an ordered ratio axis; callers clone it to preserve coordinate identity.
pub(crate) fn axis(length: usize) -> TestResult<Axis> {
    let limits = ShapeLimits::default();
    let entries = (0..length)
        .map(|index| AxisEntry::new(format!("coordinate-{index}"), Descriptor::ratio()))
        .collect::<Result<_, _>>()?;
    Ok(Axis::new(
        entries,
        Partition::whole(length, limits)?,
        limits,
    )?)
}

/// Check a ratio rectangle over existing axes and one frozen owner.
pub(crate) fn shape(
    owner: &Rc<CheckedDeclarations>,
    domain: &Axis,
    codomain: &Axis,
) -> TestResult<CheckedShape> {
    let rows = codomain.entries().len();
    let columns = domain.entries().len();
    let blocks = if rows == 0 || columns == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(
            0..rows,
            0..columns,
            vec![EntryDraft::Declared(Descriptor::ratio()); rows * columns],
        )]
    };
    Ok(ShapeDraft::new(
        domain.clone(),
        codomain.clone(),
        blocks,
        StructureScope::WholeFamily,
    )
    .check(Rc::clone(owner), ShapeLimits::default())?)
}

/// Run the actual public transaction; no oracle participates in candidate generation.
pub(crate) fn execute<P: Scalar + 'static>(
    heap: &mut Heap,
    draft: OperationDraft,
    inputs: Vec<Handle<Value, P>>,
) -> TestResult<Outcome<Vec<PublishedBundle<P>>>> {
    let limits = ExecutionLimits::default();
    let operation = draft.check(limits)?;
    let bound = Binding::new(operation, inputs, limits).validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    Ok(bound.publish(candidate)?)
}

/// Inspect the one complete output after successful atomic publication.
pub(crate) fn output<P: Scalar>(
    outcome: &Outcome<Vec<PublishedBundle<P>>>,
) -> TestResult<PublishedBundle<P>> {
    Ok(*outcome.output().first().ok_or("missing kernel output")?)
}

/// Read a binary candidate and bound for independent rational comparison.
pub(crate) fn entry<P: Scalar + 'static>(
    heap: &Heap,
    bundle: PublishedBundle<P>,
    index: usize,
) -> TestResult<(f64, f64, f64)> {
    let value = heap.value(bundle.value())?;
    let row = index % value.region().extent().rows();
    let column = index / value.region().extent().rows();
    let total = heap.companion(bundle.bound().ok_or("total")?)?;
    let error = heap.companion(bundle.evaluation().ok_or("evaluation")?)?;
    Ok((
        value.region().get(row, column)?.to_f64(),
        total.region().get(row, column)?.to_f64(),
        error.region().get(row, column)?.to_f64(),
    ))
}
