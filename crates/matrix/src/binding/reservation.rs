// SPDX-License-Identifier: AGPL-3.0-only
//! Reserve real backend slabs and descriptor occupations without exposing a handle.

use nrz_scalar::Scalar;

use super::{InputSnapshot, OperationError, buffer};
use crate::heap::identity::identity;
use crate::heap::publication::{SchemaPlan, prepare_records};
use crate::heap::records::{Location, Record};
use crate::heap::schema::{Schema, intern};
use crate::heap::slots::PreparedSlots;
use crate::storage::PreparedStorage;
use crate::{
    Coverage, Extent, Heap, HeapError, PublishedBundle, SchemaId, ValueId, ValueStamp, ValueVersion,
};

/// Reserved future identity and schema, without permission to resolve an output.
#[derive(Clone, Copy, Debug)]
pub struct OutputReservation {
    /// New value identity and initial version.
    stamp: ValueStamp,
    /// Exact selected extent of the future contiguous region.
    extent: Extent,
    /// Immutable coherent value schema, reserved but possibly not yet interned.
    schema: SchemaId,
    /// Complete propagated coverage for this output.
    coverage: Coverage,
    /// Reserved total and evaluation regions are independent of physical coverage.
    pub(crate) bounded: bool,
    /// Distinct evaluation companion exists for this output.
    pub(crate) evaluation: bool,
}

impl OutputReservation {
    /// Future value identity and version; this is not a published handle.
    #[must_use]
    pub const fn stamp(self) -> ValueStamp {
        self.stamp
    }
    /// Future logical rectangle.
    #[must_use]
    pub const fn extent(self) -> Extent {
        self.extent
    }
    /// Reserved exact schema identity.
    #[must_use]
    pub const fn schema(self) -> SchemaId {
        self.schema
    }
    /// Supported companion coverage, preserved by exact copy.
    #[must_use]
    pub const fn coverage(self) -> Coverage {
        self.coverage
    }
}

/// Complete invisible bundle; its prepared slots use the heap's existing commit primitive.
#[derive(Debug)]
pub(crate) struct Reservations<P: Scalar> {
    /// Real allocated zero-initialized destination slabs, unavailable to views.
    pub(crate) storage: PreparedStorage,
    /// New immutable schemas with pre-reserved heap capacity.
    pub(crate) schemas: Vec<Schema>,
    /// Complete value/companion descriptors with reserved future target versions.
    pub(crate) records: PreparedSlots<Record>,
    /// Handles retained privately until the publication commit.
    pub(crate) bundles: Vec<PublishedBundle<P>>,
    /// Publicly inspectable reservation facts, without handles.
    pub(crate) outputs: Vec<OutputReservation>,
}

impl<Q: Scalar + 'static> Reservations<Q> {
    /// Allocate all destinations as one reservation using existing storage and slot plans.
    pub(crate) fn prepare<P: Scalar + 'static>(
        heap: &mut Heap,
        inputs: &[InputSnapshot<P>],
        operation: &crate::CheckedOperation,
    ) -> Result<Self, OperationError> {
        let (plans, schemas) = schema_plans(heap, inputs, operation)?;
        let payloads = zeros::<P, Q>(inputs, &plans)?;
        let slices = payloads.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let slots = heap
            .descriptors
            .prepare(slices.len(), heap.limits.descriptors)?;
        let storage = heap.storage.prepare(&slices, &[], heap.limits)?;
        let mut records = buffer(slices.len())?;
        let mut bundles = buffer(inputs.len())?;
        let mut outputs = buffer(inputs.len())?;
        for (input, plan) in inputs.iter().zip(plans) {
            let start = records.len();
            let count =
                1 + usize::from(plan.bound.is_some()) + usize::from(plan.evaluation.is_some());
            let end = start.checked_add(count).ok_or(HeapError::Capacity)?;
            let stamp = ValueStamp {
                id: ValueId(identity()?),
                version: ValueVersion(1),
            };
            let extent = input.layout().extent();
            let (mut prepared, bundle) = prepare_records::<Q>(
                heap.scope,
                extent,
                slots.keys().get(start..end).ok_or(HeapError::Capacity)?,
                storage.keys().get(start..end).ok_or(HeapError::Capacity)?,
                &plan,
                stamp,
                storage.epoch(),
            )?;
            if let Some(record) = prepared.first_mut()
                && let crate::heap::records::Meaning::Value { coverage, .. } = &mut record.meaning
            {
                *coverage = input.coverage;
            }
            outputs.push(OutputReservation {
                stamp,
                extent,
                schema: SchemaId {
                    scope: heap.scope,
                    index: plan.value,
                },
                coverage: input.coverage,
                bounded: plan.bound.is_some(),
                evaluation: plan.evaluation.is_some(),
            });
            records.extend(prepared);
            bundles.push(bundle);
        }
        Ok(Self {
            storage,
            schemas,
            records: slots.bind(records)?,
            bundles,
            outputs,
        })
    }

    /// Reuse the layout proof on actual reserved regions, including every companion.
    pub(crate) fn validate(&self, heap: &Heap) -> Result<(), OperationError> {
        if self.bundles.len() != self.outputs.len()
            || self.storage.keys().len() != self.records.keys().len()
        {
            return Err(HeapError::LengthMismatch.into());
        }
        let schema_count = heap
            .schemas
            .len()
            .checked_add(self.schemas.len())
            .ok_or(HeapError::Capacity)?;
        let mut destinations = buffer(self.records.values.len())?;
        for (record, arena) in self.records.values.iter().zip(self.storage.keys()) {
            let Location::Owned(region) = record.location else {
                return Err(HeapError::InvalidLayout.into());
            };
            if region.arena != *arena || record.schema >= schema_count {
                return Err(HeapError::InvalidLayout.into());
            }
            record
                .layout
                .validate(region.length, heap.limits.layout_work)?;
            destinations.push((region, &record.layout));
        }
        crate::layout::validate_outputs(&destinations, heap.limits.layout_work)?;
        Ok(())
    }
}

