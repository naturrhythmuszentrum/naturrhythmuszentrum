// SPDX-License-Identifier: AGPL-3.0-only
//! Materialize cross-output blocks only from retained checked source lineages.

use super::{
    StatisticsError,
    allocation::RegionData,
    capture::Input,
    contraction::{Work, multiply},
    matrix::Matrix,
    store::{Parameter, Record},
};
use crate::{Dependency, Handle, Heap, JointLimits, MomentRef, OperationError, Value};
use nrz_scalar::{ArithmeticContext, Scalar};

impl Heap {
    /// Publish the full cross covariance of two current value selections.
    ///
    /// Both values must have K source-selection rows or propagated lineage under one
    /// complete model. Equal centers, schemas or distinct IDs establish no independence.
    /// Parameters use the left target's precision; the right target keeps its own format.
    /// # Errors
    /// Refuses missing sources/models/blocks, stale targets, unsupported precision or limits.
    pub fn materialize_cross_covariance<P: Scalar + 'static, Q: Scalar + 'static>(
        &mut self,
        left: Handle<Value, P>,
        right: Handle<Value, Q>,
        limits: JointLimits,
    ) -> Result<MomentRef<P>, OperationError> {
        let context = ArithmeticContext::new()?;
        let first = super::capture::input(self, left, 0, limits, &context)?;
        let second = super::capture::input(self, right, 1, limits, &context)?;
        if first.lineage.model.id != second.lineage.model.id {
            return Err(StatisticsError::MissingJointModel.into());
        }
        let mut lineage = first.lineage.clone();
        lineage.columns.extend_from_slice(&second.lineage.columns);
        lineage.columns.sort_unstable();
        lineage.columns.dedup();
        lineage.roots.extend_from_slice(&second.lineage.roots);
        lineage.roots.sort_unstable();
        lineage.roots.dedup();
        if second.lineage.assurance == super::MomentAssurance::LinearizedMoments {
            lineage.assurance = super::MomentAssurance::LinearizedMoments;
        }
        let work = preflight(&first, &second, lineage.columns.len(), limits)?;
        let mut work = Work::new(work);
        let source = lineage
            .model
            .covariance(&lineage.columns, &context, limits)?;
        let first = align::<P, P>(&first, &lineage.columns, &context)?;
        let second = align::<Q, P>(&second, &lineage.columns, &context)?;
        let intermediate = multiply(&first, &source, &context, &mut work)?;
        let covariance = multiply(&intermediate, &second.transpose()?, &context, &mut work)?;
        let targets = [
            Dependency::from_snapshot(&crate::InputSnapshot::capture(self, left)?),
            Dependency::from_snapshot(&crate::InputSnapshot::capture(self, right)?),
        ];
        let quantities = [
            self.binding_quantity(left)?.clone(),
            self.binding_quantity(right)?.clone(),
        ];
        publish(self, targets, &quantities, lineage, &covariance, &context)
    }
}
/// Prepare both target maps and all independent numerical regions before any visible append.
fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    targets: [Dependency; 2],
    quantities: &[nrz_bindings::Descriptor; 2],
    lineage: super::store::Lineage,
    covariance: &Matrix<P>,
    context: &ArithmeticContext,
) -> Result<MomentRef<P>, OperationError> {
    let descriptors = super::publish::covariance_descriptors(
        &quantities[0],
        &quantities[1],
        covariance.extent.entries(),
    )?;
    let regions = [
        RegionData {
            extent: covariance.extent,
            quantity: nrz_bindings::Descriptor::ratio(),
            values: covariance.centers.clone(),
        },
        RegionData {
            extent: covariance.extent,
            quantity: nrz_bindings::Descriptor::ratio(),
            values: covariance.errors(context)?,
        },
    ];
    heap.statistics
        .records
        .try_reserve(1)
        .map_err(|_| StatisticsError::ResourceLimit)?;
    let prepared = super::allocation::standalone(heap, &regions)?;
    let keys = prepared.records.keys();
    let record = Record {
        targets,
        lineage,
        covariance: Parameter {
            regions: [
                *keys.first().ok_or(StatisticsError::Contract)?,
                *keys.get(1).ok_or(StatisticsError::Contract)?,
            ],
            descriptors,
        },
        sensitivity: None,
        jacobian: None,
        derivative: None,
        means: None,
    };
    let handle = MomentRef {
        scope: heap.scope,
        index: heap.statistics.records.len(),
        marker: std::marker::PhantomData,
    };
    heap.storage.publish(prepared.storage);
    crate::binding::schema::publish(heap, prepared.schemas);
    heap.descriptors.publish(prepared.records);
    heap.statistics.records.push(record);
    Ok(handle)
}
/// Bound complete dense storage and both matrix contractions before their allocations.
fn preflight<P: Scalar, Q: Scalar>(
    left: &Input<P>,
    right: &Input<Q>,
    sources: usize,
    limits: JointLimits,
) -> Result<usize, OperationError> {
    let rows = left.sensitivity.extent.rows();
    let columns = right.sensitivity.extent.rows();
    limits.matrix(rows, columns)?;
    limits.matrix(rows, sources)?;
    limits.matrix(columns, sources)?;
    let work = sources
        .checked_mul(sources)
        .and_then(|count| count.checked_mul(left.lineage.model.terms))
        .and_then(|count| {
            rows.checked_mul(sources)?
                .checked_mul(sources)?
                .checked_add(count)
        })
        .and_then(|count| {
            rows.checked_mul(columns)?
                .checked_mul(sources)?
                .checked_add(count)
        })
        .and_then(|count| count.checked_mul(16))
        .ok_or(StatisticsError::ResourceLimit)?;
    limits.check_work(work)?;
    Ok(work)
}
/// Remap sensitivity columns by exact source index, retaining the complete row selection.
fn align<P: Scalar + 'static, Q: Scalar + 'static>(
    input: &Input<P>,
    sources: &[usize],
    context: &ArithmeticContext,
) -> Result<Matrix<Q>, OperationError> {
    let mut result = Matrix::zeros(input.sensitivity.extent.rows(), sources.len())?;
    for (original, source) in input.lineage.columns.iter().enumerate() {
        let column = sources
            .binary_search(source)
            .map_err(|_| StatisticsError::Contract)?;
        for row in 0..result.extent.rows() {
            let index = input.sensitivity.extent.index(row, original)?;
            let (center, truth) = super::precision::entry(
                *input
                    .sensitivity
                    .centers
                    .get(index)
                    .ok_or(StatisticsError::Contract)?,
                input.sensitivity.entry(row, original)?,
                context,
            )?;
            result.set(row, column, center, truth)?;
        }
    }
    Ok(result)
}
