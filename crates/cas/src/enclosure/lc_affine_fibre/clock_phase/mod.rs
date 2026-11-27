// SPDX-License-Identifier: AGPL-3.0-only
//! Complete common-clock and common-phase lifted affine branch.

mod check;
mod claim;
mod geometry;
mod service;
#[cfg(test)]
mod tests;
mod wire;

/// Dynamic storage of the cloned original request.
pub(super) fn clock_phase_request_storage(request: &ClockPhaseRequest) -> u64 {
    service::request_storage_bytes(request)
}
pub use service::{ClockPhaseCertificate, ClockPhaseReceipt};

use super::{PiAffine, QuotientTimestamp, SignPredicate};
use crate::{Location, Rational, pi_bounds::PiBoundCertificate};

/// Original-inventory and resource refusal stage.
const AT: Location = Location::new("LC affine clock phase");
/// Registered common-clock and common-phase branch theorem.
pub(super) const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: "lc-affine-fibre-clock-phase",
    label: "thm:cas:lc-affine-fibre-clock-phase",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "For one complete ordered original common-clock and common-phase inventory and one checked integer-lift branch, the exact four-coordinate affine fibre has the exhaustive finite vertex and edge incidence returned by the checker; every point restores the common clock offset, physical initial phase and both phase layers for every observation.",
    premises: "Positive bounded rate, closed bounded initial, clock and phase-offset supports, every original reported time and physical timestamp declaration, both measured and physical phase declarations, one shared complete integer-turn domain per observation, exact pi authority, all affine rows, exhaustive feasible four-row intersections, active rows, independent edge incidence and ordered original sign stratum. Changed source versions require a fresh live check; the receipt does not close a whole-family union or a native enclosure.",
};
/// Exact proposal resource refusal stage.
const PROPOSAL_AT: Location = Location::new("LC affine clock phase proposal");
/// Receiving-store replay resource refusal stage.
const REPLAY_AT: Location = Location::new("LC affine clock phase replay");
/// Changed topology refusal stage.
const TOPOLOGY_AT: Location = Location::new("LC affine clock phase topology");
/// Changed exact sign inventory refusal stage.
const SIGN_AT: Location = Location::new("LC affine clock phase sign stratum");

/// One original reported-time, measured-phase and physical-phase inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseObservation {
    /// Exact reported time relative to the common epoch.
    pub reported: Rational,
    /// Original physical timestamp declarations, if any.
    pub time_declarations: Vec<[Rational; 2]>,
    /// Original measured phase declarations before the common phase offset.
    pub measured_declarations: Vec<[Rational; 2]>,
    /// Original physical phase declarations after the common phase offset.
    pub physical_declarations: Vec<[Rational; 2]>,
    /// Selected integer turn for both phase layers.
    pub lift: i64,
    /// Independently certified complete finite integer branch interval.
    pub lift_domain: [i64; 2],
}

/// Kind of one original declaration in its authenticated order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockPhaseSourceKind {
    /// Positive rate.
    Rate,
    /// Physical initial phase.
    Initial,
    /// Common clock offset.
    Clock,
    /// Common phase offset.
    Offset,
    /// Original physical timestamp at one observation.
    Time(usize),
    /// Original measured phase at one observation.
    Measured(usize),
    /// Original physical phase at one observation.
    Physical(usize),
}

/// Position of one original source declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockPhaseSourceSlot {
    /// Source kind and observation index, if present.
    pub kind: ClockPhaseSourceKind,
    /// Occurrence within that kind.
    pub declaration: usize,
}

/// Complete original common-clock and common-phase branch inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseRequest {
    /// Effective positive rate support.
    pub rate: [Rational; 2],
    /// Every original rate support.
    pub rate_declarations: Vec<[Rational; 2]>,
    /// Effective physical initial phase support.
    pub initial: [Rational; 2],
    /// Every original physical initial phase support.
    pub initial_declarations: Vec<[Rational; 2]>,
    /// Effective common clock offset support.
    pub clock: [Rational; 2],
    /// Every original common clock declaration.
    pub clock_declarations: Vec<[Rational; 2]>,
    /// Effective common phase offset support.
    pub offset: [Rational; 2],
    /// Every original common phase offset declaration.
    pub offset_declarations: Vec<[Rational; 2]>,
    /// Ordered original observations.
    pub observations: Vec<ClockPhaseObservation>,
    /// Original common epoch identity.
    pub epoch: String,
    /// Complete source declaration order.
    pub source_order: Vec<ClockPhaseSourceSlot>,
    /// Authenticated source versions in that same order.
    pub source_versions: Vec<u64>,
    /// Exact pi authority for selected integer turns.
    pub pi: PiBoundCertificate,
    /// Semantic schema revision.
    pub revision: u32,
}

/// One exact feasible lifted point in `(a,gamma,s,z)` order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseVertex {
    /// Exact coordinates, with pi coefficients retained.
    pub point: [PiAffine; 4],
    /// Every original row active at this point.
    pub active: Vec<u32>,
}

/// One maximal one-dimensional face and every original supporting row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseEdge {
    /// Endpoint vertex IDs in canonical order.
    pub ends: [u32; 2],
    /// Every original row active at both endpoints.
    pub supports: Vec<u32>,
}

/// Complete bounded lifted vertex and edge incidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseTopology {
    /// Affine dimension, or `None` for the empty fibre.
    pub dimension: Option<u8>,
    /// Exact lexicographically ordered vertices.
    pub vertices: Vec<ClockPhaseVertex>,
    /// Maximal one-dimensional faces in vertex order.
    pub edges: Vec<ClockPhaseEdge>,
}

/// Joint physical witness restored from one checked lifted point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockPhaseWitness {
    /// Positive physical rate.
    pub rate: PiAffine,
    /// Measured intercept gamma.
    pub measured_intercept: PiAffine,
    /// One common phase offset.
    pub phase_offset: PiAffine,
    /// One common clock offset `z/a`, retaining its positive denominator.
    pub clock_offset: QuotientTimestamp,
    /// Physical initial phase `gamma+s-z`.
    pub initial: PiAffine,
    /// Exact measured lifted phases for every observation.
    pub measured: Vec<PiAffine>,
    /// Exact physical lifted phases for every observation.
    pub physical: Vec<PiAffine>,
}

/// Ordered predicate inventory for one current branch.
type Signs = Vec<SignPredicate>;
