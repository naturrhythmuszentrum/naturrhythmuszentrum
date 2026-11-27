// SPDX-License-Identifier: AGPL-3.0-only
//! Store-scoped future-ray construction, replay and original witnesses.

use super::{
    AT, FutureRayProposal, FutureRayRequest, FutureRaySource, REPLAY_AT, SCHEMA, check, claim, wire,
};
use crate::budget::{Budget, BudgetKind};
use crate::enclosure::lc_affine_fibre::{self, PiAffine};
use crate::{Cas, Fault, InvalidCode, Rational};
use std::borrow::Cow;
use std::sync::Arc;

/// Portable, untrusted future-ray proposal. Original source certificates remain in the request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureRayCertificate(Vec<u8>);
impl FutureRayCertificate {
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
    /// Export charged bytes.
    /// # Errors
    /// Refuses exhausted resources.
    pub fn to_bytes(&self, b: &mut Budget) -> Result<Vec<u8>, Fault> {
        Self::from_bytes(&self.0, b).map(|copy| copy.0)
    }
}

/// Reconstructed original tuple for an interpolated projected phase.
#[derive(Clone, Debug)]
pub enum FutureRayWitness {
    /// Original independent timing observation witnesses.
    Timing(lc_affine_fibre::TimingWitness),
    /// Original common clock and phase offset witnesses.
    ClockPhase(lc_affine_fibre::ClockPhaseWitness),
    /// Original common offsets and independent timestamp errors.
    TimingClockPhase(lc_affine_fibre::TimingClockPhaseWitness),
}

#[derive(Clone, Debug)]
/// One checked original fibre retained for physical restoration.
pub(super) enum SourceReceipt {
    /// Independent timing fibre.
    Timing(lc_affine_fibre::Receipt),
    /// Common clock and phase fibre.
    ClockPhase(lc_affine_fibre::ClockPhaseReceipt),
    /// Common offsets and independent timestamp errors.
    TimingClockPhase(lc_affine_fibre::TimingClockPhaseReceipt),
}

