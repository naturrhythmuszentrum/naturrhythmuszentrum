// SPDX-License-Identifier: AGPL-3.0-only
//! Original statistical selections are fixed before any input parameter matrix is allocated.

use super::{StatisticsError, selection};
use crate::statistics::{model::Model, store::Lineage};
use crate::{Dependency, Handle, Heap, InputSnapshot, JointLimits, OperationError, Value};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Exact immutable record supplying a selected statistical operand.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Origin {
    /// Previously propagated sensitivity and optional mean parameters.
    Propagated(usize),
    /// Original joint declaration and its selected target side.
    Declared {
        /// Original joint companion record.
        record: usize,
        /// Original ordered target side.
        side: usize,
    },
}
/// Metadata-only binding shared by numerical capture and the historical inventory.
#[derive(Debug)]
pub(crate) struct InputBinding {
    /// Exact original value occupation and ordered storage map.
    pub(crate) dependency: Dependency,
    /// Original value and own-target companion declarations.
    pub(crate) symbols: nrz_bindings::BindingSymbols,
    /// Original model, selected columns, roots and admitted law.
    pub(crate) lineage: Rc<Lineage>,
    /// Selected logical rows in the source parameter record.
    pub(crate) rows: Vec<usize>,
    /// Immutable source record identity, never selected by equal values.
    pub(crate) origin: Origin,
    /// An explicit original mean declaration exists.
    pub(crate) means: bool,
}
/// Resolve all source meanings and exact selection order without materializing floats.
pub(crate) fn inspect<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
    slot: usize,
    limits: JointLimits,
) -> Result<InputBinding, OperationError> {
    let snapshot = InputSnapshot::capture(heap, handle)?;
    let value = heap.value(handle)?;
    for (index, record) in heap.statistics.records.iter().enumerate() {
        if record.targets[0].stamp() == value.stamp() && record.sensitivity.is_some() {
            crate::statistics::access::validate(heap, record)?;
            let rows = selection(
                value.region().layout(),
                record.targets[0].layout(),
                slot,
                limits,
            )?;
            limits.matrix(rows.len(), record.lineage.columns.len())?;
            return Ok(InputBinding {
                dependency: Dependency::from_snapshot(&snapshot),
                symbols: snapshot.symbols,
                lineage: Rc::clone(&record.lineage),
                rows,
                origin: Origin::Propagated(index),
                means: record.means.is_some(),
            });
        }
    }
    for (index, record) in heap.joint.records.iter().enumerate() {
        for (side, target) in record.targets.iter().enumerate() {
            if target.stamp == value.stamp() {
                for target in &record.targets {
                    target.validate(heap)?;
                }
                let rows = selection(value.region().layout(), &target.layout, slot, limits)?;
                let lineage = declared::<P>(record, index, side, &rows, limits)?;
                return Ok(InputBinding {
                    dependency: Dependency::from_snapshot(&snapshot),
                    symbols: snapshot.symbols,
                    lineage: Rc::new(lineage),
                    rows,
                    origin: Origin::Declared {
                        record: index,
                        side,
                    },
                    means: record.means.is_some(),
                });
            }
        }
    }
    Err(StatisticsError::MissingSource { slot, entry: 0 }.into())
}
/// Original source rows determine the compact source order before allocating sensitivity channels.
fn declared<P: Scalar + 'static>(
    record: &crate::companions::JointRecord,
    index: usize,
    side: usize,
    rows: &[usize],
    limits: JointLimits,
) -> Result<Lineage, OperationError> {
    let model = Model::capture::<P>(&record.model)?;
    let source_map = record
        .selections
        .get(side)
        .ok_or(StatisticsError::Contract)?;
    let mut columns = crate::binding::buffer(rows.len())?;
    for &row in rows {
        columns.push(*source_map.get(row).ok_or(StatisticsError::Contract)?);
    }
    columns.sort_unstable();
    columns.dedup();
    limits.matrix(rows.len(), columns.len())?;
    Ok(Lineage {
        model,
        columns,
        roots: vec![index],
        assurance: crate::MomentAssurance::AffineMoments,
    })
}
