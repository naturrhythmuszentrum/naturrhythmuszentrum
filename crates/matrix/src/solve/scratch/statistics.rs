// SPDX-License-Identifier: AGPL-3.0-only
//! Solve moment contractions retain the exact original model and selected source order.

use super::{Stage, plan::Specification, schedule::specification};
use crate::{Extent, ModelId, MomentAssurance, OperationError};
use nrz_scalar::Scalar;
use std::{any::Any, rc::Rc};

/// Original source model and ordered selection, shared independently of numerical temporary arrays.
pub(super) struct Binding {
    /// Original immutable model allocation; never rebuilt from rounded values.
    pub(super) model: Rc<dyn Any>,
    /// Public model identity corresponding to that original owner.
    pub(super) id: ModelId,
    /// Selected original source columns in exact order.
    pub(super) columns: Vec<usize>,
    /// Original companion records establishing source lineage.
    pub(super) roots: Vec<usize>,
    /// Affine or explicitly linearized statistical law.
    pub(super) assurance: MomentAssurance,
    /// Existing complete statistical work admission, also reserving opaque backend storage.
    pub(super) work: usize,
    /// Whether all exact affine input means are present.
    pub(super) means: bool,
}
impl Binding {
    /// Predict metadata before allocating either the binding or its selection copies.
    pub(super) fn predicted<P: Scalar>(
        plan: &crate::statistics::Plan<P>,
    ) -> Result<usize, OperationError> {
        plan.lineage.as_ref().map_or(Ok(0), |lineage| {
            bytes(lineage.columns.len(), lineage.roots.len())
        })
    }
    /// Keep original owners and copy only the small ordered selection metadata.
    pub(super) fn capture<P: Scalar>(
        plan: &crate::statistics::Plan<P>,
    ) -> Result<Option<Box<Self>>, OperationError> {
        let Some(lineage) = &plan.lineage else {
            return Ok(None);
        };
        let mut columns = crate::binding::buffer(lineage.columns.len())?;
        columns.extend_from_slice(&lineage.columns);
        let mut roots = crate::binding::buffer(lineage.roots.len())?;
        roots.extend_from_slice(&lineage.roots);
        Ok(Some(Box::new(Self {
            model: Rc::clone(&lineage.model.data),
            id: lineage.model.id,
            columns,
            roots,
            assurance: lineage.assurance,
            work: plan.work,
            means: has_means(plan),
        })))
    }
    /// Count actual selection capacities and the independently owned binding header.
    pub(super) fn owned_bytes(&self) -> Result<usize, OperationError> {
        bytes(self.columns.capacity(), self.roots.capacity())
    }
    /// Publication compares the original model owner and every selected source/root identity.
    pub(super) fn matches<P: Scalar>(&self, plan: &crate::statistics::Plan<P>) -> bool {
        plan.lineage.as_ref().is_some_and(|lineage| {
            Rc::ptr_eq(&self.model, &lineage.model.data)
                && self.id == lineage.model.id
                && self.columns == lineage.columns
                && self.roots == lineage.roots
                && self.assurance == lineage.assurance
                && self.work == plan.work
                && self.means == has_means(plan)
        })
    }
}
/// Metadata addition is checked before either selection is allocated.
fn bytes(columns: usize, roots: usize) -> Result<usize, OperationError> {
    columns
        .checked_add(roots)
        .and_then(|n| n.checked_mul(size_of::<usize>()))
        .and_then(|n| n.checked_add(size_of::<Binding>()))
        .ok_or(OperationError::ResourceLimit)
}
/// Means are materialized only when the exact affine law and every declared input mean permit them.
pub(super) fn has_means<P: Scalar>(plan: &crate::statistics::Plan<P>) -> bool {
    plan.lineage
        .as_ref()
        .is_some_and(|lineage| lineage.assurance == MomentAssurance::AffineMoments)
        && plan.inputs.iter().all(|input| input.means.is_some())
}
impl Stage {
    /// Contraction indices have distinct mathematical products under one original source relation.
    pub(crate) const fn is_contraction(self) -> bool {
        matches!(
            self,
            Self::ContractionLeft
                | Self::ContractionRight
                | Self::ContractionResult
                | Self::ContractionCenters
                | Self::ContractionTruth
        )
    }
    /// Statistical stages are required independently of the deterministic recovery prefix.
    pub(crate) const fn is_statistical(self) -> bool {
        self.is_contraction()
            || matches!(
                self,
                Self::StatisticalInputs
                    | Self::StatisticalInputsTruth
                    | Self::StatisticalModel
                    | Self::StatisticalModelTruth
                    | Self::StatisticalTranspose
                    | Self::StatisticalTransposeTruth
                    | Self::StatisticalMeans
                    | Self::StatisticalMeansTruth
                    | Self::ParameterCenters
                    | Self::ParameterErrors
            )
    }
    /// The intermediate covariance product is consumed; final sensitivity, covariance and means remain live.
    pub(crate) const fn is_candidate_at(self, index: Option<usize>) -> bool {
        self.is_candidate()
            || (matches!(self, Self::ContractionCenters | Self::ContractionTruth)
                && matches!(index, Some(0 | 2 | 3)))
    }
}
/// The complete optional mean path is fixed before numerical allocation.
pub(super) fn count<P: Scalar>(plan: &crate::statistics::Plan<P>) -> usize {
    if plan.lineage.is_none() {
        0
    } else if has_means(plan) {
        36
    } else {
        27
    }
}
/// All rectangles follow the original checked solution, input order and common source columns.
pub(super) fn specifications<P: Scalar>(
    contract: &super::super::SolveContract,
    plan: &crate::statistics::Plan<P>,
    result: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    let Some(lineage) = &plan.lineage else {
        return Ok(());
    };
    let inputs = contract.inputs.iter().try_fold(0_usize, |n, v| {
        n.checked_add(v.shape().extent().entries())
            .ok_or(OperationError::ResourceLimit)
    })?;
    let output = contract.solution.shape().extent().entries();
    let sources = lineage.columns.len();
    for (center, truth, rows, columns) in [
        (
            Stage::StatisticalInputs,
            Stage::StatisticalInputsTruth,
            inputs,
            sources,
        ),
        (
            Stage::StatisticalModel,
            Stage::StatisticalModelTruth,
            sources,
            sources,
        ),
        (
            Stage::StatisticalTranspose,
            Stage::StatisticalTransposeTruth,
            sources,
            output,
        ),
    ] {
        pair(result, [center, truth], None, Extent::new(rows, columns)?);
    }
    contraction(result, 0, [output, inputs, sources])?;
    contraction(result, 1, [output, sources, sources])?;
    contraction(result, 2, [output, sources, output])?;
    parameter(result, 0, Extent::new(output, output)?);
    parameter(result, 1, Extent::new(output, sources)?);
    parameter(result, 2, Extent::new(output, inputs)?);
    if has_means(plan) {
        pair(
            result,
            [Stage::StatisticalMeans, Stage::StatisticalMeansTruth],
            None,
            Extent::new(inputs, 1)?,
        );
        contraction(result, 3, [output, inputs, 1])?;
        parameter(result, 3, Extent::new(output, 1)?);
    }
    Ok(())
}
/// Backend operands/results and independent center/enclosure owners are separate requests.
fn contraction(
    result: &mut Vec<Specification>,
    index: usize,
    [rows, inner, columns]: [usize; 3],
) -> Result<(), OperationError> {
    result.push(specification(
        Stage::ContractionLeft,
        Some(index),
        Extent::new(rows, inner)?,
    ));
    result.push(specification(
        Stage::ContractionRight,
        Some(index),
        Extent::new(inner, columns)?,
    ));
    let extent = Extent::new(rows, columns)?;
    result.push(specification(Stage::ContractionResult, Some(index), extent));
    pair(
        result,
        [Stage::ContractionCenters, Stage::ContractionTruth],
        Some(index),
        extent,
    );
    Ok(())
}
/// Publication copies have the same parameter identity and distinct numerical-error targets.
fn parameter(result: &mut Vec<Specification>, index: usize, extent: Extent) {
    pair(
        result,
        [Stage::ParameterCenters, Stage::ParameterErrors],
        Some(index),
        extent,
    );
}
/// One logical parameter matrix has separately owned center and truth/error channels.
fn pair(result: &mut Vec<Specification>, stages: [Stage; 2], index: Option<usize>, extent: Extent) {
    for stage in stages {
        result.push(specification(stage, index, extent));
    }
}

