// SPDX-License-Identifier: AGPL-3.0-only
//! Calibrated future questions retain their original joint fibres and full original extremizers.

use super::{AT, LcRelationError, storage};
use crate::{
    Budget, LcClockPhaseCover, LcClockPhaseWitness, LcHorizon, LcRelation,
    LcSharedMeasurementCover, LcSharedMeasurementWitness, LcWitness,
};
use nrz_cas::{Rational, lattice::PiWidthHorizon};
use std::{cmp::Ordering, rc::Rc};

/// Original calibrated source and checked physical extremizers for a reported-clock horizon.
#[derive(Clone, Debug)]
pub struct LcCalibratedFuture(Rc<Data>);

/// Exact elimination retains the complete original calibration fibres beside its auxiliary horizon.
#[derive(Debug)]
struct Data {
    /// Complete original phase or phase/amplitude source, including checked extremizers.
    original: Original,
    /// Complete auxiliary clock projection before any shared phase-offset restoration.
    auxiliary: LcHorizon,
    /// Checked physical reported-intercept/rate extremizers after restoring the shared phase fibre.
    extrema: [[Rational; 2]; 2],
    /// Exact physical future width and its finite/infinite/undecided threshold.
    mathematical: PiWidthHorizon,
    /// Conservative total retained ownership.
    bytes: u64,
}

/// Both variants retain their own original measurement question rather than a marginal substitute.
#[derive(Debug)]
enum Original {
    /// Shared clock and phase offsets with the complete original physical phase domain.
    Phase {
        /// Complete original phase-calibrated family.
        cover: LcClockPhaseCover,
        /// Lower and upper full-original physical extremizers.
        witnesses: Vec<LcClockPhaseWitness>,
    },
    /// The same joint clock/phase domain with original amplitude readings and one shared gain.
    Amplitude {
        /// Complete original amplitude, phase and clock family.
        cover: LcSharedMeasurementCover,
        /// Lower and upper physical extremizers with every original calibration.
        witnesses: Vec<LcSharedMeasurementWitness>,
    },
}

impl LcCalibratedFuture {
    /// Retain the original phase calibration and recheck correspondence of both physical extremizers.
    pub(super) fn phase(
        cover: LcClockPhaseCover,
        witnesses: Vec<LcClockPhaseWitness>,
        extrema: [[Rational; 2]; 2],
        auxiliary: LcHorizon,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        if witnesses.len() != 2 {
            return Err(LcRelationError::Observation);
        }
        let mut bytes = cover.retained_payload_bytes();
        for (point, expected) in witnesses.iter().zip(&extrema) {
            budget.work(1, AT)?;
            if !point.phase().shares_allocation(cover.phase()) {
                return Err(LcRelationError::Containment);
            }
            extremizer(
                point.original().original(),
                point.original().offset(),
                expected,
                budget,
            )?;
            bytes = bytes
                .saturating_add(point.original().original().retained_payload_bytes())
                .saturating_add(storage::rational(point.offset()))
                .saturating_add(storage::rational(point.original().offset()));
        }
        Self::finish(
            Original::Phase { cover, witnesses },
            auxiliary,
            extrema,
            bytes,
            budget,
        )
    }

    /// Retain every amplitude reading and gain beside the original physical future extremizers.
    pub(super) fn amplitude(
        cover: LcSharedMeasurementCover,
        witnesses: Vec<LcSharedMeasurementWitness>,
        auxiliary: LcHorizon,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        if witnesses.len() != 2 {
            return Err(LcRelationError::Observation);
        }
        let mut bytes = cover.retained_payload_bytes();
        for (point, expected) in witnesses.iter().zip(auxiliary.extremal_parameters()) {
            budget.work(1, AT)?;
            if !point.measurements().shares_allocation(cover.measurements()) {
                return Err(LcRelationError::Containment);
            }
            extremizer(
                point.original().original().original(),
                point.original().original().offset(),
                expected,
                budget,
            )?;
            bytes = bytes
                .saturating_add(
                    point
                        .original()
                        .original()
                        .original()
                        .retained_payload_bytes(),
                )
                .saturating_add(storage::rational(point.gain()))
                .saturating_add(storage::rational(point.original().offset()))
                .saturating_add(storage::rational(point.original().original().offset()));
        }
        let extrema = auxiliary.extremal_parameters().clone();
        Self::finish(
            Original::Amplitude { cover, witnesses },
            auxiliary,
            extrema,
            bytes,
            budget,
        )
    }

