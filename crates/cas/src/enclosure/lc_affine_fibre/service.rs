// SPDX-License-Identifier: AGPL-3.0-only
//! Store-scoped timing theorem and atomic receiving-store replay.

use super::{
    AT, Edge, SCHEMA, SIGN_AT, SignPredicate, TOPOLOGY_AT, TimingRequest, Topology, Vertex, check,
    claim, wire,
};
use crate::budget::{Budget, BudgetKind};
use crate::{Cas, Fault, InvalidCode};
use std::borrow::Cow;
use std::sync::Arc;

/// Untrusted portable proposal; possession grants no theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate(Vec<u8>);

impl Certificate {
    /// Copy bounded hostile bytes without trusting their mathematical fields.
    /// # Errors
    /// Refuses exhausted caller witness and staging limits.
    pub fn from_bytes(bytes: &[u8], budget: &mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::WitnessBytes, bytes.len() as u64, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.len() as u64, AT)?;
        budget.work(bytes.len() as u64, AT)?;
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len())
            .map_err(|_| Fault::allocation(AT))?;
        copy.extend_from_slice(bytes);
        Ok(Self(copy))
    }

    /// Export a charged copy of the untrusted proposal.
    /// # Errors
    /// Refuses exhausted caller resources.
    pub fn to_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        Self::from_bytes(&self.0, budget).map(|value| value.0)
    }
}

/// One checked branch topology bound to the receiving store and original inventory.
#[derive(Clone, Debug)]
pub struct Receipt {
    /// Original checked inventory retained for replay.
    original: TimingRequest,
    /// Checked original topology.
    topology: Topology,
    /// Ordered exact original predicate images and required signs.
    original_signs: Vec<SignPredicate>,
    /// Portable proposal bytes retained for export.
    wire: Vec<u8>,
    /// Identity of the store that admitted this receipt.
    owner: Arc<()>,
}

