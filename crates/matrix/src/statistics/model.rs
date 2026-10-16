// SPDX-License-Identifier: AGPL-3.0-only
//! Preserve original immutable models and source identities through propagation.

use super::{StatisticsError, matrix::Matrix};
use crate::{JointLimits, JointModel, ModelId, OperationError, SourceId};
use nrz_scalar::{ArithmeticContext, Scalar};
use nrz_units::Descriptor;
use std::{any::Any, rc::Rc};

/// Original precision-erased source model; propagation never invents sources.
#[derive(Clone)]
pub(crate) struct Model {
    /// Immutable model retaining its sealed precision.
    pub(crate) data: Rc<dyn Any>,
    /// Shared identity checked before joining lineages.
    pub(crate) id: ModelId,
    /// Ordered source identities.
    pub(crate) sources: Vec<SourceId>,
    /// Original source difference descriptors.
    pub(crate) quantities: Vec<Descriptor>,
    /// Terms required for one source covariance entry.
    pub(crate) terms: usize,
}
impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The precision-erased original model deliberately remains opaque.
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}
impl Model {
    /// Retain K's admitted model after validating its target-side rows.
    pub(crate) fn capture<P: Scalar + 'static>(data: &Rc<dyn Any>) -> Result<Self, OperationError> {
        let model = data
            .downcast_ref::<JointModel<P>>()
            .ok_or(StatisticsError::Contract)?;
        let terms = match model.representation() {
            crate::CovarianceRepresentation::Factor(factor) => factor.columns().max(1),
            crate::CovarianceRepresentation::Dense(_) => 1,
        };
        Ok(Self {
            data: Rc::clone(data),
            id: model.id(),
            sources: model.sources().iter().map(crate::Source::id).collect(),
            quantities: model
                .sources()
                .iter()
                .map(|source| source.descriptor().clone())
                .collect(),
            terms,
        })
    }
    /// Require the entire selected block before numerical allocation.
    pub(crate) fn require<P: Scalar + 'static>(
        &self,
        sources: &[usize],
        limits: JointLimits,
    ) -> Result<(), OperationError> {
        let model = self
            .data
            .downcast_ref::<JointModel<P>>()
            .ok_or(StatisticsError::Contract)?;
        model.require_blocks(sources, sources, limits)?;
        Ok(())
    }
    /// Materialize complete joint covariance; missing blocks never become zeros.
    pub(crate) fn covariance<P: Scalar + 'static>(
        &self,
        sources: &[usize],
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Matrix<P>, OperationError> {
        self.require::<P>(sources, limits)?;
        let model = self
            .data
            .downcast_ref::<JointModel<P>>()
            .ok_or(StatisticsError::Contract)?;
        let mut result = Matrix::zeros(sources.len(), sources.len())?;
        for (column, &source_column) in sources.iter().enumerate() {
            for (row, &source_row) in sources.iter().enumerate() {
                let truth = model.covariance(source_row, source_column, context, limits)?;
                result.set(row, column, truth.lower().get(), truth)?;
            }
        }
        Ok(result)
    }
}
