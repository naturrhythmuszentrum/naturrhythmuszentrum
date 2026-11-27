// SPDX-License-Identifier: AGPL-3.0-only
//! Checked complete affine-fibre projections and whole-future-ray extrema.

mod check;
mod claim;
mod service;
#[cfg(test)]
mod tests;
mod wire;

use crate::enclosure::lc_affine_fibre::{
    Certificate as TimingCertificate, ClockPhaseCertificate, ClockPhaseRequest, PiAffine,
    TimingClockPhaseCertificate, TimingClockPhaseRequest, TimingRequest,
};
use crate::{Location, Rational};
pub use service::{FutureRayCertificate, FutureRayReceipt, FutureRayWitness};

/// Original claim and resource fault location.
const AT: Location = Location::new("LC future ray");
/// Exact reference-premise fault location.
const REFERENCE_AT: Location = Location::new("LC future ray reference");
/// Whole-ray extremality fault location.
const EXTREMUM_AT: Location = Location::new("LC future ray extremum");
/// Projected vertex ordering and convex-hull premise fault location.
const TOPOLOGY_AT: Location = Location::new("LC future ray topology");
/// Gauge-union fault location.
const UNION_AT: Location = Location::new("LC future ray gauge union");
/// Portable replay fault location.
const REPLAY_AT: Location = Location::new("LC future ray replay");

/// Registered plain and narrow future-ray lemma.
pub const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: "lc-future-ray",
    label: "thm:cas:lc-future-ray",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "For a nonempty complete admitted affine fibre and a checked rational reference after its observation window, the exact projected polygon and four inequalities at every vertex certify one lower and one upper affine line over the whole declared future ray; interpolation of their original lifted vertices realizes every intervening phase value. A complete gauge-equivalent union has the same wrapped image.",
    premises: "One checked timing, clock-phase or timing-clock-phase original certificate for plain actual time, or clock-phase/timing-clock-phase for narrow reported time; all original measured and physical strips in narrow form; exact pi authority, complete finite lift domains, rational reference at or after the safe reference and strictly after the observation window, all projected source vertices and their convex hull, four exact signed ray differences per vertex, positive rate, and verified bidirectional full-turn maps for every branch in a complete union. Changed originals and reference require a fresh exact check; broad, non-equivalent unions and native enclosure remain outside this lemma.",
};

/// The two admitted future query coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutureRayForm {
    /// Physical actual time with intercept alpha.
    Plain,
    /// Reported time with intercept beta=gamma+s.
    Narrow,
}

/// A complete original branch and its portable, independently checked fibre certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FutureRaySource {
    /// Independent timing and one selected lift branch.
    Timing(TimingRequest, TimingCertificate),
    /// One common clock and phase offset with zero independent errors.
    ClockPhase(ClockPhaseRequest, ClockPhaseCertificate),
    /// Common offsets and all independent original timestamp errors.
    TimingClockPhase(TimingClockPhaseRequest, TimingClockPhaseCertificate),
}

/// One declared branch in a complete same-gauge union.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureRayBranch {
    /// Stable branch identity in original declaration order.
    pub id: u64,
    /// Exact full-turn shift from the representative branch.
    pub shift: i64,
    /// Complete original source and its fibre certificate.
    pub source: FutureRaySource,
}

/// A rational ray explicitly narrowed from its exact safe reference if needed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureRayRequest {
    /// Plain actual-time or narrow reported-time contract.
    pub form: FutureRayForm,
    /// Exact rational beginning of the certified ray.
    pub reference: Rational,
    /// Exact safe reference, possibly containing mathematical pi.
    pub safe_reference: PiAffine,
    /// True only when the caller explicitly requests the later rational ray.
    pub narrowed: bool,
    /// Original physical or reported epoch identity.
    pub epoch: String,
    /// Complete ordered branch inventory; the first is the representative.
    pub branches: Vec<FutureRayBranch>,
}

/// One exact projected source vertex, retaining its original lifted preimage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureRayVertex {
    /// Branch index in the complete original inventory.
    pub branch: u32,
    /// Source-topology vertex index within that branch.
    pub source_vertex: u32,
    /// Positive rate and actual or reported intercept.
    pub point: [PiAffine; 2],
}

/// Complete convex hull and the finite whole-ray proof over all projected vertices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureRayProposal {
    /// Canonical unique source-projected vertices; their convex hull is the image polygon.
    pub vertices: Vec<FutureRayVertex>,
    /// Canonical counterclockwise convex-hull vertex IDs, or one/two degenerate endpoints.
    pub hull: Vec<u32>,
    /// Selected lower ray line's projected vertex ID.
    pub low: u32,
    /// Selected upper ray line's projected vertex ID.
    pub high: u32,
    /// Four exact affine-pi differences for every projected vertex in order.
    pub comparisons: Vec<[PiAffine; 4]>,
}
