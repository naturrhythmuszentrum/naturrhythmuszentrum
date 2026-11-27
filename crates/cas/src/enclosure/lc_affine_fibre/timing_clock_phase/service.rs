// SPDX-License-Identifier: AGPL-3.0-only
//! Store-scoped timing-clock-phase admission, replay, and live stratum checks.

use super::super::{PiAffine, QuotientTimestamp};
use super::{
    AT, PROPOSAL_AT, REPLAY_AT, SIGN_AT, Signs, TOPOLOGY_AT, TimingClockPhaseRequest,
    TimingClockPhaseTopology, TimingClockPhaseWitness, check, claim, geometry, wire,
};
use crate::budget::{Budget, BudgetKind};
use crate::{Cas, Fault, FaultKind, InvalidCode};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::Arc;

/// Portable untrusted proposal bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingClockPhaseCertificate(Vec<u8>);

impl TimingClockPhaseCertificate {
    /// Owned portable certificate bytes in the cloned original.
    pub(crate) const fn storage_len(&self) -> u64 {
        self.0.len() as u64
    }
    /// Copy bounded untrusted bytes.
    /// # Errors
    /// Refuses exhausted resources.
    pub fn from_bytes(bytes: &[u8], b: &mut Budget) -> Result<Self, Fault> {
        b.check(BudgetKind::WitnessBytes, bytes.len() as u64, AT)?;
        b.check(BudgetKind::StagedPayloadBytes, bytes.len() as u64, AT)?;
        b.work(bytes.len() as u64, AT)?;
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len())
            .map_err(|_| Fault::allocation(AT))?;
        copy.extend_from_slice(bytes);
        Ok(Self(copy))
    }

    /// Export a charged copy.
    /// # Errors
    /// Refuses exhausted resources.
    pub fn to_bytes(&self, b: &mut Budget) -> Result<Vec<u8>, Fault> {
        Self::from_bytes(&self.0, b).map(|value| value.0)
    }
}

/// Checked common-clock and common-phase branch owned by one receiving store.
#[derive(Clone, Debug)]
pub struct TimingClockPhaseReceipt {
    /// Original checked inventory retained for replay.
    original: TimingClockPhaseRequest,
    /// Complete original lifted topology.
    topology: TimingClockPhaseTopology,
    /// Required original signs of all branch predicates.
    original_signs: Signs,
    /// Portable proposal bytes retained for export.
    wire: Vec<u8>,
    /// Identity of the admitting store.
    owner: Arc<()>,
}

impl TimingClockPhaseRequest {
    /// Compare complete canonical originals with charged traversal and byte comparison.
    /// # Errors
    /// Refuses exhausted resources.
    pub fn same_charged(&self, other: &Self, b: &mut Budget) -> Result<bool, Fault> {
        b.work(4, AT)?;
        if self.source_order.len() != self.source_versions.len()
            || other.source_order.len() != other.source_versions.len()
            || self.source_order.len() != other.source_order.len()
            || self.observations.len() != other.observations.len()
            || self.rate_declarations.len() != other.rate_declarations.len()
            || self.initial_declarations.len() != other.initial_declarations.len()
            || self.clock_declarations.len() != other.clock_declarations.len()
            || self.offset_declarations.len() != other.offset_declarations.len()
            || self
                .observations
                .iter()
                .zip(&other.observations)
                .any(|(a, z)| {
                    a.error_declarations.len() != z.error_declarations.len()
                        || a.time_declarations.len() != z.time_declarations.len()
                        || a.measured_declarations.len() != z.measured_declarations.len()
                        || a.physical_declarations.len() != z.physical_declarations.len()
                })
        {
            return Ok(false);
        }
        let left = wire::scope(self, b)?;
        let right = wire::scope(other, b)?;
        b.work(left.len().max(right.len()) as u64, AT)?;
        Ok(left == right)
    }
}