/// Copy a bounded checked lifted topology using fallible vector reserves.
fn copy_points<'a>(
    vertices: impl Iterator<Item = &'a [PiAffine]>,
    count: usize,
    b: &mut Budget,
) -> Result<claim::SourcePoints, Fault> {
    let mut bytes = count
        .checked_mul(std::mem::size_of::<Vec<PiAffine>>())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    b.check(BudgetKind::StagedVertices, count as u64, AT)?;
    b.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    b.work(bytes, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for point in vertices {
        let structural = point
            .len()
            .checked_mul(std::mem::size_of::<PiAffine>())
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let point_bytes = point.iter().fold(structural, |sum, value| {
            sum.saturating_add(rational_storage(&value.base))
                .saturating_add(rational_storage(&value.pi))
        });
        bytes = bytes
            .checked_add(point_bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        b.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        b.work(point_bytes, AT)?;
        let mut copy = Vec::new();
        copy.try_reserve_exact(point.len())
            .map_err(|_| Fault::allocation(AT))?;
        copy.extend_from_slice(point);
        result.push(copy);
    }
    Ok(result)
}

/// Logical component bytes for both exact rational numerators and denominators.
fn rational_storage(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .max(1)
        .saturating_add(denominator.div_ceil(8).max(1))
}

/// Checked future-ray theorem and complete original source inventory.
#[derive(Clone, Debug)]
pub struct FutureRayReceipt {
    /// Complete source and reference scope.
    original: FutureRayRequest,
    /// Checked projected polygon and ray inequalities.
    proposal: FutureRayProposal,
    /// Each independently checked original receipt.
    sources: Vec<SourceReceipt>,
    /// Full original source vertices for witness interpolation.
    points: Vec<claim::SourcePoints>,
    /// Portable proposal bytes for export.
    wire: Vec<u8>,
    /// Identity of the admitting store.
    owner: Arc<()>,
}

/// Replay every complete original certificate and retain its full vertices.
pub(super) fn sources(
    cas: &mut Cas,
    request: &FutureRayRequest,
    b: &mut Budget,
) -> Result<(Vec<SourceReceipt>, Vec<claim::SourcePoints>), Fault> {
    let mut receipts = Vec::new();
    let mut points = Vec::new();
    receipts
        .try_reserve_exact(request.branches.len())
        .map_err(|_| Fault::allocation(AT))?;
    points
        .try_reserve_exact(request.branches.len())
        .map_err(|_| Fault::allocation(AT))?;
    for branch in &request.branches {
        let (receipt, topology) = match &branch.source {
            FutureRaySource::Timing(original, certificate) => {
                let receipt = cas.check_lc_affine_timing(original, certificate, b)?;
                let topology = receipt.topology(cas, b)?;
                let vertices = copy_points(
                    topology.vertices.iter().map(|v| v.point.as_slice()),
                    topology.vertices.len(),
                    b,
                )?;
                (SourceReceipt::Timing(receipt), vertices)
            }
            FutureRaySource::ClockPhase(original, certificate) => {
                let receipt = cas.check_lc_affine_clock_phase(original, certificate, b)?;
                let topology = receipt.topology(cas, b)?;
                let vertices = copy_points(
                    topology.vertices.iter().map(|v| v.point.as_slice()),
                    topology.vertices.len(),
                    b,
                )?;
                (SourceReceipt::ClockPhase(receipt), vertices)
            }
            FutureRaySource::TimingClockPhase(original, certificate) => {
                let receipt = cas.check_lc_affine_timing_clock_phase(original, certificate, b)?;
                let topology = receipt.topology(cas, b)?;
                let vertices = copy_points(
                    topology.vertices.iter().map(|v| v.point.as_slice()),
                    topology.vertices.len(),
                    b,
                )?;
                (SourceReceipt::TimingClockPhase(receipt), vertices)
            }
        };
        receipts.push(receipt);
        points.push(topology);
    }
    Ok((receipts, points))
}

/// Separately inspectable heap and owner inventory.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct StorageInventory {
    /// Receipt shell and embedded owner handles.
    pub(super) shell: u64,
    /// Canonical portable proposal, including its source scope.
    pub(super) wire: u64,
    /// Dynamic heap of the distinct cloned original request and its certificates.
    pub(super) original: u64,
    /// Epoch string retained by the cloned request.
    pub(super) epoch: u64,
    /// Checked original receipt array.
    pub(super) source_receipts: u64,
    /// Outer and inner original lifted vertex vectors.
    pub(super) point_vectors: u64,
    /// Complete original branch records.
    pub(super) branches: u64,
    /// Projected vertex records.
    pub(super) projected: u64,
    /// Four exact comparison records per projected vertex.
    pub(super) comparisons: u64,
    /// Projected convex-hull vertex identifiers.
    pub(super) hull: u64,
    /// Original lifted coordinate records.
    pub(super) coordinates: u64,
    /// Exact numerator and denominator limbs in original lifted coordinates and proposal.
    pub(super) limbs: u64,
}
impl StorageInventory {
    /// Sum all retained components without wraparound.
    pub(super) const fn total(&self) -> u64 {
        self.shell
            .saturating_add(self.wire)
            .saturating_add(self.original)
            .saturating_add(self.epoch)
            .saturating_add(self.source_receipts)
            .saturating_add(self.point_vectors)
            .saturating_add(self.branches)
            .saturating_add(self.projected)
            .saturating_add(self.comparisons)
            .saturating_add(self.hull)
            .saturating_add(self.coordinates)
            .saturating_add(self.limbs)
    }
}
/// Account for the cloned original, portable wire, checked topology, and exact number limbs.
pub(super) fn storage_inventory(
    request: &FutureRayRequest,
    proposal: &FutureRayProposal,
    receipts: &[SourceReceipt],
    points: &[claim::SourcePoints],
    wire_len: usize,
) -> StorageInventory {
    StorageInventory {
        shell: std::mem::size_of::<FutureRayReceipt>() as u64,
        wire: wire_len as u64,
        original: request
            .branches
            .iter()
            .map(|branch| match &branch.source {
                FutureRaySource::Timing(original, certificate) => {
                    lc_affine_fibre::timing_request_storage(original)
                        .saturating_add(certificate.storage_len())
                }
                FutureRaySource::ClockPhase(original, certificate) => {
                    lc_affine_fibre::clock_phase_request_storage(original)
                        .saturating_add(certificate.storage_len())
                }
                FutureRaySource::TimingClockPhase(original, certificate) => {
                    lc_affine_fibre::timing_clock_phase_request_storage(original)
                        .saturating_add(certificate.storage_len())
                }
            })
            .sum::<u64>()
            .saturating_add(rational_storage(&request.reference))
            .saturating_add(rational_storage(&request.safe_reference.base))
            .saturating_add(rational_storage(&request.safe_reference.pi)),
        epoch: request.epoch.len() as u64,
        source_receipts: std::mem::size_of_val(receipts) as u64,
        point_vectors: (std::mem::size_of_val(points)
            + (points.iter().map(Vec::len).sum::<usize>() * std::mem::size_of::<Vec<PiAffine>>()))
            as u64,
        branches: (request.branches.len() * std::mem::size_of::<super::FutureRayBranch>()) as u64,
        projected: (proposal.vertices.len() * std::mem::size_of::<super::FutureRayVertex>()) as u64,
        comparisons: (proposal.comparisons.len() * std::mem::size_of::<[PiAffine; 4]>()) as u64,
        hull: (proposal.hull.len() * std::mem::size_of::<u32>()) as u64,
        coordinates: points
            .iter()
            .flatten()
            .map(|point| point.len() * std::mem::size_of::<PiAffine>())
            .sum::<usize>() as u64,
        limbs: points
            .iter()
            .flatten()
            .flat_map(|point| point.iter())
            .fold(0u64, |sum, value| {
                sum.saturating_add(rational_storage(&value.base))
                    .saturating_add(rational_storage(&value.pi))
            })
            .saturating_add(
                proposal
                    .vertices
                    .iter()
                    .flat_map(|vertex| vertex.point.iter())
                    .fold(0u64, |sum, value| {
                        sum.saturating_add(rational_storage(&value.base))
                            .saturating_add(rational_storage(&value.pi))
                    }),
            )
            .saturating_add(
                proposal
                    .comparisons
                    .iter()
                    .flatten()
                    .fold(0u64, |sum, value| {
                        sum.saturating_add(rational_storage(&value.base))
                            .saturating_add(rational_storage(&value.pi))
                    }),
            ),
    }
}

/// Charge and retain one checked future-ray receipt.
pub(super) fn retain(
    cas: &Cas,
    request: &FutureRayRequest,
    proposal: FutureRayProposal,
    receipts: Vec<SourceReceipt>,
    points: Vec<claim::SourcePoints>,
    bytes: &[u8],
    b: &mut Budget,
) -> Result<FutureRayReceipt, Fault> {
    let storage = storage_inventory(request, &proposal, &receipts, &points, bytes.len()).total();
    b.check(BudgetKind::OutputBytes, storage, AT)?;
    b.check(BudgetKind::StagedPayloadBytes, storage, AT)?;
    b.work(storage, AT)?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    wire.extend_from_slice(bytes);
    Ok(FutureRayReceipt {
        original: request.clone(),
        proposal,
        sources: receipts,
        points,
        wire,
        owner: Arc::clone(&cas.memo_owner),
    })
}

impl Cas {
    /// Construct and independently check one whole-future-ray projection.
    /// # Errors
    /// Refuses unsupported originals, gauge unions, references and extrema.
    pub fn construct_lc_future_ray(
        &mut self,
        request: &FutureRayRequest,
        b: &mut Budget,
    ) -> Result<FutureRayReceipt, Fault> {
        let (receipts, points) = sources(self, request, b)?;
        let proposal = claim::propose(request, &points, b)?;
        check::verify(request, &points, &proposal, b)?;
        let scope = wire::scope(request, b)?;
        let bytes = wire::encode(&scope, &proposal, b)?;
        retain(self, request, proposal, receipts, points, &bytes, b)
    }
    /// Replay an untrusted future-ray proposal against complete receiver originals.
    /// # Errors
    /// Refuses changed scope or independently invalid proposal.
    pub fn check_lc_future_ray(
        &mut self,
        request: &FutureRayRequest,
        certificate: &FutureRayCertificate,
        b: &mut Budget,
    ) -> Result<FutureRayReceipt, Fault> {
        let scope = wire::scope(request, b)?;
        let proposal = wire::decode(&certificate.0, &scope, b).map_err(replay_fault)?;
        let (receipts, points) = sources(self, request, b).map_err(replay_fault)?;
        check::verify(request, &points, &proposal, b).map_err(replay_fault)?;
        retain(self, request, proposal, receipts, points, &certificate.0, b).map_err(replay_fault)
    }
}
/// Attribute exhausted receiving-store work to future-ray replay.
const fn replay_fault(mut fault: Fault) -> Fault {
    if matches!(fault.kind, crate::FaultKind::Exhausted { .. }) {
        fault.location = REPLAY_AT;
    }
    fault
}

impl FutureRayReceipt {
    /// Registered mathematical statement.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::proof::LemmaSchema {
        &SCHEMA
    }
    /// Recheck and borrow the complete original branch certificates and reference scope.
    /// # Errors
    /// Refuses foreign stores and invalidated retained evidence.
    pub fn original<'a>(
        &'a self,
        cas: &Cas,
        b: &mut Budget,
    ) -> Result<&'a FutureRayRequest, Fault> {
        self.proposal(cas, b)?;
        Ok(&self.original)
    }
    /// Recheck and access the complete projected polygon and four comparisons.
    /// # Errors
    /// Refuses foreign stores and invalidated retained evidence.
    pub fn proposal<'a>(
        &'a self,
        cas: &Cas,
        b: &mut Budget,
    ) -> Result<&'a FutureRayProposal, Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        check::verify(&self.original, &self.points, &self.proposal, b)?;
        Ok(&self.proposal)
    }
    /// Export a checked portable proposal.
    /// # Errors
    /// Refuses foreign stores or exhausted resources.
    pub fn certificate(&self, cas: &Cas, b: &mut Budget) -> Result<FutureRayCertificate, Fault> {
        self.proposal(cas, b)?;
        FutureRayCertificate::from_bytes(&self.wire, b)
    }
    /// Reuse unchanged originals or rederive changed originals and reference exactly.
    /// # Errors
    /// Refuses a failed exact current check.
    pub fn apply<'a>(
        &'a self,
        cas: &mut Cas,
        current: &FutureRayRequest,
        b: &mut Budget,
    ) -> Result<Cow<'a, Self>, Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        if current.same_charged(&self.original, b)? {
            check::verify(current, &self.points, &self.proposal, b)?;
            return Ok(Cow::Borrowed(self));
        }
        let fresh = cas.construct_lc_future_ray(current, b)?;
        Ok(Cow::Owned(fresh))
    }
    /// Interpolate full lifted extremizers and restore a physical witness at each end.
    /// `lambda=0` chooses the lower line; `lambda=1` chooses the upper line.
    /// # Errors
    /// Refuses a weight outside `[0,1]` or invalid original restoration.
    pub fn interpolate(
        &self,
        cas: &Cas,
        lambda: &Rational,
        b: &mut Budget,
    ) -> Result<FutureRayWitness, Fault> {
        self.proposal(cas, b)?;
        if lambda.is_negative()
            || lambda.compare(&Rational::one(), b)? == std::cmp::Ordering::Greater
        {
            return Err(claim::invalid());
        }
        let low = &self.proposal.vertices[self.proposal.low as usize];
        let high = &self.proposal.vertices[self.proposal.high as usize];
        let left = &self.points[0][low.source_vertex as usize];
        let right = &self.points[0][high.source_vertex as usize];
        let remaining = Rational::one().sub(lambda, b)?;
        let mut point = Vec::new();
        point
            .try_reserve_exact(left.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (x, y) in left.iter().zip(right) {
            point.push(claim::add(
                &claim::scale(x, &remaining, b)?,
                &claim::scale(y, lambda, b)?,
                b,
            )?);
        }
        match (&self.original.branches[0].source, &self.sources[0]) {
            (FutureRaySource::Timing(original, _), SourceReceipt::Timing(receipt)) => {
                Ok(FutureRayWitness::Timing(receipt.restore(
                    cas,
                    original,
                    &[point[0].clone(), point[1].clone()],
                    b,
                )?))
            }
            (FutureRaySource::ClockPhase(original, _), SourceReceipt::ClockPhase(receipt)) => {
                Ok(FutureRayWitness::ClockPhase(receipt.restore(
                    cas,
                    original,
                    &[
                        point[0].clone(),
                        point[1].clone(),
                        point[2].clone(),
                        point[3].clone(),
                    ],
                    b,
                )?))
            }
            (
                FutureRaySource::TimingClockPhase(original, _),
                SourceReceipt::TimingClockPhase(receipt),
            ) => Ok(FutureRayWitness::TimingClockPhase(
                receipt.restore(cas, original, &point, b)?,
            )),
            _ => Err(claim::invalid()),
        }
    }
}

