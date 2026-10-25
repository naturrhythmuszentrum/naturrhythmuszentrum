// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare independent factor regions and all relation metadata before publication.

use super::{CompanionRef, JointRecord, TargetMap};
use crate::binding::schema::{BindingSchema, intern};
use crate::heap::{
    identity::Key,
    records::{Location, Meaning, Record},
};
use crate::storage::{Format, Region};
use crate::{Extent, Heap, JointError, JointLimits, JointModel, Layout};
use nrz_bindings::{PairingWitness, UncertaintyMode};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Unpublished two-sided source relation over existing versioned value selections.
#[derive(Clone, Debug)]
pub struct CompanionSetDraft<P: Scalar> {
    /// Immutable admitted joint model.
    model: JointModel<P>,
    /// Optional declared mathematical means with parameter error regions.
    means: Option<super::MeanParameters<P>>,
    /// Covariance row and column target maps.
    targets: [TargetMap<P>; 2],
    /// Corresponding ordered source coordinates.
    selections: [Vec<usize>; 2],
}
impl<P: Scalar> CompanionSetDraft<P> {
    /// Describe both target sides and source maps; publication validates every entry.
    #[must_use]
    pub const fn new(
        model: JointModel<P>,
        targets: [TargetMap<P>; 2],
        selections: [Vec<usize>; 2],
    ) -> Self {
        Self {
            model,
            targets,
            selections,
            means: None,
        }
    }
}
impl<P: Scalar + 'static> CompanionSetDraft<P> {
    /// Declare mathematical mean parameters separately from the represented centers.
    ///
    /// # Errors
    /// Refuses stale targets, mismatched lengths, unsupported radius descriptors,
    /// exhausted work or nonfinite outward center-parameter displacement.
    pub fn with_means(
        mut self,
        heap: &Heap,
        means: [Vec<nrz_scalar::Enclosure<P>>; 2],
        context: &nrz_scalar::ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Self, JointError> {
        self.means = Some(super::means::prepare(
            heap,
            &self.targets,
            means,
            context,
            limits,
        )?);
        Ok(self)
    }
}
impl Heap {
    /// Publish factor regions and a complete statistical coverage inventory atomically.
    ///
    /// Targets are operating centers, not asserted expectations. Source fluctuations
    /// are zero-mean. Attaching after any operation history refuses, since those
    /// historical recipes did not capture this statistical dependency. Selected original
    /// readings retain their own quantity and complete value/companion version closure.
    ///
    /// # Errors
    /// Refuses stale/wrong targets, inconsistent source maps, unsupported descriptors,
    /// operation history and exhausted storage/work capacity. No partial region escapes.
    pub fn publish_joint<P: Scalar + 'static>(
        &mut self,
        draft: CompanionSetDraft<P>,
        limits: JointLimits,
    ) -> Result<CompanionRef<P>, JointError> {
        if self.provenance_usage().records() != 0 {
            return Err(JointError::UnsupportedMode);
        }
        let pairings = validate_draft(self, &draft, limits)?;
        let index = self.joint.records.len();
        if index >= self.limits.descriptors || index >= limits.entries {
            return Err(JointError::ResourceLimit);
        }
        self.joint
            .records
            .try_reserve(1)
            .map_err(|_| JointError::ResourceLimit)?;
        let evaluation = evaluation_descriptors(&draft.model)?;
        let declarations =
            super::declarations::prepare(self, &draft.model, &draft.targets, draft.means.as_ref())?;
        let prepared = prepare_regions(
            self,
            draft.model.representation(),
            draft.means.as_ref(),
            &declarations,
        )?;
        crate::binding::parameters::validate(&prepared.covers, &prepared.records)?;
        let [row_target, column_target] = draft.targets;
        let record = JointRecord {
            targets: [row_target.target, column_target.target],
            selections: draft.selections,
            model: Rc::new(draft.model),
            means: draft
                .means
                .map(|value| Rc::new(value) as Rc<dyn std::any::Any>),
            regions: prepared.keys,
            pairings,
            evaluation,
        };
        self.storage.publish(prepared.storage);
        crate::binding::schema::publish(self, prepared.schemas);
        self.descriptors.publish(prepared.records);
        self.symbols.publish(prepared.covers);
        self.joint.records.push(record);
        Ok(CompanionRef {
            scope: self.scope,
            index,
            marker: PhantomData,
        })
    }
}
/// Validate concrete source schemas and complete ordered target maps before allocation.
fn validate_draft<P: Scalar + 'static>(
    heap: &Heap,
    draft: &CompanionSetDraft<P>,
    limits: JointLimits,
) -> Result<Vec<PairingWitness>, JointError> {
    if draft.targets[0].stamp() == draft.targets[1].stamp()
        && (draft.targets[0].layout() != draft.targets[1].layout()
            || draft.selections[0] != draft.selections[1])
    {
        return Err(JointError::SourceConflict);
    }
    let count = limits.matrix(draft.selections[0].len(), draft.selections[1].len())?;
    let selections = draft.selections[0]
        .len()
        .checked_add(draft.selections[1].len())
        .ok_or(JointError::ResourceLimit)?;
    let work = selections
        .checked_mul(selections)
        .and_then(|value| {
            heap.joint
                .records
                .len()
                .checked_mul(selections.max(1))
                .and_then(|old| value.checked_add(old))
        })
        .and_then(|value| value.checked_add(count))
        .ok_or(JointError::ResourceLimit)?;
    limits.check_work(work)?;
    for (side, (target, selection)) in draft.targets.iter().zip(&draft.selections).enumerate() {
        validate_selection(heap, &draft.model, target, selection)?;
        validate_existing(heap, draft, side)?;
    }
    let mut pairings = Vec::new();
    for &column in &draft.selections[1] {
        for &row in &draft.selections[0] {
            pairings.push(PairingWitness::uncertainty(
                UncertaintyMode::Covariance,
                draft.model.sources()[row].descriptor(),
                Some(draft.model.sources()[column].descriptor()),
                None,
            )?);
        }
    }
    Ok(pairings)
}
/// Assemble independently owned statistical descriptor records with checked shapes.
fn region_records<P: Scalar + 'static>(
    schemas: &[usize],
    extents: &[Extent],
    arenas: &[Key],
    epoch: u64,
) -> Result<Vec<Record>, JointError> {
    let mut records = Vec::new();
    for ((&schema, &extent), &arena) in schemas.iter().zip(extents).zip(arenas) {
        records.push(Record {
            schema,
            layout: Layout::contiguous(extent)?,
            location: Location::Owned(Region {
                epoch,
                arena,
                format: Format::of::<P>(),
                offset: 0,
                length: extent.entries(),
            }),
            meaning: Meaning::Statistical,
        });
    }
    Ok(records)
}

