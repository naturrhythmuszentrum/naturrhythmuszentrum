// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod error;
mod nonnegative;
mod vector;

pub use error::AccuracyFailure;
pub use nonnegative::Nonnegative;
pub use vector::{BoundVectorRef, NormKind, NormSummary};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/type_twins.rs"]
mod type_twins;
