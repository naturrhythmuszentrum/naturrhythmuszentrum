// SPDX-License-Identifier: AGPL-3.0-only
//! Only K's checked target rows or a previously propagated lineage can supply sources.

use super::{
    StatisticsError,
    matrix::{Matrix, singleton},
    model::Model,
    store::Lineage,
};
use crate::{Handle, Heap, HeapError, JointLimits, MeanParameters, OperationError, Value};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Selected source rows and mean parameters for one concrete operand.
#[derive(Debug)]
pub(crate) struct Input<P: Scalar> {
    /// Immutable common model, original sources and assurance.
    pub(crate) lineage: Lineage,
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

/// Resolve checked source-selection rows without inferring dependence from equal bytes.
pub(crate) fn input<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
    slot: usize,
    limits: JointLimits,
    context: &ArithmeticContext,
) -> Result<Input<P>, OperationError> {
    let value = heap.value(handle)?;
    let layout = value.region().layout();
    for record in &heap.statistics.records {
        if record.targets[0].stamp() == value.stamp() && record.sensitivity.is_some() {
            super::access::validate(heap, record)?;
            let rows = selection(layout, record.targets[0].layout(), slot, limits)?;
            let parameter = record
                .sensitivity
                .as_ref()
                .ok_or(StatisticsError::Contract)?;
            let extent = heap.descriptors.get(parameter.regions[0])?.layout.extent();
            limits.matrix(extent.rows(), extent.columns())?;
            let sensitivity = super::access::matrix(heap, parameter, context)?;
            let means = record
                .means
                .as_ref()
                .map(|parameter| super::access::matrix(heap, parameter, context))
                .transpose()?;
            return Ok(Input {
                lineage: record.lineage.clone(),
                sensitivity: select(&sensitivity, &rows)?,
                means: means.map(|matrix| select(&matrix, &rows)).transpose()?,
            });
        }
    }
    for (index, record) in heap.joint.records.iter().enumerate() {
        for (side, target) in record.targets.iter().enumerate() {
            if target.stamp == value.stamp() {
                for target in &record.targets {
                    target.validate(heap)?;
                }
                let rows = selection(layout, &target.layout, slot, limits)?;
                return declared::<P>(record, index, side, &rows, limits);
            }
        }
    }
    Err(StatisticsError::MissingSource { slot, entry: 0 }.into())
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

/// Turn K's rows into exact unit sensitivity rows over a compact ordered source selection.
fn declared<P: Scalar + 'static>(
    record: &crate::companions::JointRecord,
    index: usize,
    side: usize,
    rows: &[usize],
    limits: JointLimits,
) -> Result<Input<P>, OperationError> {
    let model = Model::capture::<P>(&record.model)?;
    let source_map = record
        .selections
        .get(side)
        .ok_or(StatisticsError::Contract)?;
    let mut columns = rows
        .iter()
        .map(|&row| {
            source_map
                .get(row)
                .copied()
                .ok_or(StatisticsError::Contract)
        })
        .collect::<Result<Vec<_>, _>>()?;
    columns.sort_unstable();
    columns.dedup();
    limits.matrix(rows.len(), columns.len())?;
    let mut sensitivity = Matrix::zeros(rows.len(), columns.len())?;
    for (row, &original) in rows.iter().enumerate() {
        let source = source_map.get(original).ok_or(StatisticsError::Contract)?;
        let column = columns
            .binary_search(source)
            .map_err(|_| StatisticsError::Contract)?;
        sensitivity.set(row, column, P::ONE, singleton(P::ONE)?)?;
    }
    let means = record
        .means
        .as_ref()
        .map(|erased| {
            let parameters = erased
                .downcast_ref::<MeanParameters<P>>()
                .ok_or(StatisticsError::Contract)?;
            let family = parameters
                .enclosures()
                .get(side)
                .ok_or(StatisticsError::Contract)?;
            let mut result = Matrix::zeros(rows.len(), 1)?;
            for (row, &original) in rows.iter().enumerate() {
                let truth = *family.get(original).ok_or(StatisticsError::Contract)?;
                result.set(row, 0, truth.lower().get(), truth)?;
            }
            Ok::<_, OperationError>(result)
        })
        .transpose()?;
    Ok(Input {
        lineage: Lineage {
            model,
            columns,
            roots: vec![index],
            assurance: super::MomentAssurance::AffineMoments,
        },
        sensitivity,
        means,
    })
}

/// Select complete parameter rows, preserving every source column.
fn select<P: Scalar>(matrix: &Matrix<P>, rows: &[usize]) -> Result<Matrix<P>, OperationError> {
    let mut result = Matrix::zeros(rows.len(), matrix.extent.columns())?;
    for column in 0..matrix.extent.columns() {
        for (row, &original) in rows.iter().enumerate() {
            let index = matrix.extent.index(original, column)?;
            result.set(
                row,
                column,
                *matrix.centers.get(index).ok_or(StatisticsError::Contract)?,
                matrix.entry(original, column)?,
            )?;
        }
    }
    Ok(result)
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