impl TimingClockPhaseTopology {
    /// Copy a bounded topology into caller-owned storage.
    /// # Errors
    /// Refuses exhausted output, staging, Work or allocation limits.
    pub fn copy_charged(&self, b: &mut Budget) -> Result<Self, Fault> {
        let mut bytes = size_of::<Self>() as u64;
        for vertex in &self.vertices {
            bytes = bytes
                .saturating_add(size_of_val(vertex) as u64)
                .saturating_add((vertex.point.len() * size_of::<PiAffine>()) as u64)
                .saturating_add((vertex.active.len() * size_of::<u32>()) as u64);
            for value in &vertex.point {
                for rational in [&value.base, &value.pi] {
                    let (n, d) = rational.component_bits();
                    bytes = bytes
                        .saturating_add((n.saturating_add(7) / 8).max(1))
                        .saturating_add((d.saturating_add(7) / 8).max(1));
                }
            }
        }
        for edge in &self.edges {
            bytes = bytes
                .saturating_add(size_of_val(edge) as u64)
                .saturating_add((edge.supports.len() * size_of::<u32>()) as u64);
        }
        b.check(BudgetKind::OutputBytes, bytes, AT)?;
        b.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
        b.work(bytes, AT)?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(self.vertices.len())
            .map_err(|_| Fault::allocation(AT))?;
        for vertex in &self.vertices {
            let mut active = Vec::new();
            active
                .try_reserve_exact(vertex.active.len())
                .map_err(|_| Fault::allocation(AT))?;
            active.extend_from_slice(&vertex.active);
            let mut point = Vec::new();
            point
                .try_reserve_exact(vertex.point.len())
                .map_err(|_| Fault::allocation(AT))?;
            point.extend_from_slice(&vertex.point);
            vertices.push(super::TimingClockPhaseVertex { point, active });
        }
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(self.edges.len())
            .map_err(|_| Fault::allocation(AT))?;
        for edge in &self.edges {
            let mut supports = Vec::new();
            supports
                .try_reserve_exact(edge.supports.len())
                .map_err(|_| Fault::allocation(AT))?;
            supports.extend_from_slice(&edge.supports);
            edges.push(super::TimingClockPhaseEdge {
                ends: edge.ends,
                supports,
            });
        }
        Ok(Self {
            dimension: self.dimension,
            vertices,
            edges,
        })
    }
}

