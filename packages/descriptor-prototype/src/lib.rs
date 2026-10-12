// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod error;
mod execution;
mod fixture;
mod heap;
mod maps;
mod plan;
mod schema;

pub use error::Error;
pub use heap::{
    Companion, FactorResource, Handle, Heap, ReferenceInfo, Scalar, Target, Value, Workspace,
};
pub use schema::{Axis, QuantityId, QuantityKey, SchemaPool, Shape, ShapeId};

pub use execution::{Bound, Candidate, NormalizedView, OutputContract, Published, solve};
pub use fixture::{Fixture, ResistorQuantities, resistor_fixture};
pub use maps::{CoordinateMap, Difference, PhysicalView, Point, difference_view, point_view};
pub use plan::{Binding, Checked, Coverage, Inputs, Layout, Maps, Plan, Slot, Unchecked};

pub mod static_probe;
