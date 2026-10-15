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
    pub const fn factor_region(&self) -> &RegionView<'_, P> {
        &self.factor
    }
    /// Nonnegative exact represented weights.
    #[must_use]
    pub const fn weight_region(&self) -> &RegionView<'_, P> {
        &self.weights
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
        Ok(StatisticalView {
            model,
            factor,
            weights,
            stamps: [record.targets[0].stamp, record.targets[1].stamp],
            selections: &record.selections,
            pairings: &record.pairings,
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
    for key in record.regions {
        let region = heap.descriptors.get(key)?;
        if !matches!(region.meaning, Meaning::Statistical) {
            return Err(HeapError::CategoryMismatch);
        }
        heap.storage.resolve(heap.owning_region(region)?)?;
    }
    Ok(())
}
