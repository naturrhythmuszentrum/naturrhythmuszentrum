// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod checked;
mod checker;
mod declaration;
mod descriptor;
mod diagnostic;
mod dimension;
mod error;
mod kind;
mod measure;
mod registry;

pub use checked::{CheckedExpression, CompanionEvidence, NodeEvidence};
pub use declaration::{Companion, Context, Declaration, Evidence, FunctionSignature, Observable};
pub use descriptor::{Descriptor, Role, uncertainty};
pub use diagnostic::{Diagnostic, Expectation, Failure, Location};
pub use dimension::{BaseDimension, Dimension, ExponentPolicy};
pub use error::UnitError;
pub use kind::{CountDomain, Kind, UncertaintyMode};
pub use measure::{Conversion, Measure, Scale};
pub use registry::{BinaryOp, Registry};
