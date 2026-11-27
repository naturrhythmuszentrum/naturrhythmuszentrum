// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite affine timing fibres with complete row incidence.

mod check;
mod claim;
mod clock_phase;
mod service;
#[cfg(test)]
mod tests;
mod timing_clock_phase;
mod wire;

pub use clock_phase::{
    ClockPhaseCertificate, ClockPhaseEdge, ClockPhaseObservation, ClockPhaseReceipt,
    ClockPhaseRequest, ClockPhaseSourceKind, ClockPhaseSourceSlot, ClockPhaseTopology,
    ClockPhaseVertex, ClockPhaseWitness,
};
pub use service::{Certificate, Receipt};
pub use timing_clock_phase::{
    TimingClockPhaseCertificate, TimingClockPhaseEdge, TimingClockPhaseObservation,
    TimingClockPhaseReceipt, TimingClockPhaseRequest, TimingClockPhaseSourceKind,
    TimingClockPhaseSourceSlot, TimingClockPhaseTopology, TimingClockPhaseVertex,
    TimingClockPhaseWitness,
};

/// Registered common-clock and common-phase fibre theorem.
pub const CLOCK_PHASE_SCHEMA: crate::proof::LemmaSchema = clock_phase::SCHEMA;
/// Registered timing, common-clock and common-phase fibre theorem.
pub const TIMING_CLOCK_PHASE_SCHEMA: crate::proof::LemmaSchema = timing_clock_phase::SCHEMA;

use crate::pi_bounds::PiBoundCertificate;
use crate::{Location, Rational};

/// Logical location for timing refusals.
const AT: Location = Location::new("LC affine fibre");
/// A changed projected topology cannot use the retained timing stratum.
const TOPOLOGY_AT: Location = Location::new("LC affine fibre topology");
/// Changed exact predicate signs cannot use the retained timing stratum.
const SIGN_AT: Location = Location::new("LC affine fibre sign stratum");
/// Registered theorem recipe name.
const RECIPE: &str = "lc-affine-fibre";

/// An exact rational plus an exact rational multiple of mathematical pi.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PiAffine {
    /// Rational part.
    pub base: Rational,
    /// Coefficient of mathematical pi, never a rational approximation.
    pub pi: Rational,
}

impl PiAffine {
    /// Embed an exact rational.
    #[must_use]
    pub fn rational(base: Rational) -> Self {
        Self {
            base,
            pi: Rational::zero(),
        }
    }
}

/// One original independent timestamp and unlifted phase support.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingObservation {
    /// Closed finite timestamp interval.
    pub time: [Rational; 2],
    /// Closed finite measured phase interval before the integer turn.
    pub phase: [Rational; 2],
    /// Every original timestamp declaration, including repetitions.
    pub time_declarations: Vec<[Rational; 2]>,
    /// Every original measured-phase declaration, including repetitions.
    pub phase_declarations: Vec<[Rational; 2]>,
    /// Chosen integer turn for this branch.
    pub lift: i64,
    /// Complete checked integer lift interval, including this branch.
    pub lift_domain: [i64; 2],
}

/// Kind of one original declaration in its unmodified input order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingSourceKind {
    /// Positive rate support.
    Rate,
    /// Physical initial phase support.
    Initial,
    /// Timestamp support for the indexed observation.
    Time(usize),
    /// Measured phase support for the indexed observation.
    Phase(usize),
}

/// Position of one declaration within its kind and observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingSourceSlot {
    /// Original source kind.
    pub kind: TimingSourceKind,
    /// Zero-based declaration occurrence within that kind.
    pub declaration: usize,
}

/// Complete original inventory for one timing branch, in original order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingRequest {
    /// Closed rate support, strictly separated from zero.
    pub rate: [Rational; 2],
    /// Closed physical initial phase support.
    pub initial: [Rational; 2],
    /// Every original rate declaration, including repetitions.
    pub rate_declarations: Vec<[Rational; 2]>,
    /// Every original initial-phase declaration, including repetitions.
    pub initial_declarations: Vec<[Rational; 2]>,
    /// Every original observation, including repetitions.
    pub observations: Vec<TimingObservation>,
    /// Original common epoch identity.
    pub epoch: String,
    /// Original nominal timestamp coordinates in observation order.
    pub nominal: Vec<Rational>,
    /// Original declaration order, preserving all repeated positions.
    pub source_order: Vec<TimingSourceSlot>,
    /// Opaque authenticated source versions in the same order as the inventory.
    pub source_versions: Vec<u64>,
    /// Exact checked pi authority, used only to decide affine signs.
    pub pi: PiBoundCertificate,
    /// Semantic schema version.
    pub revision: u32,
}

/// Dimension of a finite projected fibre.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dimension {
    /// No feasible projected point.
    Empty,
    /// Exactly one projected point.
    Point,
    /// A closed line segment.
    Segment,
    /// A bounded full-dimensional polygon.
    Polygon,
}

/// A checked candidate vertex and all original rows active at it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vertex {
    /// Exact projected rate and initial phase.
    pub point: [PiAffine; 2],
    /// Original row IDs in declaration order.
    pub active: Vec<u32>,
}

/// A maximal boundary edge with all supporting original rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    /// Endpoint vertex IDs in sorted vertex order.
    pub ends: [u32; 2],
    /// Supporting original row IDs in declaration order.
    pub supports: Vec<u32>,
}

/// Proposed complete projected topology; checker reconstructs it independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Topology {
    /// Empty, point, segment or polygon.
    pub dimension: Dimension,
    /// Every feasible independent-row intersection, with duplicates merged.
    pub vertices: Vec<Vertex>,
    /// Every maximal boundary edge.
    pub edges: Vec<Edge>,
}

/// Exact current image and required sign of one ordered timing predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignPredicate {
    /// Sign retained by the admitted original stratum.
    pub required: std::cmp::Ordering,
    /// Certified rational enclosure of the exact expression, including pi.
    pub image: [Rational; 2],
}

/// Exact quotient representing one restored physical timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotientTimestamp {
    /// Numerator selected from the original nonempty intersection.
    pub numerator: PiAffine,
    /// Strictly positive projected rate; division preserves all inequalities.
    pub denominator: PiAffine,
}

/// Joint witness for one projected point and every original timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingWitness {
    /// Projected positive rate.
    pub rate: PiAffine,
    /// Projected initial phase.
    pub initial: PiAffine,
    /// One exact quotient in original observation order.
    pub timestamps: Vec<QuotientTimestamp>,
}

/// Registered timing theorem schema. A receipt is branch scoped and store scoped.
pub const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: RECIPE,
    label: "thm:cas:lc-affine-fibre-timing",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "For the complete ordered original timing inventory and one checked integer-lift branch, the exact rate/initial-phase image is the finite polygon obtained by eliminating every independent timestamp. Empty, point, segment and polygon incidences are exhaustive; every projected point restores every original timestamp in its own nonempty interval intersection.",
    premises: "Positive bounded rate, bounded closed initial phase, closed finite timestamp and phase supports, an independently checked complete integer lift interval per observation, exact pi authority, all original affine rows, exhaustive pair intersections, feasibility, duplicate and edge incidence. A changed source version requires a new live stratum check; this receipt does not close a whole-branch union or a native enclosure by itself.",
};
