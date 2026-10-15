// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "faer and units carry duplicate transitive dependency versions"
)]

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

pub use layout::{Layout, PackedRegion};

pub use shape::{
    Axis, AxisAlignment, AxisEntry, BlockDraft, CheckedShape, CholeskyRootContract, EntryContract,
    EntryDraft, EntryMap, Partition, QrShapeContract, ShapeDraft, ShapeError, ShapeLimits,
    ShapeRecipe, StructureScope,
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