impl Topology {
    /// Make an owned topology for a caller that cannot keep the receipt borrow.
    /// # Errors
    /// Refuses exhausted Work, output/staging bytes or allocation capacity.
    pub fn copy_charged(&self, b: &mut Budget) -> Result<Self, Fault> {
        let mut work = size_of::<Self>() as u64;
        work = work.saturating_add((self.vertices.len() * size_of::<Vertex>()) as u64);
        work = work.saturating_add((self.edges.len() * size_of::<Edge>()) as u64);
        for vertex in &self.vertices {
            work = work.saturating_add((vertex.active.len() * size_of::<u32>()) as u64);
            for point in &vertex.point {
                for rational in [&point.base, &point.pi] {
                    let (n, d) = rational.component_bits();
                    work = work.saturating_add((n.saturating_add(7) / 8).max(1));
                    work = work.saturating_add((d.saturating_add(7) / 8).max(1));
                }
            }
        }
        for edge in &self.edges {
            work = work.saturating_add((edge.supports.len() * size_of::<u32>()) as u64);
        }
        b.check(BudgetKind::OutputBytes, work, AT)?;
        b.check(BudgetKind::StagedPayloadBytes, work.saturating_mul(2), AT)?;
        b.work(work, AT)?;
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
            vertices.push(Vertex {
                point: vertex.point.clone(),
                active,
            });
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
            edges.push(Edge {
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

impl TimingRequest {
    /// Compare complete canonical originals with traversal and byte comparison charged.
    /// # Errors
    /// Refuses exhausted caller resources.
    pub fn same_charged(&self, other: &Self, b: &mut Budget) -> Result<bool, Fault> {
        // Cover the two length checks here and their repeat in Receipt::apply.
        b.work(4, AT)?;
        if self.source_order.len() != other.source_order.len()
            || self.source_versions.len() != other.source_versions.len()
        {
            return Ok(false);
        }
        let left = wire::scope(self, b)?;
        let right = wire::scope(other, b)?;
        b.work(left.len().max(right.len()) as u64, AT)?;
        Ok(left == right)
    }
}

impl Receipt {
    /// Registered mathematical statement, scoped to one original lift branch.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::proof::LemmaSchema {
        &SCHEMA
    }

    /// Checked topology at construction time, with a fresh original recheck.
    /// # Errors
    /// Refuses foreign stores, stale originals or exhausted caller resources.
    pub fn topology<'a>(&'a self, cas: &Cas, b: &mut Budget) -> Result<&'a Topology, Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        check::verify(&self.original, &self.topology, b)?;
        Ok(&self.topology)
    }

    /// Export portable untrusted bytes after checking the retained original claim.
    /// # Errors
    /// Refuses foreign/stale authority or exhausted caller resources.
    pub fn certificate(&self, cas: &Cas, b: &mut Budget) -> Result<Certificate, Fault> {
        self.topology(cas, b)?;
        Certificate::from_bytes(&self.wire, b)
    }

    /// Recompute changed symbolic incidence and return its ordered sign inventory.
    /// Unchanged originals reuse the admitted topology and sign inventory.
    /// Arrival values must advance at least one source version when they change.
    /// # Errors
    /// Refuses changed inventory, stale version, changed sign stratum or exact uncertainty.
    pub fn apply<'a>(
        &'a self,
        cas: &Cas,
        current: &TimingRequest,
        b: &mut Budget,
    ) -> Result<(Cow<'a, Topology>, Cow<'a, [SignPredicate]>), Fault> {
        if !Arc::ptr_eq(&self.owner, &cas.memo_owner) {
            return Err(Fault::invalid(InvalidCode::ForeignHandle, AT));
        }
        // The charged canonical traversal covers every complete field comparison
        // below when source-vector lengths match. A length mismatch refuses here,
        // before those comparisons; same_charged prepays both length checks.
        if current.same_charged(&self.original, b)? {
            return Ok((
                Cow::Borrowed(&self.topology),
                Cow::Borrowed(&self.original_signs),
            ));
        }
        if current.source_order.len() != self.original.source_order.len()
            || current.source_versions.len() != self.original.source_versions.len()
        {
            return Err(claim::invalid());
        }
        if current.revision != self.original.revision
            || current.pi != self.original.pi
            || current.epoch != self.original.epoch
            || current.nominal != self.original.nominal
            || current.source_order != self.original.source_order
            || current.rate_declarations.len() != self.original.rate_declarations.len()
            || current.initial_declarations.len() != self.original.initial_declarations.len()
            || current.observations.len() != self.original.observations.len()
            || current.source_versions.len() != self.original.source_versions.len()
        {
            return Err(claim::invalid());
        }
        for (new, old) in current.observations.iter().zip(&self.original.observations) {
            if new.lift != old.lift
                || new.lift_domain != old.lift_domain
                || new.time_declarations.len() != old.time_declarations.len()
                || new.phase_declarations.len() != old.phase_declarations.len()
            {
                return Err(claim::invalid());
            }
        }
        if current.source_versions == self.original.source_versions {
            return Err(claim::invalid());
        }
        let live = claim::propose(current, b)?;
        check::verify(current, &live, b)?;
        if live.dimension != self.topology.dimension
            || live.vertices.len() != self.topology.vertices.len()
            || live.edges != self.topology.edges
        {
            return Err(Fault::invalid(InvalidCode::PendingPremise, TOPOLOGY_AT));
        }
        for (left, right) in live.vertices.iter().zip(&self.topology.vertices) {
            if left.active != right.active {
                return Err(Fault::invalid(InvalidCode::PendingPremise, TOPOLOGY_AT));
            }
        }
        let current_signs = claim::predicates(current, b)?;
        if self.original_signs.len() != current_signs.len()
            || self
                .original_signs
                .iter()
                .zip(&current_signs)
                .any(|(old, new)| old.required != new.required)
        {
            return Err(Fault::invalid(InvalidCode::PendingPremise, SIGN_AT));
        }
        Ok((Cow::Owned(live), Cow::Owned(current_signs)))
    }

    /// Return exact live topology and ordered sign images from one live application.
    /// # Errors
    /// Refuses foreign stores, changed originals, changed signs and exhausted resources.
    pub fn live_sign_predicates<'a>(
        &'a self,
        cas: &Cas,
        current: &TimingRequest,
        b: &mut Budget,
    ) -> Result<(Cow<'a, Topology>, Cow<'a, [SignPredicate]>), Fault> {
        self.apply(cas, current, b)
    }
}

impl Cas {
    /// Construct an untrusted timing proposal and admit it only through the independent checker.
    /// # Errors
    /// Refuses invalid original supports, incomplete lifts, uncertain signs or exhausted resources.
    pub fn construct_lc_affine_timing(
        &self,
        request: &TimingRequest,
        b: &mut Budget,
    ) -> Result<Receipt, Fault> {
        let proposed = claim::propose(request, b)?;
        check::verify(request, &proposed, b)?;
        let scope = wire::scope(request, b)?;
        let bytes = wire::encode(&scope, &proposed, b)?;
        retain(self, request, proposed, &bytes, b)
    }