/// Intern selected value and companion schemas together, reusing exact pool equality.
fn schema_plans<P: Scalar + 'static>(
    heap: &mut Heap,
    inputs: &[InputSnapshot<P>],
    operation: &crate::CheckedOperation,
) -> Result<(Vec<SchemaPlan>, Vec<Schema>), OperationError> {
    let mut plans = buffer(inputs.len())?;
    let mut pending = Vec::new();
    let numerical = matches!(operation.recipe, super::NativeRecipe::Affine(_));
    for (input, output) in inputs.iter().zip(operation.outputs.iter()) {
        let extent = input.layout().extent();
        let quantity = output.descriptor().clone();
        let value = intern(
            &heap.schemas,
            &mut pending,
            Schema {
                extent,
                quantity: quantity.clone(),
            },
            heap.limits.schemas,
        )?;
        let bounded = numerical || input.companion().is_some();
        let bound = if bounded {
            let radius =
                nrz_units::uncertainty(nrz_units::UncertaintyMode::Radius, &quantity, None, None)
                    .map_err(HeapError::from)?;
            Some(intern(
                &heap.schemas,
                &mut pending,
                Schema {
                    extent,
                    quantity: radius,
                },
                heap.limits.schemas,
            )?)
        } else {
            None
        };
        plans.push(SchemaPlan {
            pending: Vec::new(),
            value,
            bound,
            evaluation: if numerical || input.evaluation.is_some() {
                bound
            } else {
                None
            },
        });
    }
    heap.schemas
        .try_reserve(pending.len())
        .map_err(|_| HeapError::Capacity)?;
    Ok((plans, pending))
}

/// Allocate every private value, total-bound and evaluation region before evaluation.
fn zeros<P: Scalar, Q: Scalar>(
    inputs: &[InputSnapshot<P>],
    plans: &[SchemaPlan],
) -> Result<Vec<Vec<Q>>, HeapError> {
    let count = plans.iter().try_fold(0_usize, |total, plan| {
        total
            .checked_add(
                1 + usize::from(plan.bound.is_some()) + usize::from(plan.evaluation.is_some()),
            )
            .ok_or(HeapError::Capacity)
    })?;
    let mut payloads = buffer(count)?;
    for (input, plan) in inputs.iter().zip(plans) {
        let copies = 1 + usize::from(plan.bound.is_some()) + usize::from(plan.evaluation.is_some());
        for _ in 0..copies {
            let length = input.layout().extent().entries();
            let mut values = buffer(length)?;
            values.resize(length, Q::ZERO);
            payloads.push(values);
        }
    }
    Ok(payloads)
}

/// Only private corruption can violate an already checked destination reservation.
#[cfg(test)]
mod tests {
    use crate::binding_testing::{TestResult, operation};
    use crate::{
        Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, Layout, OperationError,
    };
    use nrz_units::Descriptor;

    /// The existing injectivity proof rechecks every private destination map.
    /// ´test:unit:reserved-output-map-cannot-repeat-an-address´
    #[test]
    fn reserved_output_map_cannot_repeat_an_address() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[1.0_f64, 2.0])?;
        let mut bound = Binding::new(
            operation(2, 1)?,
            vec![source.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        bound.reservations.validate(bound.heap)?;
        let record = bound
            .reservations
            .records
            .values
            .first_mut()
            .ok_or("missing reservation")?;
        record.layout = Layout::gather(Extent::new(2, 1)?, 2, &[0, 0], 2)?;
        assert!(matches!(
            bound.reservations.validate(bound.heap),
            Err(OperationError::Heap(HeapError::OutputOverlap))
        ));
        assert_eq!(bound.heap.descriptors.iter().count(), 1);
        Ok(())
    }
}
