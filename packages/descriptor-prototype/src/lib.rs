// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod error;
mod heap;
mod schema;

pub use error::Error;
pub use heap::{
    Companion, FactorResource, Handle, Heap, ReferenceInfo, Scalar, Target, Value, Workspace,
};
pub use schema::{Axis, QuantityId, QuantityKey, SchemaPool, Shape, ShapeId};
