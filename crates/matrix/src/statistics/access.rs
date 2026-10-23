// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical access validates current targets and retains parameter-specific error targets.

use super::{
    MomentAssurance, MomentRef, StatisticsError,
    matrix::{Matrix, enclosed},
    store::{Parameter, Record},
};
use crate::{Companion, Handle, Heap, HeapError, OperationError, RegionView, Value, ValueStamp};
use nrz_bindings::{EvaluationDescriptor, PairingWitness};
use nrz_scalar::{ArithmeticContext, Scalar};

/// A numerical statistical parameter and its own deterministic evaluation-error region.
#[derive(Debug)]
pub struct ParameterView<'h, P: Scalar> {
    /// Independently owned parameter center region.
    centers: RegionView<'h, P>,
    /// Numerical errors on that parameter, never physical input radii.
    errors: RegionView<'h, P>,
    /// Exact parameter target of the error region.
    target: Handle<Companion, P>,
    /// Paired entry meanings; means instead use their homogeneous region descriptor.
    descriptors: &'h [EvaluationDescriptor],
    /// Explicit parameter role remains meaningful for empty matrices.
    mean: Option<&'h nrz_bindings::Descriptor>,
}
impl<P: Scalar + 'static> ParameterView<'_, P> {
    /// Complete parameter rectangle.
    #[must_use]
    pub const fn extent(&self) -> crate::Extent {
        self.centers.extent()
    }
    /// Exact parameter occupation targeted by the numerical error.
    #[must_use]
    pub const fn error_target(&self) -> Handle<Companion, P> {
        self.target
    }
    /// Read a represented statistical parameter.
    /// # Errors
    /// Refuses an out-of-range row or column.
    pub fn get(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.centers.get(row, column)
    }
    /// Read the certified numerical displacement of this parameter.
    /// # Errors
    /// Refuses an out-of-range row or column.
    pub fn error(&self, row: usize, column: usize) -> Result<P, HeapError> {
        self.errors.get(row, column)
    }
    /// Paired parameter meaning in logical column order.
    /// # Errors
    /// Refuses invalid indices or a mean parameter, whose meaning is homogeneous.
    pub fn descriptor(&self, row: usize, column: usize) -> Result<&PairingWitness, HeapError> {
        Ok(self.error_descriptor(row, column)?.target())
    }
    /// Numerical error retains the entire parameter's dimension and paired sides.
    /// # Errors
    /// Refuses invalid indices or a mean parameter.
    pub fn error_descriptor(
        &self,
        row: usize,
        column: usize,
    ) -> Result<&EvaluationDescriptor, HeapError> {
        self.descriptors
            .get(self.extent().index(row, column)?)
            .ok_or(HeapError::InvalidLayout)
    }
    /// Mean parameters retain the value quantity; paired matrices have per-entry descriptors.
    #[must_use]
    pub const fn mean_descriptor(&self) -> Option<&nrz_bindings::Descriptor> {
        self.mean
    }
}