    /// The equivalent auxiliary proof is retained privately with every original source and witness.
    fn finish(
        original: Original,
        auxiliary: LcHorizon,
        extrema: [[Rational; 2]; 2],
        bytes: u64,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        let mut identical = true;
        for (physical, effective) in extrema
            .iter()
            .flatten()
            .zip(auxiliary.extremal_parameters().iter().flatten())
        {
            identical &= physical.compare(effective, budget)? == Ordering::Equal;
        }
        let mathematical = if identical {
            auxiliary.mathematical().clone()
        } else {
            let width = [
                extrema[1][0].sub(&extrema[0][0], budget)?,
                extrema[1][1].sub(&extrema[0][1], budget)?,
            ];
            PiWidthHorizon::check(
                width,
                auxiliary.mathematical().reference().clone(),
                auxiliary.mathematical().pi_bounds().clone(),
                budget,
            )?
        };
        let bytes = extrema
            .iter()
            .flatten()
            .fold(bytes, |sum, value| {
                sum.saturating_add(storage::rational(value))
            })
            .saturating_add(mathematical.retained_payload_bytes());
        let bytes = bytes
            .saturating_add(auxiliary.retained_payload_bytes())
            .saturating_add(auxiliary.source_payload_bytes())
            .saturating_add(2_048);
        storage::retained(bytes, budget)?;
        Ok(Self(Rc::new(Data {
            original,
            auxiliary,
            extrema,
            mathematical,
            bytes,
        })))
    }

    /// Complete original physical relation and every original input.
    #[must_use]
    pub fn relation(&self) -> &LcRelation {
        match &self.0.original {
            Original::Phase { cover, .. } => cover.relation(),
            Original::Amplitude { cover, .. } => cover.relation(),
        }
    }

    /// Original clock/phase cover, when no amplitude calibration is attached.
    #[must_use]
    pub fn clock_phase_cover(&self) -> Option<&LcClockPhaseCover> {
        match &self.0.original {
            Original::Phase { cover, .. } => Some(cover),
            Original::Amplitude { .. } => None,
        }
    }

    /// Original clock/phase/amplitude cover, with all calibration fibres retained.
    #[must_use]
    pub fn shared_measurement_cover(&self) -> Option<&LcSharedMeasurementCover> {
        match &self.0.original {
            Original::Amplitude { cover, .. } => Some(cover),
            Original::Phase { .. } => None,
        }
    }

    /// Full original phase witnesses for the lower and upper future extremizers, in that order.
    #[must_use]
    pub fn phase_witnesses(&self) -> Option<&[LcClockPhaseWitness]> {
        match &self.0.original {
            Original::Phase { witnesses, .. } => Some(witnesses),
            Original::Amplitude { .. } => None,
        }
    }

    /// Full original amplitude/phase witnesses for both future extremizers, in endpoint order.
    #[must_use]
    pub fn amplitude_witnesses(&self) -> Option<&[LcSharedMeasurementWitness]> {
        match &self.0.original {
            Original::Amplitude { witnesses, .. } => Some(witnesses),
            Original::Phase { .. } => None,
        }
    }

    /// Conservative complete original and auxiliary proof ownership.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Exact physical image after restoring any original shared phase offset.
    pub(super) fn extrema(&self) -> &[[Rational; 2]; 2] {
        &self.0.extrema
    }

    /// Checked threshold for the physical image, reusing unchanged prepared evidence.
    pub(super) fn mathematical(&self) -> &PiWidthHorizon {
        &self.0.mathematical
    }

