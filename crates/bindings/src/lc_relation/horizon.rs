// SPDX-License-Identifier: AGPL-3.0-only
//! Published forward certainty horizons retain their exact joint phase/rate derivation.

use super::{AT, LcCertainty, LcPairsCover, LcRelation, LcRelationError, LcZeroTimes, storage};
use crate::{
    Budget, Descriptor, LcCalibratedFuture, LcClockCover, LcQuantity, LcSharedTimingCover,
    LcTimingCover,
};
use nrz_cas::{
    Rational,
    lattice::PiWidthHorizon,
    limits::BudgetUsage,
    pi_bounds::{CheckedPiBounds, PiBoundCertificate},
};
use std::rc::Rc;

pub use nrz_cas::lattice::PiHorizonValue as LcHorizonValue;

/// The future argument's exact relationship to the retained physical clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcHorizonCoordinate {
    /// Actual coherent time measured from the original physical epoch.
    ActualTime,
    /// A future reported reading r, with actual time `r+d` under the same retained shared offset.
    ReportedClock,
}

/// Original joint feasible set; every source retains its physical and measurement constraints.
#[derive(Clone, Debug)]
enum Source {
    /// Independent positive rate rectangle with exact starting phase zero.
    Rectangle(LcZeroTimes),
    /// Original phase and amplitude calibration fibres with checked future extremizers.
    Calibrated(LcCalibratedFuture),
    /// Retained correlated phase-at-rate fibres and complete original cycle branch.
    Pairs(LcPairsCover),
    /// Original independent timestamp intervals and complete physical phase/rate fibres.
    Timing(LcTimingCover),
    /// Original physical initial phase correlated with one shared clock offset.
    Clock(LcClockCover),
    /// Both original timing layers and their physical initial-phase/offset correlation.
    SharedTiming(LcSharedTimingCover),
}

impl Source {
    /// Exact original physical question.
    fn relation(&self) -> &LcRelation {
        match self {
            Self::Rectangle(cover) => cover.relation(),
            Self::Calibrated(cover) => cover.relation(),
            Self::Pairs(cover) => cover.relation(),
            Self::Timing(cover) => cover.relation(),
            Self::Clock(cover) => cover.relation(),
            Self::SharedTiming(cover) => cover.relation(),
        }
    }

    /// Conservative shared original source ownership.
    fn bytes(&self) -> u64 {
        match self {
            Self::Rectangle(cover) => cover.retained_payload_bytes(),
            Self::Calibrated(cover) => cover.retained_payload_bytes(),
            Self::Pairs(cover) => cover.retained_payload_bytes(),
            Self::Timing(cover) => cover.retained_payload_bytes(),
            Self::Clock(cover) => cover.retained_payload_bytes(),
            Self::SharedTiming(cover) => cover.retained_payload_bytes(),
        }
    }
}

/// Eventual loss of phase exclusion after an explicit reference in the declared future clock coordinate.
#[derive(Clone, Debug)]
pub struct LcHorizon(Rc<Data>);

/// Exact endpoint parameters give the whole lifted future image by convexity.
#[derive(Debug)]
struct Data {
    /// Complete original relation and constructive feasible family.
    source: Source,
    /// Explicit future-coordinate contract, including the retained reported-clock correlation.
    coordinate: LcHorizonCoordinate,
    /// Lower and upper extremizers, each in query-intercept/rate order.
    extrema: [[Rational; 2]; 2],
    /// Checked affine future width and finite/infinite/unresolved full-turn threshold.
    mathematical: PiWidthHorizon,
    /// Seconds and duration difference kind; no affine clock offset is inferred.
    descriptor: Descriptor,
    /// Before/after exact derivation counters.
    usage: [BudgetUsage; 2],
    /// Conservative newly retained exact ownership.
    bytes: u64,
}

impl LcZeroTimes {
    /// Derive the independent rectangle horizon `2*pi/(a_max-a_min)`.
    /// # Errors
    /// Refuses false pi evidence and exhausted resources; the complete time family survives.
    pub fn forward_horizon(
        &self,
        pi: &PiBoundCertificate,
        budget: &mut Budget,
    ) -> Result<LcHorizon, LcRelationError> {
        let before = budget.usage();
        budget.work(1, AT)?;
        let pi = pi.check(budget)?;
        let extrema = [
            [Rational::zero(), self.family().rate()[0].clone()],
            [Rational::zero(), self.family().rate()[1].clone()],
        ];
        LcHorizon::new(
            Source::Rectangle(self.clone()),
            LcHorizonCoordinate::ActualTime,
            extrema,
            Rational::zero(),
            pi,
            &before,
            budget,
        )
    }
}

