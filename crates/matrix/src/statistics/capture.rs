// SPDX-License-Identifier: AGPL-3.0-only
//! Only K's checked target rows or a previously propagated lineage can supply sources.

use super::inventory::{Role, Run};
use super::{
    StatisticsError,
    matrix::{Matrix, singleton},
    store::Lineage,
};
use crate::{Handle, Heap, HeapError, JointLimits, MeanParameters, OperationError, Value};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

#[path = "capture_binding.rs"]
mod binding;
pub(crate) use binding::{InputBinding, Origin, inspect};

/// Selected source rows and mean parameters for one concrete operand.
#[derive(Debug)]
pub(crate) struct Input<P: Scalar> {
    /// Immutable common model, original sources and assurance.
    pub(crate) lineage: Rc<Lineage>,
    /// Logical entry rows, including exact repetitions.
    pub(crate) sensitivity: Matrix<P>,
    /// Mean parameters if explicitly declared.
    pub(crate) means: Option<Matrix<P>>,
}

/// Detect statistical coverage by version, including selected aliases.
pub(crate) fn present<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
) -> Result<bool, HeapError> {
    let stamp = heap.value(handle)?.stamp();
    Ok(heap
        .joint
        .records
        .iter()
        .any(|record| record.targets.iter().any(|target| target.stamp == stamp))
        || heap
            .statistics
            .records
            .iter()
            .any(|record| record.targets[0].stamp() == stamp && record.sensitivity.is_some()))
}

/// Materialize only the exact rows already retained by the covering metadata inventory.
pub(crate) fn input<P: Scalar + 'static>(
    heap: &Heap,
    slot: usize,
    run: &Run,
    context: &ArithmeticContext,
) -> Result<Input<P>, OperationError> {
    let binding = run.binding(slot)?;
    binding.dependency.validate(heap)?;
    let mut sensitivity = run.matrix(Role::InputSensitivity, slot)?;
    let mut means = if run.contains(Role::InputMeans, slot) {
        Some(run.matrix(Role::InputMeans, slot)?)
    } else {
        None
    };
    match binding.origin {
        Origin::Propagated(index) => {
            let record = heap
                .statistics
                .records
                .get(index)
                .ok_or(StatisticsError::Contract)?;
            let parameter = record
                .sensitivity
                .as_ref()
                .ok_or(StatisticsError::Contract)?;
            super::access::selected_into(
                heap,
                parameter,
                &binding.rows,
                context,
                &mut sensitivity,
            )?;
            if let Some(means) = &mut means {
                let parameter = record.means.as_ref().ok_or(StatisticsError::Contract)?;
                super::access::selected_into(heap, parameter, &binding.rows, context, means)?;
            }
        }
        Origin::Declared { record, side } => {
            let record = heap
                .joint
                .records
                .get(record)
                .ok_or(StatisticsError::Contract)?;
            declared(record, side, binding, &mut sensitivity, means.as_mut())?;
        }
    }
    Ok(Input {
        lineage: Rc::clone(&binding.lineage),
        sensitivity,
        means,
    })
}

/// Match exact underlying addresses; repeated logical entries intentionally reuse a row.
fn selection(
    selected: &crate::Layout,
    original: &crate::Layout,
    slot: usize,
    limits: JointLimits,
) -> Result<Vec<usize>, OperationError> {
    limits.check_work(
        selected
            .extent()
            .entries()
            .checked_mul(original.extent().entries())
            .ok_or(StatisticsError::ResourceLimit)?,
    )?;
    let mut result = crate::binding::buffer(selected.extent().entries())?;
    for entry in 0..selected.extent().entries() {
        let address = selected.flat_index(entry)?;
        let mut found = None;
        for row in 0..original.extent().entries() {
            if original.flat_index(row)? == address {
                found = Some(row);
                break;
            }
        }
        result.push(found.ok_or(StatisticsError::MissingSource { slot, entry })?);
    }
    Ok(result)
}

/// Original K rows supply exact unit sensitivities and explicitly declared means.
fn declared<P: Scalar + 'static>(
    record: &crate::companions::JointRecord,
    side: usize,
    binding: &InputBinding,
    sensitivity: &mut Matrix<P>,
    means: Option<&mut Matrix<P>>,
) -> Result<(), OperationError> {
    let source_map = record
        .selections
        .get(side)
        .ok_or(StatisticsError::Contract)?;
    for (row, &original) in binding.rows.iter().enumerate() {
        let source = source_map.get(original).ok_or(StatisticsError::Contract)?;
        let column = binding
            .lineage
            .columns
            .binary_search(source)
            .map_err(|_| StatisticsError::Contract)?;
        sensitivity.set(row, column, P::ONE, singleton(P::ONE)?)?;
    }
    if let Some(means) = means {
        let parameters = record
            .means
            .as_ref()
            .and_then(|erased| erased.downcast_ref::<MeanParameters<P>>())
            .ok_or(StatisticsError::Contract)?;
        let family = parameters
            .enclosures()
            .get(side)
            .ok_or(StatisticsError::Contract)?;
        for (row, &original) in binding.rows.iter().enumerate() {
            let truth = *family.get(original).ok_or(StatisticsError::Contract)?;
            means.set(row, 0, truth.lower().get(), truth)?;
        }
    }
    Ok(())
}

/// Preserve the active solve boundary until a statistical solve law is implemented.
pub(crate) fn refuse<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
) -> Result<(), HeapError> {
    if present(heap, handle)? {
        Err(HeapError::MissingCoverage)
    } else {
        Ok(())
    }
}