    /// Only the retaining physical horizon constructor can reuse the equivalent auxiliary proof.
    pub(super) fn auxiliary(&self) -> &LcHorizon {
        &self.0.auxiliary
    }
}

/// The original full witness attains the exact reported intercept and rate used in the horizon.
fn extremizer(
    point: &LcWitness,
    clock: &Rational,
    expected: &[Rational; 2],
    budget: &mut Budget,
) -> Result<(), LcRelationError> {
    if matches_extremizer(point, clock, expected, budget)? {
        Ok(())
    } else {
        Err(LcRelationError::Containment)
    }
}

/// A sealed immutable original tuple can be reused after an exact endpoint identity check.
/// This is a lookup of existing physical evidence, never a new witness inferred from marginals.
pub(super) fn matches_extremizer(
    point: &LcWitness,
    clock: &Rational,
    expected: &[Rational; 2],
    budget: &mut Budget,
) -> Result<bool, LcRelationError> {
    budget.work(1, AT)?;
    let values = point.values();
    let Some(rate) = values.rate.as_rational() else {
        return Ok(false);
    };
    let Some(alpha) = values
        .initial_phase
        .as_ref()
        .and_then(crate::ExactConstant::as_rational)
    else {
        return Ok(false);
    };
    let intercept = alpha.add(&rate.mul(clock, budget)?, budget)?;
    let same_rate = rate.compare(&expected[1], budget)? == Ordering::Equal;
    let same_intercept = intercept.compare(&expected[0], budget)? == Ordering::Equal;
    Ok(same_rate && same_intercept)
}

/// Complete bound clock-phase question and its checked exact future-ray projection.
#[derive(Debug)]
pub struct LcAffineFutureRay {
    /// Complete original binding owner for the declared coordinate.
    original: AffineOriginal,
    /// Complete original CAS inventory and future coordinate scope.
    pub(super) request: nrz_cas::enclosure::lc_future_ray::FutureRayRequest,
    /// Checked store-scoped future-ray theorem.
    pub(super) receipt: nrz_cas::enclosure::lc_future_ray::FutureRayReceipt,
}

/// The original timing and phase owners whose declarations produced the CAS source.
#[derive(Clone, Debug)]
enum AffineOriginal {
    /// Independent timestamp intervals and physical phase strips.
    Timing(crate::LcSampleTimes),
    /// Common clock and phase offset with measured and physical strips.
    ClockPhase(crate::LcSharedClockPhase),
    /// Common offsets and independent timestamp errors with both phase layers.
    TimingClockPhase(crate::LcSharedClockPhase, crate::LcSharedTiming),
}

impl crate::LcSharedClockPhase {
    /// Construct a plain or narrow future ray from the same complete original declarations.
    /// Each branch supplies its selected original turns; `shifts` gives full-turn maps from
    /// the first branch. The CAS checker requires complete gauge-equivalent coverage.
    /// # Errors
    /// Refuses incomplete branches, unsupported originals or exhausted resources.
    #[expect(
        clippy::too_many_arguments,
        reason = "complete original branch and reference authority"
    )]
    pub fn affine_future_ray(
        &self,
        cas: &mut nrz_cas::Cas,
        form: nrz_cas::enclosure::lc_future_ray::FutureRayForm,
        lifts: &[Vec<i64>],
        domains: &[[i64; 2]],
        shifts: &[i64],
        versions: &[u64],
        pi: &nrz_cas::pi_bounds::PiBoundCertificate,
        reference: Rational,
        safe_reference: nrz_cas::enclosure::lc_affine_fibre::PiAffine,
        narrowed: bool,
        budget: &mut Budget,
    ) -> Result<LcAffineFutureRay, LcRelationError> {
        use nrz_cas::enclosure::lc_future_ray::{
            FutureRayBranch, FutureRayRequest, FutureRaySource,
        };
        if lifts.is_empty()
            || lifts.len() != shifts.len()
            || lifts.iter().any(|v| v.len() != domains.len())
        {
            return Err(LcRelationError::Observation);
        }
        budget.work(lifts.len() as u64, AT)?;
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(lifts.len())
            .map_err(|_| LcRelationError::Capacity)?;
        for (id, (branch_lifts, shift)) in lifts.iter().zip(shifts).enumerate() {
            let original = self.affine_clock_phase_request(
                branch_lifts,
                domains,
                versions,
                (*pi).clone(),
                budget,
            )?;
            let certificate = cas
                .construct_lc_affine_clock_phase(&original, budget)?
                .certificate(cas, budget)?;
            branches.push(FutureRayBranch {
                id: u64::try_from(id).map_err(|_| LcRelationError::Capacity)?,
                shift: *shift,
                source: FutureRaySource::ClockPhase(original, certificate),
            });
        }
        let request = FutureRayRequest {
            form,
            reference,
            safe_reference,
            narrowed,
            epoch: self.relation().epoch().to_owned(),
            branches,
        };
        let receipt = cas.construct_lc_future_ray(&request, budget)?;
        Ok(LcAffineFutureRay {
            original: AffineOriginal::ClockPhase(self.clone()),
            request,
            receipt,
        })
    }
}

