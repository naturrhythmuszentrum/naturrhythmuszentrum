// SPDX-License-Identifier: AGPL-3.0-only
//! Complete lifted affine fibre with independent timestamp errors.

mod check;
mod claim;
mod geometry;
mod service;
#[cfg(test)]
mod tests;
mod wire;

use super::{PiAffine, QuotientTimestamp, SignPredicate};
use crate::{Location, Rational, pi_bounds::PiBoundCertificate};
pub use service::{TimingClockPhaseCertificate, TimingClockPhaseReceipt};

/// General timing and clock-phase fibre fault location.
const AT: Location = Location::new("LC affine timing clock phase");
/// Topology proposal fault location.
const PROPOSAL_AT: Location = Location::new("LC affine timing clock phase proposal");
/// Receipt replay fault location.
const REPLAY_AT: Location = Location::new("LC affine timing clock phase replay");
/// Exact topology premise location.
const TOPOLOGY_AT: Location = Location::new("LC affine timing clock phase topology");
/// Symbolic sign-stratum premise location.
const SIGN_AT: Location = Location::new("LC affine timing clock phase sign stratum");
/// Pi-interval premise location.
const PI_AT: Location = Location::new("LC affine timing clock phase pi stratum");

/// Formal schema for the timing and clock-phase fibre.
pub(super) const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: "lc-affine-fibre-timing-clock-phase",
    label: "thm:cas:lc-affine-fibre-timing-clock-phase",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "For a complete ordered original timing, common-clock and common-phase inventory and one checked integer-lift branch, the exact affine fibre with one timestamp-error coordinate per observation has exhaustive finite vertex and edge incidence; every point jointly restores the clock offset, physical initial phase, every timestamp error, every physical timestamp and both phase layers.",
    premises: "Positive bounded rate, closed bounded initial, clock and phase-offset supports, every original reported time, independent timestamp-error interval, physical timestamp and measured/physical phase declaration, complete finite turn domain per observation, exact pi authority, every affine row, exhaustive independent-row intersections, active rows, independently reconstructed edges and ordered exact sign stratum. Current source versions require a live check; no whole-family union or native enclosure is established.",
};

/// One original observation and its own independent timestamp error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingClockPhaseObservation {
    /// Exact reported time relative to the common epoch.
    pub reported: Rational,
    /// Effective independent timestamp-error interval.
    pub error: [Rational; 2],
    /// Every original timestamp-error declaration.
    pub error_declarations: Vec<[Rational; 2]>,
    /// Every original physical timestamp declaration.
    pub time_declarations: Vec<[Rational; 2]>,
    /// Every original measured-phase declaration.
    pub measured_declarations: Vec<[Rational; 2]>,
    /// Every original physical-phase declaration.
    pub physical_declarations: Vec<[Rational; 2]>,
    /// Selected integer turn for both phase layers.
    pub lift: i64,
    /// Complete certified finite turn domain.
    pub lift_domain: [i64; 2],
}

/// Kind and observation position of one original declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingClockPhaseSourceKind {
    /// Positive rate.
    Rate,
    /// Physical initial phase.
    Initial,
    /// Shared clock offset.
    Clock,
    /// Shared phase offset.
    Offset,
    /// Independent timestamp error at an observation.
    Error(usize),
    /// Physical timestamp at an observation.
    Time(usize),
    /// Measured phase at an observation.
    Measured(usize),
    /// Physical phase at an observation.
    Physical(usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One original declaration position.
pub struct TimingClockPhaseSourceSlot {
    /// Original declaration kind and observation index.
    pub kind: TimingClockPhaseSourceKind,
    /// Occurrence within its original kind.
    pub declaration: usize,
}

/// Complete ordered originals for `(a,gamma,s,z,w_0,...,w_n)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingClockPhaseRequest {
    /// Effective positive rate support.
    pub rate: [Rational; 2],
    /// Every original rate support.
    pub rate_declarations: Vec<[Rational; 2]>,
    /// Effective physical initial-phase support.
    pub initial: [Rational; 2],
    /// Every original initial-phase support.
    pub initial_declarations: Vec<[Rational; 2]>,
    /// Effective shared clock-offset support.
    pub clock: [Rational; 2],
    /// Every original clock-offset declaration.
    pub clock_declarations: Vec<[Rational; 2]>,
    /// Effective shared phase-offset support.
    pub offset: [Rational; 2],
    /// Every original phase-offset declaration.
    pub offset_declarations: Vec<[Rational; 2]>,
    /// Original observations in their authenticated order.
    pub observations: Vec<TimingClockPhaseObservation>,
    /// Original common epoch identity.
    pub epoch: String,
    /// Complete ordered original source slots.
    pub source_order: Vec<TimingClockPhaseSourceSlot>,
    /// Authenticated source versions in slot order.
    pub source_versions: Vec<u64>,
    /// Exact pi authority for selected turns.
    pub pi: PiBoundCertificate,
    /// Semantic schema revision.
    pub revision: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One exact feasible vertex and all active original rows.
pub struct TimingClockPhaseVertex {
    /// Exact coordinates in `(a,gamma,s,z,w_0,...)` order.
    pub point: Vec<PiAffine>,
    /// Every original row active at this vertex.
    pub active: Vec<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// One maximal edge and all original supporting rows.
pub struct TimingClockPhaseEdge {
    /// Endpoint IDs in canonical vertex order.
    pub ends: [u32; 2],
    /// Every original row supporting this edge.
    pub supports: Vec<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// Complete bounded lifted vertex and edge incidence.
pub struct TimingClockPhaseTopology {
    /// Affine dimension, or none for an empty fibre.
    pub dimension: Option<u8>,
    /// All exact feasible vertices in lexicographic order.
    pub vertices: Vec<TimingClockPhaseVertex>,
    /// All maximal one-dimensional faces.
    pub edges: Vec<TimingClockPhaseEdge>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// Joint physical witness restored from one lifted point.
pub struct TimingClockPhaseWitness {
    /// Positive physical rate.
    pub rate: PiAffine,
    /// Measured-phase intercept gamma.
    pub measured_intercept: PiAffine,
    /// Shared physical phase offset.
    pub phase_offset: PiAffine,
    /// Shared clock offset z/a.
    pub clock_offset: QuotientTimestamp,
    /// Physical initial phase gamma+s-z.
    pub initial: PiAffine,
    /// Each independent timestamp error `w_j/a`.
    pub errors: Vec<QuotientTimestamp>,
    /// Each physical timestamp `r_j+(z+w_j)/a`.
    pub timestamps: Vec<QuotientTimestamp>,
    /// Measured lifted phases in observation order.
    pub measured: Vec<PiAffine>,
    /// Physical lifted phases in observation order.
    pub physical: Vec<PiAffine>,
}
/// Ordered sign predicates for the retained topology.
type Signs = Vec<SignPredicate>;