/// Current moment evidence; this type supplies no deterministic physical-input certificate.
#[derive(Debug)]
pub struct MomentView<'h, P: Scalar> {
    /// Complete immutable source and target relation.
    record: &'h Record,
    /// Full covariance and its own error target.
    covariance: ParameterView<'h, P>,
    /// Retained composed source derivative.
    sensitivity: Option<ParameterView<'h, P>>,
    /// Local derivative with its numerical error.
    jacobian: Option<ParameterView<'h, P>>,
    /// Declared exact affine means, absent for nominal operating points.
    means: Option<ParameterView<'h, P>>,
}
impl<P: Scalar> MomentView<'_, P> {
    /// Statistical law without any promotion to deterministic assurance.
    #[must_use]
    pub const fn assurance(&self) -> MomentAssurance {
        self.record.lineage.assurance
    }
    /// Exact row and column target versions.
    #[must_use]
    pub const fn targets(&self) -> [ValueStamp; 2] {
        [
            self.record.targets[0].stamp(),
            self.record.targets[1].stamp(),
        ]
    }
    /// Both complete target selections, including aliases and column-order maps.
    #[must_use]
    pub const fn target_maps(&self) -> &[crate::Dependency; 2] {
        &self.record.targets
    }
    /// Original immutable joint-model identity.
    #[must_use]
    pub const fn model(&self) -> crate::ModelId {
        self.record.lineage.model.id
    }
    /// Original source IDs; selected columns are identified by `source_indices`.
    #[must_use]
    pub fn source_ids(&self) -> &[crate::SourceId] {
        &self.record.lineage.model.sources
    }
    /// Source coordinates used by the composed sensitivity columns.
    #[must_use]
    pub fn source_indices(&self) -> &[usize] {
        &self.record.lineage.columns
    }
    /// Full covariance, with both parameter axes retained.
    #[must_use]
    pub const fn covariance(&self) -> &ParameterView<'_, P> {
        &self.covariance
    }
    /// Composed source sensitivity and its own evaluation error.
    #[must_use]
    pub const fn sensitivity(&self) -> Option<&ParameterView<'_, P>> {
        self.sensitivity.as_ref()
    }
    /// Checked derivative at the operation's retained input point.
    #[must_use]
    pub const fn jacobian(&self) -> Option<&ParameterView<'_, P>> {
        self.jacobian.as_ref()
    }
    /// Replayed native symbolic derivative identity, distinct from its numerical evaluation.
    #[must_use]
    pub fn checked_sensitivity(&self) -> Option<&nrz_bindings::CheckedSensitivity> {
        self.record.derivative.as_deref()
    }
    /// Mathematical mean parameters; nominal centers never create this declaration.
    #[must_use]
    pub const fn means(&self) -> Option<&ParameterView<'_, P>> {
        self.means.as_ref()
    }
}
impl Heap {
    /// Locate the full propagated moments for this current value version.
    /// # Errors
    /// Refuses stale/foreign values or absent propagated statistical coverage.
    pub fn moment_companion<P: Scalar + 'static>(
        &self,
        value: Handle<Value, P>,
    ) -> Result<MomentRef<P>, OperationError> {
        let stamp = self.value(value)?.stamp();
        let index = self
            .statistics
            .records
            .iter()
            .position(|record| record.targets[0].stamp() == stamp && record.sensitivity.is_some())
            .ok_or(StatisticsError::MissingSource { slot: 0, entry: 0 })?;
        Ok(MomentRef {
            scope: self.scope,
            index,
            marker: std::marker::PhantomData,
        })
    }
    /// Resolve both targets and every original source dependency before lending moments.
    /// # Errors
    /// Refuses wrong owner/precision, stale source rows, missing regions and invalid layouts.
    pub fn moments<P: Scalar + 'static>(
        &self,
        handle: MomentRef<P>,
    ) -> Result<MomentView<'_, P>, OperationError> {
        if handle.scope != self.scope {
            return Err(HeapError::ForeignHandle.into());
        }
        let record = self
            .statistics
            .records
            .get(handle.index)
            .ok_or(HeapError::StaleEvidence)?;
        validate(self, record)?;
        Ok(MomentView {
            record,
            covariance: parameter(self, &record.covariance, false)?,
            sensitivity: record
                .sensitivity
                .as_ref()
                .map(|item| parameter(self, item, false))
                .transpose()?,
            jacobian: record
                .jacobian
                .as_ref()
                .map(|item| parameter(self, item, false))
                .transpose()?,
            means: record
                .means
                .as_ref()
                .map(|item| parameter(self, item, true))
                .transpose()?,
        })
    }
    /// Raw statistical dependency check, safe within the provenance refresh traversal.
    pub(crate) fn validate_statistical_roots(&self, stamp: ValueStamp) -> Result<(), HeapError> {
        let mut remaining = self.limits.layout_work;
        for record in &self.statistics.records {
            charge(&mut remaining)?;
            if record.targets.iter().any(|target| target.stamp() == stamp) {
                roots(self, &record.lineage.roots, &mut remaining)?;
            }
        }
        for record in &self.joint.records {
            charge(&mut remaining)?;
            if record.targets.iter().any(|target| target.stamp == stamp) {
                for target in &record.targets {
                    charge(&mut remaining)?;
                    target.validate(self)?;
                }
            }
        }
        Ok(())
    }
}
/// Check original K rows without reentering the provenance walker that calls this function.
/// K admission forbids attaching these roots after operation history, so their captured raw versions suffice.
fn roots(heap: &Heap, indices: &[usize], remaining: &mut usize) -> Result<(), HeapError> {
    for &index in indices {
        charge(remaining)?;
        let record = heap
            .joint
            .records
            .get(index)
            .ok_or(HeapError::StaleEvidence)?;
        for target in &record.targets {
            charge(remaining)?;
            target.validate(heap)?;
        }
        for &key in &record.regions {
            charge(remaining)?;
            heap.descriptors.get(key)?;
        }
    }
    Ok(())
}
/// Validate raw dependencies without recursively resolving a public statistical view.
pub(crate) fn validate(heap: &Heap, record: &Record) -> Result<(), OperationError> {
    for target in &record.targets {
        target.validate(heap)?;
        match target.precision() {
            nrz_scalar::Precision::F32 => {
                heap.validate_provenance(Handle::<Value, f32>::new(heap.scope, target.key))?;
            }
            nrz_scalar::Precision::F64 => {
                heap.validate_provenance(Handle::<Value, f64>::new(heap.scope, target.key))?;
            }
        }
    }
    let mut remaining = heap.limits.layout_work;
    roots(heap, &record.lineage.roots, &mut remaining)?;
    Ok(())
}
/// Metadata traversal is finite and charged before each record or dependency inspection.
fn charge(remaining: &mut usize) -> Result<(), HeapError> {
    *remaining = remaining.checked_sub(1).ok_or(HeapError::WorkLimit)?;
    Ok(())
}
/// Statistical storage does not masquerade as a deterministic bound companion.
fn parameter<'h, P: Scalar + 'static>(
    heap: &'h Heap,
    record: &'h Parameter,
    mean: bool,
) -> Result<ParameterView<'h, P>, OperationError> {
    let [center, error] = record
        .regions
        .map(|key| Handle::<Companion, P>::new(heap.scope, key));
    let centers = heap.record(center)?;
    let errors = heap.record(error)?;
    if !matches!(centers.meaning, crate::heap::records::Meaning::Statistical)
        || !matches!(errors.meaning, crate::heap::records::Meaning::Statistical)
    {
        return Err(StatisticsError::Contract.into());
    }
    Ok(ParameterView {
        centers: heap.region_view(centers)?,
        errors: heap.region_view(errors)?,
        target: center,
        descriptors: &record.descriptors,
        mean: mean.then(|| heap.binding_quantity(center)).transpose()?,
    })
}
/// Recover the complete mathematical parameter enclosure from its own stored error.
pub(crate) fn matrix<P: Scalar + 'static>(
    heap: &Heap,
    record: &Parameter,
    context: &ArithmeticContext,
) -> Result<Matrix<P>, OperationError> {
    let view = parameter::<P>(heap, record, false)?;
    let mut result = Matrix::zeros(view.extent().rows(), view.extent().columns())?;
    for column in 0..view.extent().columns() {
        for row in 0..view.extent().rows() {
            let center = view.get(row, column)?;
            result.set(
                row,
                column,
                center,
                enclosed(center, view.error(row, column)?, context)?,
            )?;
        }
    }
    Ok(result)
}

impl Heap {
    /// Read a base stamp through already checked owner indirection only.
    pub(crate) fn value_stamp_raw<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<ValueStamp, HeapError> {
        let base = self.base_handle(handle)?;
        if let crate::heap::records::Meaning::Value { stamp, .. } = self.record(base)?.meaning {
            Ok(stamp)
        } else {
            Err(HeapError::CategoryMismatch)
        }
    }
}
