// SPDX-License-Identifier: AGPL-3.0-only
//! Materialize cross-output blocks only from retained checked source lineages.

use super::inventory::{Role, Run};
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
        let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
        self.materialize_cross_covariance_with_inventory_budget(left, right, limits, &budget)
    }
    /// Publish cross covariance with explicit staged and retained inventory byte ceilings.
    /// # Errors
    /// Refuses missing coverage, foreign models, stale selections or metadata exhaustion before numerical allocation.
    pub fn materialize_cross_covariance_with_inventory_budget<
        P: Scalar + 'static,
        Q: Scalar + 'static,
    >(
        &mut self,
        left: Handle<Value, P>,
        right: Handle<Value, Q>,
        limits: JointLimits,
        budget: &nrz_bindings::Budget,
    ) -> Result<MomentRef<P>, OperationError> {
        let bindings = vec![
            super::capture::inspect(self, left, 0, limits)?,
            super::capture::inspect(self, right, 1, limits)?,
        ];
        let trace =
            crate::execution::Trace::new(crate::ExecutionId(crate::heap::identity::identity()?));
        let run = Run::new::<P>(bindings, None, &trace, budget)?;
        let first = run.binding(0)?;
        let second = run.binding(1)?;
        let work = preflight(first, second, run.columns().len(), limits)?;
        let mut lineage = (*first.lineage).clone();
        lineage.columns = run.columns().to_vec();
        lineage.roots.extend_from_slice(&second.lineage.roots);
        lineage.roots.sort_unstable();
        lineage.roots.dedup();
        if second.lineage.assurance == super::MomentAssurance::LinearizedMoments {
            lineage.assurance = super::MomentAssurance::LinearizedMoments;
        }
        lineage.model.require(&lineage.columns, limits)?;
        run.backend_allowance(work, size_of::<P>())?;
        run.law(lineage.assurance);
        let context = ArithmeticContext::new()?;
        let covariance = evaluate::<P, Q>(self, &run, limits, &context, work)?;
        let targets = [first.dependency.clone(), second.dependency.clone()];
        let quantities = [
            self.binding_quantity(left)?.clone(),
            self.binding_quantity(right)?.clone(),
        ];
        publish(
            self,
            targets,
            &quantities,
            lineage,
            &covariance,
            &context,
            &run,
        )
    }
}
/// All selected captures and temporary contractions drop before publication copies are allocated.
fn evaluate<P: Scalar + 'static, Q: Scalar + 'static>(
    heap: &Heap,
    run: &Run,
    limits: JointLimits,
    context: &ArithmeticContext,
    work: usize,
) -> Result<Matrix<P>, OperationError> {
    let first = super::capture::input::<P>(heap, 0, run, context)?;
    let aligned_left = align::<P, P>(&first, run.columns(), context, run, false)?;
    drop(first);
    let mut source = run.matrix(Role::Model, 0)?;
    run.binding(0)?
        .lineage
        .model
        .covariance_into(run.columns(), context, limits, &mut source)?;
    let mut work = Work::new(work);
    let intermediate = multiply(
        &aligned_left,
        &source,
        context,
        &mut work,
        run.request(Role::CovarianceProduct, 0),
    )?;
    drop(aligned_left);
    drop(source);
    let second = super::capture::input::<Q>(heap, 1, run, context)?;
    let transposed = align::<Q, P>(&second, run.columns(), context, run, true)?;
    drop(second);
    multiply(
        &intermediate,
        &transposed,
        context,
        &mut work,
        run.request(Role::Covariance, 0),
    )?
    .into_candidate()
}
/// Prepare both target maps and all independent numerical regions before any visible append.
fn publish<P: Scalar + 'static>(
    heap: &mut Heap,
    targets: [Dependency; 2],
    quantities: &[nrz_bindings::Descriptor; 2],
    lineage: super::store::Lineage,
    covariance: &Matrix<P>,
    context: &ArithmeticContext,
    run: &Run,
) -> Result<MomentRef<P>, OperationError> {
    if !run.matches(covariance, Role::Covariance, 0) {
        return Err(crate::PrivateAllocationError::ForeignInventory.into());
    }
    run.validate_lineage(&lineage)?;
    let descriptors = super::publish::covariance_descriptors(
        &quantities[0],
        &quantities[1],
        covariance.extent.entries(),
    )?;
    let mut target_symbols = Vec::new();
    for target in &targets {
        target_symbols.push(crate::binding::parameters::dependency_targets(
            heap, target,
        )?);
    }
    let declaration = crate::binding::parameters::paired(
        covariance.extent,
        &descriptors,
        &target_symbols[0],
        &target_symbols[1],
    )?;
    let error_symbol = crate::binding::parameters::errors(&declaration)?;
    let [centers, errors] = Run::copies(covariance, 0, context)?;
    let regions = [
        RegionData {
            extent: covariance.extent,
            quantity: None,
            declaration,
            values: centers,
        },
        RegionData {
            extent: covariance.extent,
            quantity: None,
            declaration: error_symbol,
            values: errors,
        },
    ];
    heap.statistics
        .records
        .try_reserve(1)
        .map_err(|_| StatisticsError::ResourceLimit)?;
    let prepared = super::allocation::standalone(heap, &regions)?;
    drop(regions);
    let inventory = run.finish(None, |role, index| run.matches(covariance, role, index))?;
    let keys = prepared.records.keys();
    let record = Record {
        inventory,
        targets,
        lineage: std::rc::Rc::new(lineage),
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
    crate::binding::parameters::validate(&prepared.covers, &prepared.records)?;
    heap.storage.publish(prepared.storage);
    crate::binding::schema::publish(heap, prepared.schemas);
    heap.descriptors.publish(prepared.records);
    heap.symbols.publish(prepared.covers);
    heap.statistics.records.push(record);
    Ok(handle)
}
/// Bound complete dense storage and both matrix contractions before their allocations.
fn preflight(
    left: &super::capture::InputBinding,
    right: &super::capture::InputBinding,
    sources: usize,
    limits: JointLimits,
) -> Result<usize, OperationError> {
    let rows = left.rows.len();
    let columns = right.rows.len();
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
    run: &Run,
    transpose: bool,
) -> Result<Matrix<Q>, OperationError> {
    let rows = input.sensitivity.extent.rows();
    let (role, extent) = if transpose {
        (Role::Transpose, crate::Extent::new(sources.len(), rows)?)
    } else {
        (Role::Aligned, crate::Extent::new(rows, sources.len())?)
    };
    let mut result = run.matrix_at(role, 0, extent)?;
    if result.extent.entries() == 0 {
        return Ok(result);
    }
    for (original, source) in input.lineage.columns.iter().enumerate() {
        let column = sources
            .binary_search(source)
            .map_err(|_| StatisticsError::Contract)?;
        for row in 0..rows {
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
            let target = if transpose {
                (column, row)
            } else {
                (row, column)
            };
            result.set(target.0, target.1, center, truth)?;
        }
    }
    Ok(result)
}