impl TimingClockPhaseReceipt {
    /// Registered mathematical statement for this exact lifted branch.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::proof::LemmaSchema {
        &super::super::TIMING_CLOCK_PHASE_SCHEMA
    }

    /// Recheck the stored theorem against its original inventory.
    /// # Errors
    /// Refuses foreign stores, invalid topology or exhausted resources.
    pub fn topology<'a>(
        &'a self,
        cas: &Cas,
        b: &mut Budget,
    ) -> Result<&'a TimingClockPhaseTopology, Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        check::verify(&self.original, &self.topology, b)?;
        Ok(&self.topology)
    }

    /// Export a portable untrusted certificate after rechecking.
    /// # Errors
    /// Refuses foreign stores, invalid topology or exhausted resources.
    pub fn certificate(
        &self,
        cas: &Cas,
        b: &mut Budget,
    ) -> Result<TimingClockPhaseCertificate, Fault> {
        self.topology(cas, b)?;
        TimingClockPhaseCertificate::from_bytes(&self.wire, b)
    }

    /// Recompute changed originals and require the same topology and ordered sign stratum.
    /// # Errors
    /// Refuses stale versions, changed incidence/signs and exhausted resources.
    pub fn apply<'a>(
        &'a self,
        cas: &Cas,
        current: &TimingClockPhaseRequest,
        b: &mut Budget,
    ) -> Result<
        (
            Cow<'a, TimingClockPhaseTopology>,
            Cow<'a, [super::super::SignPredicate]>,
        ),
        Fault,
    > {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        if current.same_charged(&self.original, b)? {
            return Ok((
                Cow::Borrowed(&self.topology),
                Cow::Borrowed(&self.original_signs),
            ));
        }
        if current.source_order.len() != self.original.source_order.len()
            || current.source_versions.len() != self.original.source_versions.len()
            || current.revision != self.original.revision
            || current.pi != self.original.pi
            || current.epoch != self.original.epoch
            || current.source_order != self.original.source_order
            || current.observations.len() != self.original.observations.len()
            || current.source_versions == self.original.source_versions
            || current.rate_declarations.len() != self.original.rate_declarations.len()
            || current.initial_declarations.len() != self.original.initial_declarations.len()
            || current.clock_declarations.len() != self.original.clock_declarations.len()
            || current.offset_declarations.len() != self.original.offset_declarations.len()
        {
            return Err(claim::invalid());
        }
        for (new, old) in current.observations.iter().zip(&self.original.observations) {
            if new.reported != old.reported
                || new.lift != old.lift
                || new.lift_domain != old.lift_domain
                || new.error_declarations.len() != old.error_declarations.len()
                || new.time_declarations.len() != old.time_declarations.len()
                || new.measured_declarations.len() != old.measured_declarations.len()
                || new.physical_declarations.len() != old.physical_declarations.len()
            {
                return Err(claim::invalid());
            }
        }
        let live = geometry::propose(current, b).map_err(proposal_fault)?;
        check::verify(current, &live, b)?;
        if live.dimension != self.topology.dimension
            || live.edges != self.topology.edges
            || live.vertices.len() != self.topology.vertices.len()
            || live
                .vertices
                .iter()
                .zip(&self.topology.vertices)
                .any(|(a, z)| a.active != z.active)
        {
            return Err(Fault::invalid(InvalidCode::PendingPremise, TOPOLOGY_AT));
        }
        let signs = geometry::predicates(current, b)?;
        if signs.len() != self.original_signs.len()
            || signs
                .iter()
                .zip(&self.original_signs)
                .any(|(a, z)| a.required != z.required)
        {
            return Err(Fault::invalid(InvalidCode::PendingPremise, SIGN_AT));
        }
        Ok((Cow::Owned(live), Cow::Owned(signs)))
    }

    /// Return current exact topology and ordered sign images.
    /// # Errors
    /// Refuses stale authority, changed stratum or exhausted resources.
    pub fn live_sign_predicates<'a>(
        &'a self,
        cas: &Cas,
        current: &TimingClockPhaseRequest,
        b: &mut Budget,
    ) -> Result<
        (
            Cow<'a, TimingClockPhaseTopology>,
            Cow<'a, [super::super::SignPredicate]>,
        ),
        Fault,
    > {
        self.apply(cas, current, b)
    }

    /// Restore every physical timestamp, independent error and both phase layers.
    /// # Errors
    /// Refuses points outside the current complete fibre or exhausted resources.
    pub fn restore(
        &self,
        cas: &Cas,
        current: &TimingClockPhaseRequest,
        point: &[PiAffine],
        b: &mut Budget,
    ) -> Result<TimingClockPhaseWitness, Fault> {
        self.apply(cas, current, b)?;
        if point.len() != current.observations.len() + 4 {
            return Err(claim::invalid());
        }
        let (rows, pi) = claim::rows(current, b)?;
        for row in &rows {
            if geometry::residual(row, point, b)?.sign(&pi, b)? == Ordering::Greater {
                return Err(claim::invalid());
            }
        }
        if point[0].sign(&pi, b)? != Ordering::Greater {
            return Err(claim::invalid());
        }
        let initial = point[1].add(&point[2], b)?.sub(&point[3], b)?;
        let mut errors = Vec::new();
        let mut timestamps = Vec::new();
        let mut measured = Vec::new();
        let mut physical = Vec::new();
        for values in [&mut errors, &mut timestamps] {
            values
                .try_reserve_exact(current.observations.len())
                .map_err(|_| Fault::allocation(AT))?;
        }
        for values in [&mut measured, &mut physical] {
            values
                .try_reserve_exact(current.observations.len())
                .map_err(|_| Fault::allocation(AT))?;
        }
        for (index, item) in current.observations.iter().enumerate() {
            let w = &point[4 + index];
            let value = point[1]
                .add(&point[0].mul(&item.reported, b)?, b)?
                .add(w, b)?;
            physical.push(value.add(&point[2], b)?);
            measured.push(value);
            errors.push(QuotientTimestamp {
                numerator: w.clone(),
                denominator: point[0].clone(),
            });
            timestamps.push(QuotientTimestamp {
                numerator: point[0]
                    .mul(&item.reported, b)?
                    .add(&point[3], b)?
                    .add(w, b)?,
                denominator: point[0].clone(),
            });
        }
        Ok(TimingClockPhaseWitness {
            rate: point[0].clone(),
            measured_intercept: point[1].clone(),
            phase_offset: point[2].clone(),
            clock_offset: QuotientTimestamp {
                numerator: point[3].clone(),
                denominator: point[0].clone(),
            },
            initial,
            errors,
            timestamps,
            measured,
            physical,
        })
    }
}

