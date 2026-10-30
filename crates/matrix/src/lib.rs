// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "faer and bindings carry duplicate transitive dependency versions"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "transaction evidence is shared across private matrix modules"
)]
mod binding;
#[expect(
    clippy::redundant_pub_crate,
    reason = "resource leases are shared across private execution modules"
)]
mod execution;
pub use execution::{ExecutionInventory, SharedFactorAllocation};

#[path = "coordinates/schema.rs"]
mod coordinate_schema;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible"
)]
mod heap;
#[expect(
    clippy::redundant_pub_crate,
    reason = "layout validation is shared across private heap and storage modules"
)]
mod layout;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shape contracts are shared across private matrix modules"
)]
mod shape;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the storage facade is shared only inside the crate"
)]
mod storage;

mod quantity;
pub use quantity::{
    DifferenceRole, DynamicRole, NormalizedQuantity, PointRole, Quantity, QuantityContext,
    QuantityError, QuantityInput, QuantityRole,
};

pub use binding::{
    AffineAdapter, CompanionAdapter, CompanionEvaluation, CompanionExecution,
    CompanionExecutionError, CompanionExecutionUsage, CoordinateContract, CoordinateError,
    CoordinateNorm, CoordinateReading, CopyDiscrepancy, DependencyPolicy, EntryTicket,
    LcSpectralEvaluation, LcSpectralPublication, LcSpectrum, LiveBindingError, MappedReading,
    OperatorAdapter, PhasorAllocation, PhasorAllocationChannel, PhasorAllocationStage, PhasorError,
    PhasorEvaluation, PhasorInventory, PhasorOperand, PhasorProduct, PhasorPublication,
    PhasorSolve, PhasorSolveCertificate, PhasorSolveEvaluation, PhasorSolvePublication,
    PhasorStatistics, PhasorTransferBounds, PhasorTransferredResidual, PiCoordinateContract,
    PublishedLcSpectrum, PublishedPhasor, PublishedPhasorSolve, ReadingAdapter,
    ReadingAdapterError, ReadingBinding, ReadingCoverage, ReadingImport, ReadingRegion,
    ReadingSelection, ReciprocalDerivativeContract, ReciprocalDerivativeError,
    ReciprocalDerivativeEvaluation, SpectralError, SpectralValue,
};
pub use binding::{
    Binding, Bound, CheckedOperation, CompanionSnapshot, ExecutionId, ExecutionLimits,
    InputSnapshot, InvariantCache, InvariantError, InvariantUse, InvariantValue, NormalizedInputs,
    NormalizedView, OperationDraft, OperationError, OutputReservation, PhysicalView,
    RegionContract, SymbolState, ValueBinding,
};

pub use layout::{Layout, PackedRegion};

pub use shape::{
    Axis, AxisAlignment, AxisEntry, BlockDraft, CheckedShape, CholeskyRootContract, EntryContract,
    EntryDraft, EntryMap, ExactCoefficient, ExpansionEntry, ExpansionMetric, ExpansionNorm,
    ExpansionRow, ExpansionTerm, Partition, QrShapeContract, ShapeDraft, ShapeError, ShapeLimits,
    ShapeRecipe, StructureScope, StructuredExpansion,
};

pub use coordinate_schema::{
    CoordinateMap, EntryNormalization, NormalizationSchema, OriginCorrection, PointOrigin,
    ReferenceScale,
};

pub use heap::{
    Category, Companion, CompanionView, Coverage, Extent, Factor, Handle, Heap, HeapError,
    HeapLimits, PublishedBundle, RegionView, SchemaId, Value, ValueId, ValueStamp, ValueVersion,
    ValueView, Workspace,
};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/quantity_twins.rs"]
mod quantity_twins;

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/type_twins.rs"]
mod type_twins;

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/shape_twins.rs"]
mod shape_twins;

#[cfg(test)]
mod tests;

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/binding_twins.rs"]
mod binding_twins;

#[cfg(test)]
use crate as test_api;

#[cfg(test)]
#[path = "testing/binding.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixtures are shared across private test modules"
)]
mod binding_testing;

#[expect(
    clippy::redundant_pub_crate,
    reason = "the heap owns private provenance storage"
)]
mod provenance;
#[expect(
    clippy::redundant_pub_crate,
    reason = "publication prepares private transaction records"
)]
mod publication;
#[expect(
    clippy::redundant_pub_crate,
    reason = "records are shared across private transaction modules"
)]
mod records;

pub use provenance::{ProvenanceId, ProvenanceLimits, ProvenanceUsage, ProvenanceView};
pub use publication::Candidate;
pub use records::{
    BundleTargetMap, CopyEvidence, Dependency, Failure, FailureRecord, OperationRecord, Outcome,
    OutputRecord, RecordId,
};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/publication_twins.rs"]
mod publication_twins;

#[expect(
    clippy::redundant_pub_crate,
    reason = "statistical records are owned by the private heap"
)]
mod companions;
#[expect(
    clippy::redundant_pub_crate,
    reason = "source declarations are shared across private companion modules"
)]
mod sources;

pub use companions::{CompanionRef, CompanionSetDraft, StatisticalView, TargetMap};
pub use sources::{
    CovarianceBlock, JointError, JointLimits, JointModel, ModelId, Source, SourceFactor, SourceId,
};

pub use sources::{CovarianceRepresentation, DenseCovariance, PsdCertificate};

