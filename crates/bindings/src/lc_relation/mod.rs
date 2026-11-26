// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable autonomous LC constraints, exact projection families and checked feasibility.

mod amplitude;
pub use amplitude::{
    LcAmplitudeCover, LcAmplitudeParameters, LcAmplitudeSamples, LcAmplitudeWitness,
};
mod attempt;
pub use attempt::{LcBranchObligation, LcPairAttempt, LcUnresolvedBranches};
mod calibrated_elements;
pub use calibrated_elements::{
    LcCalibratedElements, LcCalibratedElementsCover, LcCalibratedElementsParameters,
    LcCalibratedElementsWitness, LcCalibratedForwardCover, LcCalibratedForwardWitness,
};
mod calibrated_future;
mod calibration;
pub use calibrated_future::LcCalibratedFuture;
mod capacitance;
mod clock;
mod clock_phase;
mod clock_refinement;
mod element;
pub use element::{LcElementCover, LcElementParameters, LcElementSamples, LcElementWitness};
mod energy;
mod energy_only;
pub use energy::{LcEnergyCover, LcEnergySamples, LcEnergyWitness};
pub use energy_only::{
    LcEnergyOnlyCalibrationCover, LcEnergyOnlyCover, LcEnergyOnlyParameters, LcEnergyOnlySamples,
    LcEnergyOnlyWitness,
};
mod forward;
pub use capacitance::{LcCapacitanceCover, LcCapacitanceParameters};
pub use clock::{
    LcClockCover, LcClockParameters, LcClockPhaseParameters, LcClockWitness, LcSharedClock,
};
pub use clock_phase::{
    LcClockPhaseCover, LcClockPhaseWitness, LcSharedClockPhase, LcSharedClockPhaseParameters,
};
pub use clock_refinement::{
    LcClockRefinement, LcClockRefinementInputs, LcClockRefinementState, LcClockRefinementSuccess,
};
mod horizon;
mod impossible;
mod measurement_impossible;
pub use measurement_impossible::LcMeasurementImpossible;
mod inclusion;
pub use horizon::{LcHorizon, LcHorizonCoordinate, LcHorizonValue};
pub use inclusion::LcInputRefinement;
mod interval;
mod joint_elements;
pub use impossible::LcImpossible;
pub use joint_elements::{
    LcJointElementsCover, LcJointElementsParameters, LcJointRootCover, LcJointRootParameters,
    LcRootRateEndpoint,
};
mod pairs;
mod phase;
pub use phase::{LcPhaseCover, LcPhaseParameters, LcPhaseWitness, LcSharedPhase};
mod quantity;
mod quiescent;
pub use quiescent::{LcQuiescentCover, LcQuiescentOutcome, LcQuiescentQuery};
mod time;
mod timing;
pub use shared::{
    LcSharedMeasurementCover, LcSharedMeasurementParameters, LcSharedMeasurementWitness,
    LcSharedMeasurements,
};
pub use time::{LcTailOutcome, LcTailStatus, LcTimeBox, LcTimeTail, LcZeroTimes};
pub use timing::{
    LcSampleTimes, LcSharedTiming, LcSharedTimingCover, LcSharedTimingWitness, LcTimingCover,
    LcTimingOuterCover,
};
mod shared;
mod storage;
mod witness;

pub use forward::{
    LcBoxForwardCover, LcCertainty, LcDeterminacy, LcForwardArc, LcForwardCover, LcPhaseEndpoint,
};
pub use interval::LcInterval;
pub use pairs::{
    LcAliasCover, LcAliasLattice, LcAliasWitness, LcConsistency, LcFreeDirection, LcPairParameters,
    LcPairRefinement, LcPairsCover, LcPhaseGauge, LcRefinementState, LcRefinementSuccess,
};
pub use quantity::LcQuantity;
pub use witness::{LcBranch, LcObservationValue, LcValues, LcWitness};

use crate::{Budget, Descriptor, ExactConstant};
use nrz_cas::{Location, limits::BudgetUsage};
use std::rc::Rc;

/// Every operation uses the caller's existing cumulative exact ledger.
const AT: Location = Location::new("autonomous LC relation");

/// Invalid declarations and unsuccessful witness attempts are not emptiness certificates.
#[derive(Debug, thiserror::Error)]
pub enum LcRelationError {
    /// Exact arithmetic or caller resource allowance refused.
    #[error("LC relation algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
    /// Physical descriptor construction refused.
    #[error("LC relation units: {0}")]
    Units(#[from] crate::UnitError),
    /// The supplied descriptor is not the quantity's coherent difference meaning.
    #[error("LC relation quantity meaning mismatch")]
    Meaning,
    /// A finite interval endpoint order is false or not proved by the closed checker.
    #[error("LC relation endpoint order is not proved")]
    UnprovedOrder,
    /// Every timestamp must refer to one explicit common epoch.
    #[error("LC relation requires a named common epoch")]
    Epoch,
    /// An observation index or witness vector does not match the declared inventory.
    #[error("LC relation observation inventory mismatch")]
    Observation,
    /// A candidate does not establish a required positive domain.
    #[error("LC relation witness positivity is not proved")]
    Positivity,
    /// An original equation has not been established exactly.
    #[error("LC relation witness does not establish an original equation")]
    Equation,
    /// A cycle proposal is not an exact integer.
    #[error("LC relation cycle is not an exact integer")]
    Cycle,
    /// A candidate does not establish membership in every supplied constraint.
    #[error("LC relation witness containment is not proved")]
    Containment,
    /// Phase and amplitude ratio have no physical value on the quiescent branch.
    #[error("LC relation quantity is undefined for the quiescent branch")]
    QuiescentPhase,
    /// Conservative metadata accounting exceeded its representable size.
    #[error("LC relation metadata capacity exhausted")]
    Capacity,
    /// This projection adapter does not cover the complete supplied constraint family.
    #[error("unsupported LC projection constraints")]
    Projection,
    /// A proposed cycle domain lacks a complete exact coverage or exclusion proof.
    #[error("LC complete cycle cover is not proved")]
    CycleCover,
}

/// One supplied deterministic constraint, retaining the original coherent meaning and range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcInput {
    /// The fixed physical quantity or declared observation position.
    quantity: LcQuantity,
    /// A canonical coherent descriptor checked against the caller's declaration.
    descriptor: Descriptor,
    /// Exact closed bounds; absent bounds remain mathematical unbounded endpoints.
    interval: LcInterval,
}