    /// Replay an untrusted timing topology against this receiver's exact expected originals.
    /// # Errors
    /// Refuses any changed original, malformed incidence or exhausted resource atomically.
    pub fn check_lc_affine_timing(
        &mut self,
        expected: &TimingRequest,
        certificate: &Certificate,
        b: &mut Budget,
    ) -> Result<Receipt, Fault> {
        let scope = wire::scope(expected, b)?;
        let proposed = wire::decode(&certificate.0, &scope, b)?;
        self.transaction(b, |candidate, b| {
            check::verify(expected, &proposed, b)?;
            retain(candidate, expected, proposed, &certificate.0, b)
        })
    }
}

/// Charge and retain one checked store scoped timing receipt.
fn retain(
    cas: &Cas,
    request: &TimingRequest,
    topology: Topology,
    bytes: &[u8],
    b: &mut Budget,
) -> Result<Receipt, Fault> {
    let original_signs = claim::predicates(request, b)?;
    let storage = (size_of::<Receipt>() as u64)
        .saturating_add(
            (request.observations.len() as u64)
                .saturating_mul(size_of::<super::TimingObservation>() as u64),
        )
        .saturating_add(
            (topology.vertices.len() as u64).saturating_mul(size_of::<super::Vertex>() as u64),
        )
        .saturating_add(
            (topology.edges.len() as u64).saturating_mul(size_of::<super::Edge>() as u64),
        )
        .saturating_add(
            (original_signs.len() as u64).saturating_mul(size_of::<SignPredicate>() as u64),
        )
        .saturating_add(bytes.len() as u64);
    b.check(BudgetKind::OutputBytes, storage, AT)?;
    b.check(BudgetKind::StagedPayloadBytes, storage, AT)?;
    b.work(storage, AT)?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    wire.extend_from_slice(bytes);
    Ok(Receipt {
        original: request.clone(),
        topology,
        original_signs,
        wire,
        owner: Arc::clone(&cas.memo_owner),
    })
}

impl Receipt {
    /// Restore every original timestamp at a point of the current projected fibre.
    /// Each quotient is selected from the exact nonempty time/phase intersection.
    /// # Errors
    /// Refuses a point outside the current fibre, changed incidence or exact uncertainty.
    pub fn restore(
        &self,
        cas: &Cas,
        current: &TimingRequest,
        point: &[super::PiAffine; 2],
        b: &mut Budget,
    ) -> Result<super::TimingWitness, Fault> {
        self.apply(cas, current, b)?;
        let (rows, pi) = claim::rows(current, b)?;
        for row in &rows {
            if claim::residual(row, point, b)?.sign(&pi, b)? == std::cmp::Ordering::Greater {
                return Err(claim::invalid());
            }
        }
        let rate = &point[0];
        let initial = &point[1];
        let mut timestamps = Vec::new();
        timestamps
            .try_reserve_exact(current.observations.len())
            .map_err(|_| Fault::allocation(AT))?;
        for item in &current.observations {
            let turn = item.lift.checked_mul(2).ok_or_else(claim::invalid)?;
            let shift = crate::Rational::from_ratio(turn, 1, b)?;
            let phase_low = super::PiAffine {
                base: item.phase[0].clone(),
                pi: shift.clone(),
            };
            let phase_high = super::PiAffine {
                base: item.phase[1].clone(),
                pi: shift,
            };
            let time_low = rate.mul(&item.time[0], b)?;
            let phase_time_low = phase_low.sub(initial, b)?;
            let numerator =
                if time_low.compare(&phase_time_low, &pi, b)? == std::cmp::Ordering::Less {
                    phase_time_low
                } else {
                    time_low
                };
            let time_high = rate.mul(&item.time[1], b)?;
            let phase_time_high = phase_high.sub(initial, b)?;
            if numerator.compare(&time_high, &pi, b)? == std::cmp::Ordering::Greater
                || numerator.compare(&phase_time_high, &pi, b)? == std::cmp::Ordering::Greater
            {
                return Err(claim::invalid());
            }
            timestamps.push(super::QuotientTimestamp {
                numerator,
                denominator: rate.clone(),
            });
        }
        Ok(super::TimingWitness {
            rate: rate.clone(),
            initial: initial.clone(),
            timestamps,
        })
    }
}