pub use companions::{CovarianceParameterView, ParameterErrorView};

pub use companions::CrossLineage;

pub use companions::MeanParameters;

#[expect(
    clippy::redundant_pub_crate,
    reason = "conversion contracts and kernels are shared across private transaction modules"
)]
mod conversion;
#[path = "coordinates/numeric.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "lowered descriptor maps are shared by the numerical conversion recipes"
)]
mod coordinate_numeric;
pub use coordinate_numeric::ConstantCoefficient;
#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/joint_twins.rs"]
mod joint_twins;
pub use conversion::{
    CertifiedCoefficient, ConstantEvidence, ConversionContract, ConversionError,
    ConversionEvidence, ConversionMode, PiConstant, PiConstantEvidence, PiConversionEvidence,
    PiConversionUse, PrecisionContract, PrecisionEvidence, RecipeEvidence,
};

#[cfg(test)]
#[path = "testing/conversion_oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent numerical oracle is shared by conversion tests"
)]
mod conversion_oracle;
#[cfg(test)]
#[path = "testing/conversion.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "conversion fixtures are shared across private test modules"
)]
mod conversion_testing;

pub use conversion::{NormalizationMaps, NormalizedEntryMaps};
#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/conversion_twins.rs"]
mod conversion_twins;

#[expect(
    clippy::redundant_pub_crate,
    reason = "closed kernel contracts are shared by transaction modules"
)]
mod kernels;
pub use kernels::{
    DomainReason, Elementwise, FixedCoefficient, KernelError, KernelEvidence, KernelKind,
    LinearMap, ProductTerm,
};

#[cfg(test)]
#[path = "testing/kernel_oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent rational truth is shared across deterministic kernel tests"
)]
mod kernel_oracle;
#[cfg(test)]
#[path = "testing/kernels.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "kernel fixtures are shared across private test modules"
)]
mod kernel_testing;
#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/kernel_twins.rs"]
mod kernel_twins;

#[path = "verification/contractions.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent contraction evidence is consumed by deterministic kernels"
)]
mod contractions;

#[expect(
    clippy::redundant_pub_crate,
    reason = "solve contracts and numerical proofs are shared across private transaction modules"
)]
mod solve;
pub use solve::{
    FactorAllocation, FactorAllocationStage, FactorDependency, FactorInventory, FactorRef,
    FactorUse, RecoveredSolve, RecoveryFailure, SensitivityEvidence, SolveAllocation,
    SolveAllocationStage, SolveAttempt, SolveBudget, SolveError, SolveEvidence, SolveFailure,
    SolveFailureRecord, SolveInventory, SolvePolicy, SolveRecordKeys, SolveRecoveryEvidence,
    SolveStage, SolveStop,
};

#[path = "verification/solve.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent inverse and residual proofs are consumed only by solve evaluation"
)]
mod solve_verification;

#[cfg(test)]
#[path = "testing/solve.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "solve fixtures are shared across the corresponding test layers"
)]
mod solve_testing;

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/solve_twins.rs"]
mod solve_twins;
#[expect(
    clippy::redundant_pub_crate,
    reason = "statistical transaction evidence is shared across private matrix modules"
)]
mod statistics;
pub use statistics::{
    MomentAssurance, MomentRef, MomentView, ParameterView, StatisticsAllocation,
    StatisticsAllocationChannel, StatisticsAllocationRole, StatisticsError, StatisticsInventory,
};

#[cfg(test)]
#[path = "testing/solve_statistics.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "solve source fixtures are shared across test modules"
)]
mod solve_statistics_testing;

#[cfg(test)]
#[path = "testing/recovery.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "bounded solve policy fixtures are shared across test modules"
)]
mod recovery_testing;

#[cfg(test)]
#[path = "testing/readings.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact reading fixtures are shared across the corresponding test layers"
)]
mod reading_testing;

#[cfg(test)]
#[path = "testing/copies.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact reading fixtures are shared across the corresponding test layers"
)]
mod copy_testing;

#[cfg(test)]
#[path = "testing/phasor.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "phasor source and publication fixtures are shared across test layers"
)]
mod phasor_testing;

#[cfg(test)]
#[path = "testing/phasor_solve.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "physical phasor solve fixtures serve public and private tests"
)]
mod phasor_solve_testing;

#[expect(
    clippy::redundant_pub_crate,
    reason = "private allocation authority is shared by numerical kernels and publication"
)]
mod private;
pub use private::{
    PrivateAllocation, PrivateAllocationError, PrivateAllocationTicket, ReciprocalInventory,
};

#[cfg(test)]
#[path = "testing/reciprocal.rs"]
#[allow(
    dead_code,
    reason = "unit tests use the private-allocation subset of shared public fixtures"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "reciprocal fixtures are shared across test layers"
)]
mod reciprocal_testing;

pub use publication::{
    DecisionInputSnapshot, DecisionSnapshotError, FrozenDecisionInputs, FrozenLatticeEnclosure,
    FrozenLatticeInputs,
};
pub use publication::{
    LiveLatticeIndex, LiveLatticeRepresentative, PreparedLatticeDecision, PublishedLatticeDecision,
};

pub use publication::{
    FrozenRatioKernel, LiveDecisionCount, LiveDecisionError, PreparedDecision, PublishedDecision,
    RatioKernelEvidence,
};
