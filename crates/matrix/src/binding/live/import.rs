// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare all readers, exact closures and numerical allocations before one publication.

use super::{DependencyPolicy, LiveBindingError, ReadingImport, ReadingRegion, RegionTicket};
use crate::binding::schema::{BindingSchema, intern};
use crate::heap::{
    identity::{Key, identity},
    records::{Location, Meaning, Record},
};
use crate::storage::{Format, Region};
use crate::{Heap, HeapError, Layout, ReadingCoverage, ValueId, ValueStamp, ValueVersion};
use nrz_bindings::{Budget, readings::algebra::Cas};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Exact operation and complete flattened source closure retained before result publication.
pub(super) struct ImportProvenance {
    /// Checked actual native operation and fresh output declaration.
    pub(super) definition: super::ResultDefinition,
    /// Every original source selection, including earlier result ancestors.
    pub(super) sources: Vec<super::selected::SelectedReading>,
    /// Original deterministic assurance scopes of the two product operands.
    pub(super) coverage: [crate::Coverage; 2],
}

impl Heap {
    /// Admit all physical readings and their complete numerical entry universe together.
    ///
    /// Region numbers index `regions`; caller metadata grants no storage authority.
    /// Exact maps and representations are retained without evaluating heap floats in CAS.
    /// # Errors
    /// Refuses uncovered/foreign entries, wrong extents, nonfinite values, invalid exact
    /// evidence and resource exhaustion, publishing neither values nor readers on failure.
    pub fn import_readings<P: Scalar + 'static>(
        &mut self,
        graph: Rc<Cas>,
        coverage: ReadingCoverage<usize>,
        companions: Vec<crate::CompanionAdapter<usize>>,
        regions: &[ReadingRegion<P>],
        policy: DependencyPolicy,
        budget: &mut Budget,
    ) -> Result<ReadingImport<P>, LiveBindingError> {
        self.import_readings_with_provenance(
            graph, coverage, companions, regions, policy, None, budget,
        )
    }

    /// A checked closed producer supplies its operation and source closure before any publication.
    #[expect(
        clippy::too_many_arguments,
        reason = "same atomic import inputs plus sealed result provenance"
    )]
    pub(super) fn import_readings_with_provenance<P: Scalar + 'static>(
        &mut self,
        graph: Rc<Cas>,
        coverage: ReadingCoverage<usize>,
        companions: Vec<crate::CompanionAdapter<usize>>,
        regions: &[ReadingRegion<P>],
        policy: DependencyPolicy,
        provenance: Option<ImportProvenance>,
        budget: &mut Budget,
    ) -> Result<ReadingImport<P>, LiveBindingError> {
        super::companions::check(&coverage, &companions, regions, budget)?;
        let copies = super::admission::universe(&coverage, &companions, regions, budget)?;
        let plan = self
            .descriptors
            .prepare(regions.len(), self.limits.descriptors)?;
        let (schemas, indices) = schemas(self, regions)?;
        let tickets = tickets::<P>(self.scope, plan.keys(), regions, &indices)?;
        let retained = super::copies::bytes(copies.capacity())?;
        let mut group = budget.with_retained_payload(retained, super::admission::AT, |budget| {
            super::dependencies::prepare(
                graph,
                coverage,
                companions,
                tickets,
                policy,
                (self.scope, P::PRECISION),
                budget,
            )
        })?;
        let mutable = Rc::get_mut(&mut group).ok_or(HeapError::Capacity)?;
        mutable.copies = copies;
        if let Some(provenance) = provenance {
            mutable.antecedents = provenance.sources;
            mutable.result_definition = Some(provenance.definition);
            mutable.phasor_inputs = Some(provenance.coverage);
        }
        if regions.is_empty() {
            self.symbols.reserve_observer(self.limits.descriptors)?;
        }
        let mut covers = crate::binding::buffer(regions.len())?;
        for (ticket, region) in group.tickets.iter().zip(regions) {
            covers.push(self.symbols.prepare_readings(
                ticket.key,
                stamp(ticket),
                &group,
                region.extent.entries(),
                self.limits.descriptors,
            )?);
        }
        let payloads = regions
            .iter()
            .map(|region| region.values.as_slice())
            .collect::<Vec<_>>();
        let storage = self.storage.prepare(&payloads, &[], self.limits)?;
        let records = records::<P>(&group.tickets, &indices, storage.keys(), storage.epoch())?;
        let records = plan.bind(records)?;
        let bindings = super::bindings(&group);
        self.storage.publish(storage);
        crate::binding::schema::publish(self, schemas);
        self.descriptors.publish(records);
        self.symbols.publish(covers);
        if regions.is_empty() {
            self.symbols.publish_observer(Rc::clone(&group));
        }
        group.publish();
        Ok(bindings)
    }
}

/// Capture numerical authority before the exact immutable plan is sealed.
fn tickets<P: Scalar>(
    scope: u64,
    keys: &[Key],
    regions: &[ReadingRegion<P>],
    schemas: &[usize],
) -> Result<Vec<RegionTicket>, HeapError> {
    keys.iter()
        .zip(regions)
        .zip(schemas)
        .map(|((&key, region), &schema)| {
            Ok(RegionTicket {
                scope,
                key,
                precision: P::PRECISION,
                layout: Layout::contiguous(region.extent)?,
                identity: ValueId(identity()?),
                generation: ValueVersion(1),
                schema,
                versions: vec![1; region.extent.entries()],
            })
        })
        .collect()
}

/// General readings have no fabricated homogeneous physical descriptor.
fn schemas<P: Scalar>(
    heap: &mut Heap,
    regions: &[ReadingRegion<P>],
) -> Result<(Vec<BindingSchema>, Vec<usize>), HeapError> {
    let mut pending = Vec::new();
    let mut indices = crate::binding::buffer(regions.len())?;
    for region in regions {
        indices.push(intern(
            &heap.meanings,
            &mut pending,
            BindingSchema {
                extent: region.extent,
                quantity: None,
            },
            heap.limits.schemas,
        )?);
    }
    crate::binding::schema::reserve(heap, pending.len())?;
    Ok((pending, indices))
}

/// Numerical records contain only shape, version, category and allocation data.
fn records<P: Scalar + 'static>(
    tickets: &[RegionTicket],
    schemas: &[usize],
    arenas: &[Key],
    epoch: u64,
) -> Result<Vec<Record>, HeapError> {
    if tickets.len() != schemas.len() || tickets.len() != arenas.len() {
        return Err(HeapError::LengthMismatch);
    }
    Ok(tickets
        .iter()
        .zip(schemas)
        .zip(arenas)
        .map(|((ticket, &schema), &arena)| Record {
            schema,
            layout: ticket.layout.clone(),
            location: Location::Owned(Region {
                arena,
                epoch,
                format: Format::of::<P>(),
                offset: 0,
                length: ticket.layout.extent().entries(),
            }),
            meaning: Meaning::Value {
                stamp: stamp(ticket),
                bound: None,
                evaluation: None,
                contributions: None,
                coverage: crate::Coverage::RepresentedInputs,
            },
        })
        .collect())
}

/// Initial allocation identity; entry versions are tracked separately by coverage.
pub(super) const fn stamp(ticket: &RegionTicket) -> ValueStamp {
    ValueStamp {
        id: ticket.identity,
        version: ValueVersion(1),
    }
}
