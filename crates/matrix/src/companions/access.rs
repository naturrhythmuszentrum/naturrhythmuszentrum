// SPDX-License-Identifier: AGPL-3.0-only
//! Every statistical evidence access resolves both exact target versions.

use super::JointRecord;
use crate::heap::records::Meaning;
use crate::{
    Companion, Handle, Heap, HeapError, JointError, JointModel, RegionView, Value, ValueStamp,
};
use nrz_scalar::Scalar;
use std::marker::PhantomData;

/// Opaque heap-scoped locator for one immutable statistical companion relation.
#[derive(Clone, Copy, Debug)]
pub struct CompanionRef<P: Scalar> {
    /// Owner identity.
    pub(crate) scope: u64,
    /// Append-only record index.
    pub(crate) index: usize,
    /// Sealed precision.
    pub(crate) marker: PhantomData<P>,
}
/// Current statistical evidence; no deterministic physical-input bound is implied.
#[derive(Debug)]
pub struct StatisticalView<'h, P: Scalar> {
    /// Declared immutable finite-moment model.
    model: &'h JointModel<P>,
    /// Exact factor coefficients in their own heap region.
    factor: RegionView<'h, P>,
    /// Exact nonnegative weights in their own heap region.
    weights: RegionView<'h, P>,
    /// Ordered physical targets, already validated for this borrow.
    stamps: [ValueStamp; 2],
    /// Logical selections of the common ordered source vector.
    selections: &'h [Vec<usize>; 2],
    /// Declared mathematical mean parameters, absent for nominal operating points.
    means: Option<&'h super::MeanParameters<P>>,
    /// Independent numerical mean-error regions in row/column target order.
    mean_errors: Vec<RegionView<'h, P>>,
    /// Independent error descriptors on full source-covariance parameters.
    evaluation: &'h [nrz_units::EvaluationDescriptor],
    /// Companion identity, distinct from every physical value target.
    identity: CompanionRef<P>,
    /// Independent paired uncertainty descriptors.
    pairings: &'h [nrz_units::PairingWitness],
}
impl<P: Scalar> StatisticalView<'_, P> {
    /// Conditional finite-moment model, never a deterministic assurance token.
    #[must_use]
    pub const fn model(&self) -> &JointModel<P> {
        self.model
    }
    /// Factor coefficients in normalized source coordinates.
    #[must_use]
    pub fn factor_region(&self) -> Option<&RegionView<'_, P>> {
        matches!(
            self.model.representation(),
            crate::CovarianceRepresentation::Factor(_)
        )
        .then_some(&self.factor)
    }
    /// Nonnegative exact represented weights.
    #[must_use]
    pub fn weight_region(&self) -> Option<&RegionView<'_, P>> {
        matches!(
            self.model.representation(),
            crate::CovarianceRepresentation::Factor(_)
        )
        .then_some(&self.weights)
    }
    /// Dense covariance parameters, when the model has a dense witness.
    #[must_use]
    pub fn covariance_region(&self) -> Option<super::CovarianceParameterView<'_, P>> {
        matches!(
            self.model.representation(),
            crate::CovarianceRepresentation::Dense(_)
        )
        .then_some(super::CovarianceParameterView {
            region: &self.factor,
            descriptors: self.evaluation,
        })
    }
    /// Numerical errors target the covariance parameters themselves.
    #[must_use]
    pub fn parameter_errors(&self) -> Option<super::ParameterErrorView<'_, P>> {
        matches!(
            self.model.representation(),
            crate::CovarianceRepresentation::Dense(_)
        )
        .then_some(super::ParameterErrorView {
            region: &self.weights,
            target: self.identity,
            descriptors: self.evaluation,
        })
    }
    /// Declared means; `None` means the centers are only operating points.
    #[must_use]
    pub const fn means(&self) -> Option<&super::MeanParameters<P>> {
        self.means
    }
    /// Numerical mean-parameter errors, never bounds on physical source excursions.
    #[must_use]
    pub fn mean_error_regions(&self) -> &[RegionView<'_, P>] {
        &self.mean_errors
    }
    /// Both target identities and versions in row/column order.
    #[must_use]
    pub const fn targets(&self) -> [ValueStamp; 2] {
        self.stamps
    }
    /// Source maps preserve repetitions and dependence.
    #[must_use]
    pub const fn source_maps(&self) -> &[Vec<usize>; 2] {
        self.selections
    }
    /// Covariance entry meanings in logical column order.
    #[must_use]
    pub const fn pairings(&self) -> &[nrz_units::PairingWitness] {
        self.pairings
    }
}
impl Heap {
    /// Resolve a joint companion and both targets before lending statistical evidence.
    ///
    /// # Errors
    /// Refuses foreign locators, precision mismatch or stale target/model storage.
    pub fn statistical<P: Scalar + 'static>(
        &self,
        handle: CompanionRef<P>,
    ) -> Result<StatisticalView<'_, P>, JointError> {
        if handle.scope != self.scope {
            return Err(HeapError::ForeignHandle.into());
        }
        let record = self
            .joint
            .records
            .get(handle.index)
            .ok_or(HeapError::StaleEvidence)?;
        validate_record(self, record)?;
        let model = record
            .model
            .downcast_ref::<JointModel<P>>()
            .ok_or(HeapError::PrecisionMismatch)?;
        let factor = self.region_view(
            self.record(Handle::<Companion, P>::new(self.scope, record.regions[0]))?,
        )?;
        let weights = self.region_view(
            self.record(Handle::<Companion, P>::new(self.scope, record.regions[1]))?,
        )?;
        let means = record
            .means
            .as_ref()
            .map(|value| {
                value
                    .downcast_ref::<super::MeanParameters<P>>()
                    .ok_or(HeapError::PrecisionMismatch)
            })
            .transpose()?;
        let mut mean_errors = Vec::new();
        for &key in record.regions.iter().skip(2) {
            mean_errors.push(
                self.region_view(self.record(Handle::<Companion, P>::new(self.scope, key))?)?,
            );
        }
        Ok(StatisticalView {
            model,
            factor,
            weights,
            means,
            mean_errors,
            stamps: [record.targets[0].stamp, record.targets[1].stamp],
            selections: &record.selections,
            pairings: &record.pairings,
            evaluation: &record.evaluation,
            identity: handle,
        })
    }
    /// Explicit statistical coverage inventory for this current base version.
    ///
    /// # Errors
    /// Refuses invalid value handles or stale statistical target relations.
    pub fn joint_companions<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<Vec<CompanionRef<P>>, JointError> {
        let stamp = self.value(handle)?.stamp();
        Ok(self
            .joint
            .records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                record
                    .targets
                    .iter()
                    .any(|target| target.stamp == stamp)
                    .then_some(CompanionRef {
                        scope: self.scope,
                        index,
                        marker: PhantomData,
                    })
            })
            .collect())
    }
    /// Hook used by value access; compare raw targets to avoid recursion.
    pub(crate) fn validate_joint_value<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<(), HeapError> {
        let base = self.base_handle(handle)?;
        let Meaning::Value { stamp, .. } = self.record(base)?.meaning else {
            return Err(HeapError::CategoryMismatch);
        };
        for record in &self.joint.records {
            if record.targets.iter().any(|target| target.stamp == stamp) {
                validate_record(self, record)?;
            }
        }
        Ok(())
    }
    /// Existing recipes cannot silently discard a carried statistical relation.
    pub(crate) fn refuse_joint_operation<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<(), crate::OperationError> {
        let stamp = self.value(handle)?.stamp();
        if self
            .joint
            .records
            .iter()
            .any(|record| record.targets.iter().any(|target| target.stamp == stamp))
        {
            return Err(HeapError::MissingCoverage.into());
        }
        Ok(())
    }
}
/// Validate immutable target and independent statistical-region links.
fn validate_record(heap: &Heap, record: &JointRecord) -> Result<(), HeapError> {
    for target in &record.targets {
        target.validate(heap)?;
    }
    for &key in &record.regions {
        let region = heap.descriptors.get(key)?;
        if !matches!(region.meaning, Meaning::Statistical) {
            return Err(HeapError::CategoryMismatch);
        }
        heap.storage.resolve(heap.owning_region(region)?)?;
    }
    Ok(())
}