impl LcInput {
    /// Admit an exact value or deterministic interval with its physical kind.
    ///
    /// Standard uncertainty and covariance are not deterministic support. Callers must
    /// declare an actual interval; this constructor supplies no confidence multiplier.
    /// # Errors
    /// Refuses kind, role or coherent measure mismatches and exhausted caller resources.
    pub fn new(
        quantity: LcQuantity,
        descriptor: &Descriptor,
        interval: LcInterval,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        budget.work(1, AT)?;
        let expected = quantity.descriptor()?;
        let kind_matches = descriptor.kind() == expected.kind();
        let role_matches = descriptor.role() == expected.role();
        let unit_scale = descriptor
            .measure()
            .scale()
            .structurally_equal(&ExactConstant::one(), budget)?;
        let zero_offset = descriptor.measure().offset().is_zero();
        if !kind_matches || !role_matches || !unit_scale || !zero_offset {
            return Err(LcRelationError::Meaning);
        }
        storage::retained(interval.bytes().saturating_add(2_048), budget)?;
        Ok(Self {
            quantity,
            descriptor: expected,
            interval,
        })
    }

    /// The original supplied quantity, including its observation index.
    #[must_use]
    pub const fn quantity(&self) -> LcQuantity {
        self.quantity
    }

    /// The checked coherent physical meaning.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// The supplied support, without intersecting away its original declaration.
    #[must_use]
    pub const fn interval(&self) -> &LcInterval {
        &self.interval
    }
}

/// One simultaneous autonomous LC declaration, shared by all exact witness attempts.
///
/// The original laws are `a² L C = 1`, `2 E = C V² = L I²` and
/// `theta0 + a t = theta + 2 pi n`, with positive L, C and a. Positive
/// amplitudes imply `V/I = sqrt(L/C)`; zero energy has a separate domain.
/// This owner establishes neither a projected outer set nor structural determinacy.
#[derive(Clone, Debug)]
pub struct LcRelation(Rc<Data>);

/// Original constraints are never replaced by their marginal hulls or witness coordinates.
#[derive(Debug)]
struct Data {
    /// Caller-declared shared epoch; timestamps are duration differences from this epoch.
    epoch: String,
    /// Complete ordered phase/time inventory, including unsupplied coordinates.
    observations: usize,
    /// Every supplied constraint; repeated quantities retain their intersection semantics.
    inputs: Vec<LcInput>,
    /// Exact arithmetic and admission work snapshots on the caller ledger.
    usage: [BudgetUsage; 2],
    /// Conservative retained record, descriptor and exact-constant ownership.
    bytes: u64,
}

impl LcRelation {
    /// Declare the fixed simultaneous relation over any supplied subset of its quantities.
    ///
    /// Every timestamp is relative to `epoch`. Distinct clock uncertainties require a
    /// separate joint-constraint adapter; no shared clock model is inferred from boxes.
    /// Missing quantities retain their physical domains, without an implicit alias cutoff.
    /// # Errors
    /// Refuses an empty epoch, out-of-inventory indices or exhausted caller resources.
    pub fn new(
        epoch: String,
        observations: usize,
        inputs: Vec<LcInput>,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        let before = budget.usage();
        budget.work(1, AT)?;
        if epoch.is_empty() {
            return Err(LcRelationError::Epoch);
        }
        let mut bytes = storage::size(epoch.capacity())?.saturating_add(256);
        bytes = bytes.saturating_add(storage::size(inputs.capacity())?.saturating_mul(2_048));
        for input in &inputs {
            budget.work(1, AT)?;
            if input
                .quantity
                .observation()
                .is_some_and(|i| i >= observations)
            {
                return Err(LcRelationError::Observation);
            }
            bytes = bytes.saturating_add(input.interval.bytes());
        }
        storage::retained(bytes, budget)?;
        Ok(Self(Rc::new(Data {
            epoch,
            observations,
            inputs,
            usage: [before, budget.usage()],
            bytes,
        })))
    }

    /// The declared common epoch, kept with every witness.
    #[must_use]
    pub fn epoch(&self) -> &str {
        &self.0.epoch
    }

    /// Number of original phase/time positions.
    #[must_use]
    pub fn observations(&self) -> usize {
        self.0.observations
    }

    /// Complete original constraints, including all repeated supplied quantities.
    #[must_use]
    pub fn inputs(&self) -> &[LcInput] {
        &self.0.inputs
    }

    /// Exact identity of the immutable original declaration, not equality of centers.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Original relation admission snapshots; witness attempts have their own receipt.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }

    /// Conservative exact metadata ownership; no numerical heap is allocated here.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Only an exact zero or a checked positive difference establishes a non-strict order.
fn ordered(
    lower: &ExactConstant,
    upper: &ExactConstant,
    budget: &mut Budget,
) -> Result<bool, LcRelationError> {
    let difference = upper.sub(lower, budget)?;
    Ok(difference.is_zero() || difference.positive().is_some())
}