impl LcAffineFutureRay {
    /// The identical original physical relation.
    #[must_use]
    pub fn original(&self) -> &LcRelation {
        match &self.original {
            AffineOriginal::Timing(source) => source.relation(),
            AffineOriginal::ClockPhase(source) | AffineOriginal::TimingClockPhase(source, _) => {
                source.relation()
            }
        }
    }
    /// The retained independent timing owner when the source has timestamp errors.
    #[must_use]
    pub const fn timing_owner(&self) -> Option<&crate::LcSharedTiming> {
        match &self.original {
            AffineOriginal::TimingClockPhase(_, owner) => Some(owner),
            _ => None,
        }
    }
    /// Recheck and borrow the exact projected polygon and whole-ray inequalities.
    /// # Errors
    /// Refuses foreign stores or invalidated evidence.
    pub fn proposal<'a>(
        &'a self,
        cas: &nrz_cas::Cas,
        budget: &mut Budget,
    ) -> Result<&'a nrz_cas::enclosure::lc_future_ray::FutureRayProposal, LcRelationError> {
        Ok(self.receipt.proposal(cas, budget)?)
    }
    /// Export the checked portable ray proposal while retaining original certificates.
    /// # Errors
    /// Refuses foreign stores or exhausted resources.
    pub fn certificate(
        &self,
        cas: &nrz_cas::Cas,
        budget: &mut Budget,
    ) -> Result<nrz_cas::enclosure::lc_future_ray::FutureRayCertificate, LcRelationError> {
        Ok(self.receipt.certificate(cas, budget)?)
    }
    /// Restore the complete original physical tuple at a rational interpolation weight.
    /// # Errors
    /// Refuses weights outside the polygon or failed original restoration.
    pub fn interpolate(
        &self,
        cas: &nrz_cas::Cas,
        weight: &Rational,
        budget: &mut Budget,
    ) -> Result<nrz_cas::enclosure::lc_future_ray::FutureRayWitness, LcRelationError> {
        Ok(self.receipt.interpolate(cas, weight, budget)?)
    }
}