impl LcHorizon {
    /// Retain original calibration fibres while reusing the checked equivalent future proof.
    pub(super) fn calibrated(
        source: LcCalibratedFuture,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        let auxiliary = source.auxiliary();
        if auxiliary.coordinate() != LcHorizonCoordinate::ReportedClock {
            return Err(LcRelationError::Projection);
        }
        let bytes = source.extrema().iter().flatten().fold(
            source
                .mathematical()
                .retained_payload_bytes()
                .saturating_add(2_048),
            |sum, value| sum.saturating_add(storage::rational(value)),
        );
        storage::retained(
            bytes
                .saturating_add(source.retained_payload_bytes())
                .saturating_add(source.relation().retained_payload_bytes()),
            budget,
        )?;
        let data = Data {
            coordinate: auxiliary.coordinate(),
            extrema: source.extrema().clone(),
            mathematical: source.mathematical().clone(),
            descriptor: auxiliary.descriptor().clone(),
            usage: [*before, budget.usage()],
            bytes,
            source: Source::Calibrated(source),
        };
        Ok(Self(Rc::new(data)))
    }

    /// Original joint calibrated owner and full original future extremizers, when applicable.
    #[must_use]
    pub fn calibrated_future(&self) -> Option<&LcCalibratedFuture> {
        match &self.0.source {
            Source::Calibrated(source) => Some(source),
            Source::Rectangle(_)
            | Source::Pairs(_)
            | Source::Timing(_)
            | Source::Clock(_)
            | Source::SharedTiming(_) => None,
        }
    }