impl Cas {
    /// Construct and independently check a complete timing-clock-phase lifted fibre.
    /// # Errors
    /// Refuses unsupported originals, uncertain signs or exhausted resources.
    pub fn construct_lc_affine_timing_clock_phase(
        &self,
        request: &TimingClockPhaseRequest,
        b: &mut Budget,
    ) -> Result<TimingClockPhaseReceipt, Fault> {
        let topology = geometry::propose(request, b).map_err(proposal_fault)?;
        check::verify(request, &topology, b)?;
        let scope = wire::scope(request, b)?;
        let bytes = wire::encode(&scope, &topology, b)?;
        retain(self, request, topology, &bytes, b)
    }

    /// Replay untrusted proposal bytes against receiver supplied exact originals.
    /// # Errors
    /// Refuses changed scope, malformed incidence or exhausted resources atomically.
    pub fn check_lc_affine_timing_clock_phase(
        &mut self,
        expected: &TimingClockPhaseRequest,
        certificate: &TimingClockPhaseCertificate,
        b: &mut Budget,
    ) -> Result<TimingClockPhaseReceipt, Fault> {
        let scope = wire::scope(expected, b)?;
        let topology = wire::decode(&certificate.0, &scope, expected.observations.len() + 4, b)?;
        self.transaction(b, |candidate, b| {
            check::verify(expected, &topology, b)?;
            retain(candidate, expected, topology, &certificate.0, b)
        })
        .map_err(replay_fault)
    }
}

/// Attribute exhausted proposal arithmetic to this tag's proposal stage.
const fn proposal_fault(mut fault: Fault) -> Fault {
    if matches!(fault.kind, FaultKind::Exhausted { .. }) {
        fault.location = PROPOSAL_AT;
    }
    fault
}

/// Attribute exhausted receiving-store replay to this tag's replay stage.
const fn replay_fault(mut fault: Fault) -> Fault {
    if matches!(fault.kind, FaultKind::Exhausted { .. }) {
        fault.location = REPLAY_AT;
    }
    fault
}

/// Charge and retain one checked store-scoped receipt.
fn retain(
    cas: &Cas,
    request: &TimingClockPhaseRequest,
    topology: TimingClockPhaseTopology,
    bytes: &[u8],
    b: &mut Budget,
) -> Result<TimingClockPhaseReceipt, Fault> {
    let original_signs = geometry::predicates(request, b)?;
    let storage = storage_bytes(request, &topology, &original_signs, bytes.len());
    b.check(BudgetKind::OutputBytes, storage, AT)?;
    b.check(BudgetKind::StagedPayloadBytes, storage, AT)?;
    b.work(storage, AT)?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    wire.extend_from_slice(bytes);
    Ok(TimingClockPhaseReceipt {
        original: request.clone(),
        topology,
        original_signs,
        wire,
        owner: Arc::clone(&cas.memo_owner),
    })
}

