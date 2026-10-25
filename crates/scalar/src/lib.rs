// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod context;
mod conversion;
mod enclosure;
mod error;
mod finite;
mod format;
mod fused;
#[expect(
    clippy::redundant_pub_crate,
    reason = "literal tokens and work counters are shared only inside private decoder modules"
)]
mod literal;
mod pi;
#[expect(
    clippy::redundant_pub_crate,
    reason = "rounded primitives are shared only between private arithmetic modules"
)]
mod primitive;
mod reduction;

pub use context::ArithmeticContext;
pub use conversion::{PrecisionChange, narrow, widen};
pub use enclosure::Enclosure;
pub use error::ArithmeticFailure;
pub use finite::Finite;
pub use format::{Precision, Scalar};
pub use literal::{DecodedLiteral, LiteralLimits, NumericLiteral};
pub use pi::{PiBounds, PiBudget, PiEnclosure, PiError, PiLimits, PiUsage};
pub use reduction::{dot, norm_upper, sum};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/type_twins.rs"]
mod type_twins;