impl FutureRayRequest {
    /// Compare complete current originals and scope under one caller ledger.
    /// # Errors
    /// Refuses exhausted comparison resources.
    pub fn same_charged(&self, other: &Self, b: &mut Budget) -> Result<bool, Fault> {
        b.work(4, AT)?;
        if self.form != other.form
            || self.reference != other.reference
            || self.safe_reference != other.safe_reference
            || self.narrowed != other.narrowed
            || self.epoch.len() != other.epoch.len()
            || self.branches.len() != other.branches.len()
        {
            return Ok(false);
        }
        b.work(self.epoch.len() as u64, AT)?;
        if self.epoch != other.epoch {
            return Ok(false);
        }
        for (left, right) in self.branches.iter().zip(&other.branches) {
            b.work(2, AT)?;
            if left.id != right.id || left.shift != right.shift {
                return Ok(false);
            }
            let equal = match (&left.source, &right.source) {
                (FutureRaySource::Timing(a, ac), FutureRaySource::Timing(z, zc)) => {
                    a.same_charged(z, b)? && ac.to_bytes(b)? == zc.to_bytes(b)?
                }
                (FutureRaySource::ClockPhase(a, ac), FutureRaySource::ClockPhase(z, zc)) => {
                    a.same_charged(z, b)? && ac.to_bytes(b)? == zc.to_bytes(b)?
                }
                (
                    FutureRaySource::TimingClockPhase(a, ac),
                    FutureRaySource::TimingClockPhase(z, zc),
                ) => a.same_charged(z, b)? && ac.to_bytes(b)? == zc.to_bytes(b)?,
                _ => false,
            };
            if !equal {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl FutureRayProposal {
    /// Copy a bounded projected polygon and every exact comparison.
    /// # Errors
    /// Refuses exhausted copy resources.
    pub fn copy_charged(&self, b: &mut Budget) -> Result<Self, Fault> {
        let bytes = wire::encode(&[], self, b)?;
        b.check(BudgetKind::OutputBytes, bytes.len() as u64, AT)?;
        b.check(
            BudgetKind::StagedPayloadBytes,
            (bytes.len() as u64).saturating_mul(2),
            AT,
        )?;
        wire::decode(&bytes, &[], b)
    }
}
