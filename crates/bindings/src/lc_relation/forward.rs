// SPDX-License-Identifier: AGPL-3.0-only
//! Complete floor-cell images of the supported positive independent LC rectangle.

mod rectangle;

use super::{AT, LcInput, LcQuantity, LcRelation, LcRelationError, LcValues, LcWitness, storage};
use crate::{Budget, ExactConstant};
pub use nrz_cas::lattice::{PiAffineEndpoint as LcPhaseEndpoint, PiIntervalArc as LcForwardArc};
use nrz_cas::{
    Rational,
    lattice::PiIntervalCover,
    limits::BudgetUsage,
    pi_bounds::{CheckedPiBounds, PiBoundCertificate},
};
use rectangle::Rectangle;
use std::rc::Rc;

/// Structural information is separate from the certainty of its evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcDeterminacy {
    /// A named continuous physical direction remains unconstrained.
    Underdetermined,
    /// An exact contradiction excludes every original branch.
    Impossible,
    /// The supported observation map determines the projection within declared uncertainty.
    DeterminedFinite,
    /// The observed projection retains a complete infinite discrete alias family.
    DeterminedInfinite,
}

/// Numerical certainty is not inferred from structural determination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcCertainty {
    /// Complete coverage, nonemptiness and the stated structural class are established.
    Certified,
    /// A covering result retains unresolved proof obligations.
    Undecided,
}

/// A complete finite indexed union, retaining the exact positive rectangle and time query.
///
/// Every integer in the inclusive cycle range is retained, without materializing a prefix.
/// Positive multiplication, inversion and the continuous rectangle map prove that every
/// lifted phase in the stored interval is attained. No live heap input is authenticated.
#[derive(Clone, Debug)]
pub struct LcForwardCover(Rc<ForwardData>);

/// Exact coverage and existence data are shared independently of numerical rendering.
#[derive(Debug)]
struct ForwardData {
    /// Original simultaneous constraints, all checked for this adapter's supported domain.
    relation: LcRelation,
    /// Intersected positive element and phase bounds with a positive admissible energy.
    rectangle: Rectangle,
    /// Original deterministic duration query relative to the same epoch.
    time: LcInput,
    /// CAS-owned complete floor cover and independently checked Machin certificate.
    mathematical: PiIntervalCover,
    /// Before/after checking counters on the caller's cumulative ledger.
    usage: [BudgetUsage; 2],
    /// Conservative new exact ownership, excluding the shared original relation.
    bytes: u64,
}

impl LcRelation {
    /// Check the complete forward phase image of independent positive element boxes.
    ///
    /// This closed adapter accepts finite rational L/C/initial-phase boxes, optional
    /// positive rational energy boxes, no observations, and nonnegative rational time.
    /// The extremal rates must reduce to rationals in the existing exact root fragment.
    /// Proposed first/last floor indices are checked against the derived endpoints;
    /// they are not search bounds. Every intermediate index remains in the answer.
    /// # Errors
    /// Refuses unsupported or empty constraints, unproved endpoint cells, malformed
    /// pi evidence and exhausted caller resources. No omitted-constraint answer is returned.
    pub fn forward_cover(
        &self,
        time: LcInput,
        cycles: [Rational; 2],
        pi: &PiBoundCertificate,
        budget: &mut Budget,
    ) -> Result<LcForwardCover, LcRelationError> {
        let before = budget.usage();
        let rectangle = Rectangle::check(self, budget)?;
        if time.quantity() != LcQuantity::Time(0) {
            return Err(LcRelationError::Meaning);
        }
        let times = rectangle::bounds(time.interval(), budget)?;
        if times[0].is_negative() {
            return Err(LcRelationError::Projection);
        }
        let lifted = rectangle.lifted(&times, budget)?;
        let mathematical = PiIntervalCover::check(lifted, cycles, pi, budget)?;
        let mut bytes = rectangle.bytes().saturating_add(time.interval().bytes());
        bytes = bytes
            .saturating_add(mathematical.retained_payload_bytes())
            .saturating_add(4_096);
        storage::retained(bytes.saturating_add(self.retained_payload_bytes()), budget)?;
        Ok(LcForwardCover(Rc::new(ForwardData {
            relation: self.clone(),
            rectangle,
            time,
            mathematical,
            usage: [before, budget.usage()],
            bytes,
        })))
    }
}

impl LcForwardCover {
    /// Exact immutable question identity, including query and branch cover.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Complete original relation; no constraint has been silently dropped.
    #[must_use]
    pub fn relation(&self) -> &LcRelation {
        &self.0.relation
    }

    /// Original time support, relative to the retained relation's epoch.
    #[must_use]
    pub fn time(&self) -> &LcInput {
        &self.0.time
    }

    /// Exact lifted image before wrapping; the rectangle map realizes its entire interval.
    #[must_use]
    pub fn lifted(&self) -> &[Rational; 2] {
        self.0.mathematical.interval()
    }

    /// Inclusive first/last indices of the complete integer family.
    #[must_use]
    pub fn cycles(&self) -> &[Rational; 2] {
        self.0.mathematical.indices()
    }

    /// Structural determination of this phase projection within the supplied uncertainty.
    #[must_use]
    pub const fn determinacy(&self) -> LcDeterminacy {
        LcDeterminacy::DeterminedFinite
    }

    /// Exact covering and continuous-image existence are both checked for this restricted domain.
    #[must_use]
    pub const fn certainty(&self) -> LcCertainty {
        LcCertainty::Certified
    }

    /// Exact pi evidence retained with the floor-cell cover.
    #[must_use]
    pub fn pi_bounds(&self) -> &CheckedPiBounds {
        self.0.mathematical.pi_bounds()
    }

    /// Materialize one arc after checking its exact membership in the complete index range.
    /// # Errors
    /// Refuses nonintegers, out-of-range indices or exhausted caller resources.
    pub fn arc(
        &self,
        index: Rational,
        budget: &mut Budget,
    ) -> Result<LcForwardArc, LcRelationError> {
        Ok(self.0.mathematical.arc(index, budget)?)
    }

    /// Check an exact parameter instance of the retained positive rectangle map.
    ///
    /// For `P=1/a²` choose `L=max(L_min,P/C_max)`, `C=P/L` and amplitudes from the
    /// retained positive energy. The resulting tuple is checked against all original inputs.
    /// # Errors
    /// Refuses parameters outside the original rectangle or unsupported exact arithmetic.
    pub fn witness(
        &self,
        rate: Rational,
        phase: Rational,
        budget: &mut Budget,
    ) -> Result<LcWitness, LcRelationError> {
        let values = self.0.rectangle.values(rate, phase, budget)?;
        self.0.relation.check_witness(values, budget)
    }

    /// Original exact checking snapshots, independent of later numerical lowering.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }

    /// Conservative new exact metadata ownership.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
