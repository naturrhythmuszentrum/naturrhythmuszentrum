// SPDX-License-Identifier: AGPL-3.0-only
//! Exact count storage cannot be replaced by a floating quantity with equal displayed coordinates.
//!
//! An explicit ratio accepts two exact counts.
//! ```rust
//! use nrz_bindings::Budget;
//! use nrz_matrix::{CountError, CountQuantity, QuantityContext};
//! fn count_ratio(context: &QuantityContext, left: &CountQuantity, right: &CountQuantity, budget: &mut Budget) -> Result<(), CountError> {
//!     context.count_ratio(left, right, budget)?;
//!     Ok(())
//! }
//! let _ = count_ratio;
//! ```
//!
//! The same entry point refuses a continuous quantity in either count slot.
//! ```compile_fail,E0308
//! use nrz_bindings::Budget;
//! use nrz_matrix::{CountError, CountQuantity, Quantity, QuantityContext};
//! fn count_ratio(context: &QuantityContext, left: &CountQuantity, right: &Quantity<f64>, budget: &mut Budget) -> Result<(), CountError> {
//!     context.count_ratio(left, right, budget)?;
//!     Ok(())
//! }
//! let _ = count_ratio;
//! ```
//!
//! Exact count ratios expose their existing rational representation before a numerical format is chosen.
//! ```rust
//! use nrz_bindings::readings::algebra::Rational;
//! use nrz_matrix::ExactCountRatio;
//! fn exact(ratio: &ExactCountRatio) -> &Rational { ratio.exact() }
//! let _ = exact;
//! ```
//!
//! An exact count ratio cannot silently become a binary64 heap quantity.
//! ```compile_fail,E0308
//! use nrz_matrix::{ExactCountRatio, Quantity};
//! fn exact(ratio: &ExactCountRatio) -> &Quantity<f64> { ratio }
//! let _ = exact;
//! ```
