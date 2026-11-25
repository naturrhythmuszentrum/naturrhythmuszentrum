// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "nrz-cas uses petgraph 0.8.3 with hashbrown 0.15 while indexmap uses hashbrown 0.17"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod admission;
mod checked;
mod checker;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod constants;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod coordinates;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private decision modules share bounded admission helpers"
)]
mod decision;
mod declaration;
mod descriptor;
mod diagnostic;
mod dimension;
mod dyadic_power;
mod engine;
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod evidence;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod exact;
mod kind;
mod lc_omega;
mod lc_relation;
pub use lc_omega::{
    LcOmegaCorrespondence, LcOmegaDeclarationRequest, LcOmegaDeclarationToken, LcOmegaOriginals,
    LcOmegaOriginalsRequest,
};
mod measure;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod original;
mod pairing;
pub mod readings;
mod registry;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod sensitivity;

pub use constants::{ExpressionEnclosure, ExpressionEnclosureError};
pub use dyadic_power::{DyadicPowerPreparation, DyadicPowerPreparationError};
pub use nrz_cas::{ExactConstant, PositiveConstant};

pub use nrz_cas::limits::{Budget, Limits, LimitsOverride};

pub use admission::{
    AdmissionError, BindingSymbols, CompanionKind, CopyWitness, HomogeneousRecipe, ParameterEntry,
    ParameterSymbol, ParameterTarget, QuantitySymbol, SymbolBinding, SymbolKind, SymbolSchema,
};

pub use checked::{CheckedExpression, CompanionEvidence, NodeEvidence};
pub use constants::{
    CheckedMap, ConstantEnclosureExport, ConstantKind, DescriptorConstant, ExactEnclosureExport,
    MapCoordinates, MapError, MapIdentity, MapKind, OriginDefinition, PiFactor,
    ReferenceDefinition,
};
pub use decision::{
    AxisPhaseAttempt, AxisPhaseAttempts, AxisPhaseCase, AxisPhaseImpedance, AxisPhaseOutcome,
    AxisPhaseSpec, CountDescriptor, CountEvidence, DecisionAttachment, DecisionCoordinate,
    DecisionError, DecisionExpression, DecisionInputs, DecisionOperand, IntegerDecisionCase,
    IntegerDecisionSpec, LatticeAttachment, LatticeAxis, LatticeCoordinate, LatticeDecisionCase,
    LatticeDecisionSpec, LatticeIndexKind, LatticeInputs, LatticeMetric, LatticePreparationUsage,
    LatticeSite, LcDrivenImpedance, SelectedDecision, SelectedLattice,
};
pub use declaration::{Companion, Context, Declaration, Evidence, FunctionSignature, Observable};
pub use descriptor::{Descriptor, Role, uncertainty};
pub use diagnostic::{Diagnostic, Expectation, Failure, Location};
pub use dimension::{BaseDimension, Dimension, ExponentPolicy};
pub use error::UnitError;
pub use evidence::{CheckedDeclarations, SnapshotError, SnapshotId};
pub use kind::{CountDomain, Kind, UncertaintyMode};
pub use lc_relation::{
    LcAliasCover, LcAliasLattice, LcAliasWitness, LcAmplitudeCover, LcAmplitudeParameters,
    LcAmplitudeSamples, LcAmplitudeWitness, LcBranch, LcBranchObligation, LcCalibratedElements,
    LcCalibratedElementsCover, LcCalibratedElementsParameters, LcCalibratedElementsWitness,
    LcCalibratedForwardCover, LcCalibratedForwardWitness, LcCalibratedFuture, LcCapacitanceCover,
    LcCapacitanceParameters, LcCertainty, LcClockCover, LcClockParameters, LcClockPhaseCover,
    LcClockPhaseParameters, LcClockPhaseWitness, LcClockRefinement, LcClockRefinementInputs,
    LcClockRefinementState, LcClockRefinementSuccess, LcClockWitness, LcConsistency, LcDeterminacy,
    LcElementCover, LcElementParameters, LcElementSamples, LcElementWitness, LcEnergyCover,
    LcEnergyOnlyCalibrationCover, LcEnergyOnlyCover, LcEnergyOnlyParameters, LcEnergyOnlySamples,
    LcEnergyOnlyWitness, LcEnergySamples, LcEnergyWitness, LcForwardArc, LcForwardCover,
    LcFreeDirection, LcHorizon, LcHorizonCoordinate, LcHorizonValue, LcImpossible, LcInput,
    LcInputRefinement, LcInterval, LcJointElementsCover, LcJointElementsParameters,
    LcJointRootCover, LcJointRootParameters, LcMeasurementImpossible, LcObservationValue,
    LcPairAttempt, LcPairParameters, LcPairRefinement, LcPairsCover, LcPhaseCover, LcPhaseEndpoint,
    LcPhaseGauge, LcPhaseParameters, LcPhaseWitness, LcQuantity, LcQuiescentCover,
    LcQuiescentOutcome, LcQuiescentQuery, LcRefinementState, LcRefinementSuccess, LcRelation,
    LcRelationError, LcRootRateEndpoint, LcSampleTimes, LcSharedClock, LcSharedClockPhase,
    LcSharedClockPhaseParameters, LcSharedMeasurementCover, LcSharedMeasurementParameters,
    LcSharedMeasurementWitness, LcSharedMeasurements, LcSharedPhase, LcSharedTiming,
    LcSharedTimingCover, LcSharedTimingWitness, LcTailOutcome, LcTailStatus, LcTimeBox, LcTimeTail,
    LcTimingCover, LcTimingOuterCover, LcUnresolvedBranches, LcValues, LcWitness, LcZeroTimes,
};
pub use measure::{Conversion, Measure, Scale};
pub use pairing::{
    EvaluationDescriptor, EvaluationKind, NamedReference, PairingError, PairingKind, PairingSide,
    PairingWitness,
};
pub use registry::{BinaryOp, FrequencyDirection, FrequencyRelation, Registry};
pub use sensitivity::{
    CheckedSensitivity, JacobianEntry, JacobianEvidence, SensitivityEntry, SensitivityError,
    SensitivityKernel, SensitivityObligation, SensitivityRecipe, SensitivityWork, SolveSymbols,
};

#[cfg(test)]
mod tests;