impl super::Run {
    /// Construct parameter pairs from their declared shape, checking both roles before allocation.
    pub(crate) fn matrix<P: Scalar + 'static>(
        &self,
        centers: Stage,
        truth: Stage,
        index: Option<usize>,
    ) -> Result<crate::statistics::Matrix<P>, OperationError> {
        let extent = self.extent(centers, index)?;
        if self.extent(truth, index)? != extent {
            return Err(crate::PrivateAllocationError::Representation.into());
        }
        let mut values = self.buffer(centers, index)?;
        let mut enclosures = self.buffer(truth, index)?;
        for _ in 0..extent.entries() {
            values.push(P::ZERO)?;
            enclosures.push(nrz_scalar::Enclosure::singleton(
                nrz_scalar::Finite::<P>::ZERO,
            ))?;
        }
        Ok(crate::statistics::Matrix {
            extent,
            centers: values,
            truth: enclosures,
        })
    }
    /// Borrow the exact rectangle without drawing authority or allowing any allocation.
    pub(crate) fn extent(
        &self,
        stage: Stage,
        index: Option<usize>,
    ) -> Result<Extent, OperationError> {
        self.0
            .specifications
            .iter()
            .find(|entry| entry.stage == stage && entry.iteration == index)
            .map(|entry| entry.extent)
            .ok_or_else(|| crate::PrivateAllocationError::UnplannedMeaning.into())
    }
    /// Publication copies retain the original solve owner through the borrowed parameter centers.
    pub(crate) fn from_parameter<P: Scalar>(matrix: &crate::statistics::Matrix<P>) -> Option<Self> {
        matrix
            .centers
            .solve_plan()
            .map(|plan| Self(Rc::clone(plan)))
    }
}

impl std::fmt::Debug for Binding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Binding")
            .field("model", &self.id)
            .field("columns", &self.columns)
            .field("roots", &self.roots)
            .field("assurance", &self.assurance)
            .finish_non_exhaustive()
    }
}