    /// Preserve a proved correlated pair extremization, including the prepared pi certificate.
    pub(super) fn pairs(
        source: LcPairsCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        let pi = source.pi_bounds().clone();
        Self::new(
            Source::Pairs(source),
            LcHorizonCoordinate::ActualTime,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Retain original independent timing instead of publishing the auxiliary strip relation.
    pub(super) fn timing(
        source: LcTimingCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        Self::new(
            Source::Timing(source),
            LcHorizonCoordinate::ActualTime,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Actual-time future phases retain the physical initial-phase/shared-clock correlation.
    pub(super) fn clock(
        source: LcClockCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        Self::new(
            Source::Clock(source),
            LcHorizonCoordinate::ActualTime,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Both original timing layers and physical initial phase survive actual-time horizon derivation.
    pub(super) fn shared_timing(
        source: LcSharedTimingCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        Self::new(
            Source::SharedTiming(source),
            LcHorizonCoordinate::ActualTime,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Reported readings keep the original shared offset through `beta=alpha+a*d`.
    pub(super) fn clock_reported(
        source: LcClockCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        Self::new(
            Source::Clock(source),
            LcHorizonCoordinate::ReportedClock,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Both original time-error layers survive a future reading under the same shared offset.
    pub(super) fn shared_timing_reported(
        source: LcSharedTimingCover,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        Self::new(
            Source::SharedTiming(source),
            LcHorizonCoordinate::ReportedClock,
            extrema,
            reference,
            pi,
            before,
            budget,
        )
    }

    /// Exact positive-source extrema derive the entire future lifted interval.
    fn new(
        source: Source,
        coordinate: LcHorizonCoordinate,
        extrema: [[Rational; 2]; 2],
        reference: Rational,
        pi: CheckedPiBounds,
        before: &BudgetUsage,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        let width = [
            extrema[1][0].sub(&extrema[0][0], budget)?,
            extrema[1][1].sub(&extrema[0][1], budget)?,
        ];
        let mathematical = PiWidthHorizon::check(width, reference, pi, budget)?;
        let bytes = extrema.iter().flatten().fold(
            mathematical.retained_payload_bytes().saturating_add(2_048),
            |sum, v| sum.saturating_add(storage::rational(v)),
        );
        storage::retained(
            bytes
                .saturating_add(source.bytes())
                .saturating_add(source.relation().retained_payload_bytes()),
            budget,
        )?;
        Ok(Self(Rc::new(Data {
            source,
            coordinate,
            extrema,
            mathematical,
            descriptor: LcQuantity::NaturalTime.descriptor()?,
            usage: [*before, budget.usage()],
            bytes,
        })))
    }

    /// Original simultaneous relation with its unchanged common epoch and branch constraints.
    #[must_use]
    pub fn relation(&self) -> &LcRelation {
        self.0.source.relation()
    }

    /// Future actual time or a reported reading under the same retained shared clock offset.
    #[must_use]
    pub fn coordinate(&self) -> LcHorizonCoordinate {
        self.0.coordinate
    }

    /// Coherent seconds with duration difference meaning.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.descriptor
    }

    /// Exact affine width, future reference and constant evidence.
    #[must_use]
    pub fn mathematical(&self) -> &PiWidthHorizon {
        &self.0.mathematical
    }

    /// Exact value or explicit infinity/unresolved outcome; no cutoff determines this result.
    #[must_use]
    pub fn value(&self) -> &LcHorizonValue {
        self.0.mathematical.value()
    }

    /// Retained extremizers in query-intercept/rate order on the original local lift.
    ///
    /// The intercept is physical alpha for `ActualTime` and `beta=alpha+a*d` for `ReportedClock`.
    /// The original source retains the conditional physical phase and shared clock-offset fibre.
    #[must_use]
    pub fn extremal_parameters(&self) -> &[[Rational; 2]; 2] {
        &self.0.extrema
    }

    /// Whole-interval existence uses the admitted continuous rectangle or convex strip family.
    #[must_use]
    pub fn certainty(&self) -> LcCertainty {
        match self.value() {
            LcHorizonValue::Finite(_) | LcHorizonValue::Infinite => LcCertainty::Certified,
            LcHorizonValue::Undecided => LcCertainty::Undecided,
        }
    }

    /// Original correlated pair cover, when this horizon was recovered from observations.
    #[must_use]
    pub fn pairs_cover(&self) -> Option<&LcPairsCover> {
        match &self.0.source {
            Source::Pairs(cover) => Some(cover),
            Source::Calibrated(_)
            | Source::Rectangle(_)
            | Source::Timing(_)
            | Source::Clock(_)
            | Source::SharedTiming(_) => None,
        }
    }

    /// Original independent complete zero-time family, when applicable.
    #[must_use]
    pub fn zero_times(&self) -> Option<&LcZeroTimes> {
        match &self.0.source {
            Source::Rectangle(cover) => Some(cover),
            Source::Calibrated(_)
            | Source::Pairs(_)
            | Source::Timing(_)
            | Source::Clock(_)
            | Source::SharedTiming(_) => None,
        }
    }

    /// Original independent timestamp owner, including its unchanged observation inventory.
    #[must_use]
    pub fn timing_cover(&self) -> Option<&LcTimingCover> {
        match &self.0.source {
            Source::Timing(cover) => Some(cover),
            Source::Calibrated(_)
            | Source::Rectangle(_)
            | Source::Pairs(_)
            | Source::Clock(_)
            | Source::SharedTiming(_) => None,
        }
    }

    /// Original shared clock; `coordinate` declares the future argument's exact meaning.
    #[must_use]
    pub fn clock_cover(&self) -> Option<&LcClockCover> {
        match &self.0.source {
            Source::Clock(cover) => Some(cover),
            Source::Calibrated(_)
            | Source::Rectangle(_)
            | Source::Pairs(_)
            | Source::Timing(_)
            | Source::SharedTiming(_) => None,
        }
    }

    /// Original independent-error/shared-offset owner with the explicit future-coordinate contract.
    #[must_use]
    pub fn shared_timing_cover(&self) -> Option<&LcSharedTimingCover> {
        match &self.0.source {
            Source::SharedTiming(cover) => Some(cover),
            Source::Calibrated(_)
            | Source::Rectangle(_)
            | Source::Pairs(_)
            | Source::Timing(_)
            | Source::Clock(_) => None,
        }
    }

    /// Cumulative derivation snapshots, excluding numerical lowering.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }

    /// Conservative new exact ownership, excluding shared original sources.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Conservative scoped source ownership for downstream numerical checking.
    #[must_use]
    pub fn source_payload_bytes(&self) -> u64 {
        self.0
            .source
            .bytes()
            .saturating_add(self.relation().retained_payload_bytes())
    }
}

impl super::calibrated_future::LcAffineFutureRay {
    /// Apply the existing exact full-turn width law to this checked whole future ray.
    /// The current width checker accepts rational endpoint differences; symbolic pi
    /// differences retain the checked ray and use the unresolved horizon fallback.
    /// # Errors
    /// Refuses foreign stores, unsupported symbolic widths or exhausted resources.
    pub fn horizon(
        &self,
        cas: &nrz_cas::Cas,
        budget: &mut Budget,
    ) -> Result<PiWidthHorizon, LcRelationError> {
        use nrz_cas::enclosure::lc_future_ray::FutureRaySource;
        let proposal = self.receipt.proposal(cas, budget)?;
        let low = &proposal.vertices[proposal.low as usize].point;
        let high = &proposal.vertices[proposal.high as usize].point;
        if low[0].pi != high[0].pi || low[1].pi != high[1].pi {
            return Err(LcRelationError::Projection);
        }
        let width = [
            high[1].base.sub(&low[1].base, budget)?,
            high[0].base.sub(&low[0].base, budget)?,
        ];
        let authority = match &self.request.branches[0].source {
            FutureRaySource::Timing(original, _) => &original.pi,
            FutureRaySource::ClockPhase(original, _) => &original.pi,
            FutureRaySource::TimingClockPhase(original, _) => &original.pi,
        };
        Ok(PiWidthHorizon::check(
            width,
            self.request.reference.clone(),
            authority.check(budget)?,
            budget,
        )?)
    }
}
