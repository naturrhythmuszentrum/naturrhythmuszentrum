// SPDX-License-Identifier: AGPL-3.0-only
//! Checked quantity markers and scalar formats enforce single-boundary compile twins.
//!
//! A checked point accepts a difference translation and refuses another point in that slot.
//!
//! ```rust
//! use nrz_bindings::Budget;
//! use nrz_matrix::{DifferenceRole, Heap, PointRole, Quantity, QuantityContext, QuantityError};
//! fn quantity_fixed_role_twins(context: &QuantityContext, heap: &mut Heap, point: &Quantity<f64, PointRole>, delta: &Quantity<f64, DifferenceRole>, budget: &mut Budget) -> Result<(), QuantityError<f64>> {
//!     context.translate(heap, point, delta, budget)?;
//!     Ok(())
//! }
//! let _ = quantity_fixed_role_twins;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_bindings::Budget;
//! use nrz_matrix::{Heap, PointRole, Quantity, QuantityContext, QuantityError};
//! fn quantity_fixed_role_twins(context: &QuantityContext, heap: &mut Heap, point: &Quantity<f64, PointRole>, delta: &Quantity<f64, PointRole>, budget: &mut Budget) -> Result<(), QuantityError<f64>> {
//!     context.translate(heap, point, delta, budget)?;
//!     Ok(())
//! }
//! let _ = quantity_fixed_role_twins;
//! ```
//!
//! Widening accepts binary32 quantities and refuses binary64 inputs at the same typed entry point.
//!
//! ```rust
//! use nrz_bindings::Budget;
//! use nrz_matrix::{Heap, Quantity, QuantityContext, QuantityError};
//! fn quantity_precision_twins(context: &QuantityContext, heap: &mut Heap, input: &Quantity<f32>, budget: &mut Budget) -> Result<(), QuantityError<f32>> {
//!     let _: Quantity<f64> = context.widen(heap, input, budget)?;
//!     Ok(())
//! }
//! let _ = quantity_precision_twins;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_bindings::Budget;
//! use nrz_matrix::{Heap, Quantity, QuantityContext, QuantityError};
//! fn quantity_precision_twins(context: &QuantityContext, heap: &mut Heap, input: &Quantity<f64>, budget: &mut Budget) -> Result<(), QuantityError<f32>> {
//!     let _: Quantity<f64> = context.widen(heap, input, budget)?;
//!     Ok(())
//! }
//! let _ = quantity_precision_twins;
//! ```
