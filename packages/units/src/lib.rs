// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod descriptor;
mod dimension;
mod error;
mod kind;
mod measure;
mod registry;

pub use descriptor::{Descriptor, Role, uncertainty};
pub use dimension::{BaseDimension, Dimension, ExponentPolicy};
pub use error::UnitError;
pub use kind::{CountDomain, Kind, UncertaintyMode};
pub use measure::{Conversion, Measure, Scale};
pub use registry::{BinaryOp, Registry};
