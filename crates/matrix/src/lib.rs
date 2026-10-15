// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "faer and units carry duplicate transitive dependency versions"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "transaction evidence is shared across private matrix modules"
)]
mod binding;

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

pub use binding::{
    Binding, Bound, CheckedOperation, CompanionSnapshot, CopyContract, ExecutionId,
    ExecutionLimits, InputSnapshot, NormalizedInputs, NormalizedView, OperationDraft,
    OperationError, OutputReservation, PhysicalView,
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
    CopyEvidence, Dependency, Failure, FailureRecord, OperationRecord, Outcome, OutputRecord,
    RecordId,
};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/publication_twins.rs"]
mod publication_twins;