/// Dynamic heap inventory of the cloned original request, without its embedded shell.
pub(super) fn request_storage_bytes(request: &TimingClockPhaseRequest) -> u64 {
    fn rational(value: &crate::Rational) -> u64 {
        let (n, d) = value.component_bits();
        (n.saturating_add(7) / 8)
            .max(1)
            .saturating_add((d.saturating_add(7) / 8).max(1))
    }
    let mut bytes = 0u64;
    bytes = bytes.saturating_add(
        (request.observations.len() * size_of::<super::TimingClockPhaseObservation>()) as u64,
    );
    bytes = bytes.saturating_add(
        (request.source_order.len() * size_of::<super::TimingClockPhaseSourceSlot>()) as u64,
    );
    bytes = bytes.saturating_add((request.source_versions.len() * size_of::<u64>()) as u64);
    bytes = bytes.saturating_add(request.epoch.len() as u64);
    for ranges in [
        &request.rate_declarations,
        &request.initial_declarations,
        &request.clock_declarations,
        &request.offset_declarations,
    ] {
        bytes = bytes.saturating_add((ranges.len() * size_of::<[crate::Rational; 2]>()) as u64);
        for range in ranges {
            bytes = bytes
                .saturating_add(rational(&range[0]))
                .saturating_add(rational(&range[1]));
        }
    }
    for range in [
        &request.rate,
        &request.initial,
        &request.clock,
        &request.offset,
    ] {
        bytes = bytes
            .saturating_add(rational(&range[0]))
            .saturating_add(rational(&range[1]));
    }
    for observation in &request.observations {
        bytes = bytes
            .saturating_add(rational(&observation.reported))
            .saturating_add(rational(&observation.error[0]))
            .saturating_add(rational(&observation.error[1]));
        for ranges in [
            &observation.error_declarations,
            &observation.time_declarations,
            &observation.measured_declarations,
            &observation.physical_declarations,
        ] {
            bytes = bytes.saturating_add((ranges.len() * size_of::<[crate::Rational; 2]>()) as u64);
            for range in ranges {
                bytes = bytes
                    .saturating_add(rational(&range[0]))
                    .saturating_add(rational(&range[1]));
            }
        }
    }
    for value in request
        .pi
        .sums
        .iter()
        .chain([&request.pi.lower, &request.pi.upper])
    {
        bytes = bytes.saturating_add(rational(value));
    }
    bytes
}
/// Logical byte inventory retained by one receipt, including vector bodies and exact components.
pub(super) fn storage_bytes(
    request: &TimingClockPhaseRequest,
    topology: &TimingClockPhaseTopology,
    signs: &Signs,
    wire_len: usize,
) -> u64 {
    fn rational(value: &crate::Rational) -> u64 {
        let (n, d) = value.component_bits();
        (n.saturating_add(7) / 8)
            .max(1)
            .saturating_add((d.saturating_add(7) / 8).max(1))
    }
    let mut bytes = (size_of::<TimingClockPhaseReceipt>() as u64)
        .saturating_add(request_storage_bytes(request));
    bytes = bytes.saturating_add(
        (topology.vertices.len() * size_of::<super::TimingClockPhaseVertex>()) as u64,
    );
    bytes = bytes
        .saturating_add((topology.edges.len() * size_of::<super::TimingClockPhaseEdge>()) as u64);
    for vertex in &topology.vertices {
        bytes = bytes
            .saturating_add((vertex.active.len() * size_of::<u32>()) as u64)
            .saturating_add((vertex.point.len() * size_of::<PiAffine>()) as u64);
        for value in &vertex.point {
            bytes = bytes
                .saturating_add(rational(&value.base))
                .saturating_add(rational(&value.pi));
        }
    }
    for edge in &topology.edges {
        bytes = bytes.saturating_add((edge.supports.len() * size_of::<u32>()) as u64);
    }
    bytes = bytes.saturating_add((signs.len() * size_of::<super::super::SignPredicate>()) as u64);
    for sign in signs {
        for value in &sign.image {
            bytes = bytes.saturating_add(rational(value));
        }
    }
    bytes.saturating_add(wire_len as u64)
}