/// Completely prepared independent region occupations.
struct PreparedRegions {
    /// Private numerical allocations.
    storage: crate::storage::PreparedStorage,
    /// Reserved immutable schema interpretations.
    schemas: Vec<BindingSchema>,
    /// Fully bound descriptor slots.
    records: crate::heap::slots::PreparedSlots<Record>,
    /// Future factor and weight locators.
    keys: Vec<Key>,
    /// Full per-entry parameter declarations.
    covers: Vec<crate::binding::symbols::RegionCover>,
}
/// Prepare every allocation before the metadata publication step.
fn prepare_regions<P: Scalar + 'static>(
    heap: &mut Heap,
    covariance: &crate::CovarianceRepresentation<P>,
    means: Option<&super::MeanParameters<P>>,
    declarations: &[nrz_bindings::ParameterSymbol],
) -> Result<PreparedRegions, JointError> {
    let (extents, payloads) = match covariance {
        crate::CovarianceRepresentation::Factor(factor) => (
            [
                Extent::new(factor.rows(), factor.columns())?,
                Extent::new(factor.columns(), 1)?,
            ],
            [factor.coefficients(), factor.weights()],
        ),
        crate::CovarianceRepresentation::Dense(dense) => (
            [Extent::new(dense.order(), dense.order())?; 2],
            [dense.centers(), dense.errors()],
        ),
    };
    let mut extents = extents.to_vec();
    let mut payloads = payloads.to_vec();
    let mut quantities = vec![None; 2];
    if let Some(means) = means {
        for (errors, descriptor) in means.errors.iter().zip(&means.descriptors) {
            extents.push(Extent::new(errors.len(), 1)?);
            payloads.push(errors.as_slice());
            quantities.push(Some(descriptor.clone()));
        }
    }
    let mut schemas = Vec::new();
    let mut indices = Vec::new();
    for (&extent, quantity) in extents.iter().zip(quantities) {
        indices.push(intern(
            &heap.meanings,
            &mut schemas,
            BindingSchema { extent, quantity },
            heap.limits.schemas,
        )?);
    }
    crate::binding::schema::reserve(heap, schemas.len())?;
    let plan = heap
        .descriptors
        .prepare(payloads.len(), heap.limits.descriptors)?;
    let keys = plan.keys().to_vec();
    let mut covers = Vec::new();
    if keys.len() != declarations.len() {
        return Err(JointError::LengthMismatch);
    }
    for (&key, declaration) in keys.iter().zip(declarations) {
        covers.push(heap.symbols.prepare_parameter(
            key,
            declaration.clone(),
            heap.limits.descriptors,
        )?);
    }
    let storage = heap.storage.prepare(&payloads, &[], heap.limits)?;
    let records = region_records::<P>(&indices, &extents, storage.keys(), storage.epoch())?;
    let records = plan.bind(records)?;
    Ok(PreparedRegions {
        storage,
        schemas,
        records,
        keys,
        covers,
    })
}