impl crate::LcSampleTimes {
    /// Construct an actual-time ray from complete independent timing originals.
    /// # Errors
    /// Refuses incomplete turns, non-gauge branches and exhausted resources.
    #[expect(
        clippy::too_many_arguments,
        reason = "complete original branch and reference authority"
    )]
    pub fn affine_future_ray(
        &self,
        cas: &mut nrz_cas::Cas,
        lifts: &[Vec<i64>],
        domains: &[[i64; 2]],
        shifts: &[i64],
        versions: &[u64],
        pi: &nrz_cas::pi_bounds::PiBoundCertificate,
        reference: Rational,
        safe_reference: nrz_cas::enclosure::lc_affine_fibre::PiAffine,
        narrowed: bool,
        budget: &mut Budget,
    ) -> Result<LcAffineFutureRay, LcRelationError> {
        use nrz_cas::enclosure::lc_future_ray::{
            FutureRayBranch, FutureRayForm, FutureRayRequest, FutureRaySource,
        };
        if lifts.is_empty()
            || lifts.len() != shifts.len()
            || lifts.iter().any(|v| v.len() != domains.len())
        {
            return Err(LcRelationError::Observation);
        }
        budget.work(lifts.len() as u64, AT)?;
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(lifts.len())
            .map_err(|_| LcRelationError::Capacity)?;
        for (id, (branch_lifts, shift)) in lifts.iter().zip(shifts).enumerate() {
            let original =
                self.affine_timing_request(branch_lifts, domains, versions, (*pi).clone(), budget)?;
            let certificate = cas
                .construct_lc_affine_timing(&original, budget)?
                .certificate(cas, budget)?;
            branches.push(FutureRayBranch {
                id: u64::try_from(id).map_err(|_| LcRelationError::Capacity)?,
                shift: *shift,
                source: FutureRaySource::Timing(original, certificate),
            });
        }
        let request = FutureRayRequest {
            form: FutureRayForm::Plain,
            reference,
            safe_reference,
            narrowed,
            epoch: self.relation().epoch().to_owned(),
            branches,
        };
        let receipt = cas.construct_lc_future_ray(&request, budget)?;
        Ok(LcAffineFutureRay {
            original: AffineOriginal::Timing(self.clone()),
            request,
            receipt,
        })
    }
}

impl crate::LcSharedTiming {
    /// Construct a plain or narrow ray retaining every common and independent timing layer.
    /// # Errors
    /// Refuses different original owners, incomplete turns and exhausted resources.
    #[expect(
        clippy::too_many_arguments,
        reason = "complete joint branch and reference authority"
    )]
    pub fn affine_future_ray_with_phase(
        &self,
        phase: &crate::LcSharedClockPhase,
        cas: &mut nrz_cas::Cas,
        form: nrz_cas::enclosure::lc_future_ray::FutureRayForm,
        lifts: &[Vec<i64>],
        domains: &[[i64; 2]],
        shifts: &[i64],
        versions: &[u64],
        pi: &nrz_cas::pi_bounds::PiBoundCertificate,
        reference: Rational,
        safe_reference: nrz_cas::enclosure::lc_affine_fibre::PiAffine,
        narrowed: bool,
        budget: &mut Budget,
    ) -> Result<LcAffineFutureRay, LcRelationError> {
        use nrz_cas::enclosure::lc_future_ray::{
            FutureRayBranch, FutureRayRequest, FutureRaySource,
        };
        if lifts.is_empty()
            || lifts.len() != shifts.len()
            || lifts.iter().any(|v| v.len() != domains.len())
        {
            return Err(LcRelationError::Observation);
        }
        budget.work(lifts.len() as u64, AT)?;
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(lifts.len())
            .map_err(|_| LcRelationError::Capacity)?;
        for (id, (branch_lifts, shift)) in lifts.iter().zip(shifts).enumerate() {
            let original = self.affine_timing_clock_phase_request(
                phase,
                branch_lifts,
                domains,
                versions,
                (*pi).clone(),
                budget,
            )?;
            let certificate = cas
                .construct_lc_affine_timing_clock_phase(&original, budget)?
                .certificate(cas, budget)?;
            branches.push(FutureRayBranch {
                id: u64::try_from(id).map_err(|_| LcRelationError::Capacity)?,
                shift: *shift,
                source: FutureRaySource::TimingClockPhase(original, certificate),
            });
        }
        let request = FutureRayRequest {
            form,
            reference,
            safe_reference,
            narrowed,
            epoch: self.relation().epoch().to_owned(),
            branches,
        };
        let receipt = cas.construct_lc_future_ray(&request, budget)?;
        Ok(LcAffineFutureRay {
            original: AffineOriginal::TimingClockPhase(phase.clone(), self.clone()),
            request,
            receipt,
        })
    }
}