/// Parameter errors retain the full covariance product meaning, never a source radius.
fn evaluation_descriptors<P: Scalar>(
    model: &JointModel<P>,
) -> Result<Vec<nrz_bindings::EvaluationDescriptor>, JointError> {
    if matches!(
        model.representation(),
        crate::CovarianceRepresentation::Factor(_)
    ) {
        return Ok(Vec::new());
    }
    let mut descriptors = Vec::new();
    for column in model.sources() {
        for row in model.sources() {
            let pairing = PairingWitness::uncertainty(
                UncertaintyMode::Covariance,
                row.descriptor(),
                Some(column.descriptor()),
                None,
            )?;
            descriptors.push(nrz_bindings::EvaluationDescriptor::new(&pairing));
        }
    }
    Ok(descriptors)
}

/// Validate one declared selection, including repeated base coordinates.
fn validate_selection<P: Scalar + 'static>(
    heap: &Heap,
    model: &JointModel<P>,
    target: &TargetMap<P>,
    selection: &[usize],
) -> Result<(), JointError> {
    target.target.validate(heap)?;
    if target.layout().extent().entries() != selection.len() {
        return Err(JointError::WrongTarget);
    }
    let quantity = target.target.quantity(heap)?;
    for (position, &index) in selection.iter().enumerate() {
        for (earlier, &previous) in selection[..position].iter().enumerate() {
            if target.layout().flat_index(position)? == target.layout().flat_index(earlier)?
                && index != previous
            {
                return Err(JointError::SourceConflict);
            }
        }
        let source = model.sources().get(index).ok_or(JointError::WrongTarget)?;
        if source.descriptor() != &quantity.difference() {
            return Err(JointError::WrongTarget);
        }
    }
    Ok(())
}
/// An existing target version cannot acquire another source or mean declaration.
fn validate_existing<P: Scalar + 'static>(
    heap: &Heap,
    draft: &CompanionSetDraft<P>,
    side: usize,
) -> Result<(), JointError> {
    let target = &draft.targets[side];
    for old in &heap.joint.records {
        for (old_side, (previous, map)) in old.targets.iter().zip(&old.selections).enumerate() {
            if previous.stamp == target.stamp() {
                let model = old
                    .model
                    .downcast_ref::<JointModel<P>>()
                    .ok_or(JointError::SourceConflict)?;
                let old_means = old
                    .means
                    .as_ref()
                    .map(|value| {
                        value
                            .downcast_ref::<super::MeanParameters<P>>()
                            .ok_or(JointError::SourceConflict)
                    })
                    .transpose()?;
                if old_means.map(|means| &means.enclosures[old_side])
                    != draft.means.as_ref().map(|means| &means.enclosures[side])
                {
                    return Err(JointError::SourceConflict);
                }
                if model.id() != draft.model.id()
                    || previous.layout != target.target.layout
                    || map != &draft.selections[side]
                {
                    return Err(JointError::SourceConflict);
                }
            }
        }
    }
    Ok(())
}
