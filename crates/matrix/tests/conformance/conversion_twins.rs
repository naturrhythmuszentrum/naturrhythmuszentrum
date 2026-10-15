// SPDX-License-Identifier: AGPL-3.0-only
//! Precision conversion keeps input and output scalar types independently sealed.
//!
//! A checked binary64-to-binary32 recipe binds binary64 inputs and publishes binary32 handles:
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, Heap, Value};
//! fn narrow(heap: &mut Heap, operation: CheckedOperation, input: Handle<Value, f64>)
//!     -> Result<Handle<Value, f32>, Box<dyn std::error::Error>>
//! {
//!     let bound = Binding::<f64, f32>::convert_precision(operation, vec![input], ExecutionLimits::default()).validate(heap)?;
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let output = bound.publish(candidate)?.into_output();
//!     Ok(output.first().ok_or("output")?.value())
//! }
//! ```
//!
//! Changing only the input handle's scalar type cannot satisfy that binding:
//! ```compile_fail
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, Heap, Value};
//! fn narrow(heap: &mut Heap, operation: CheckedOperation, input: Handle<Value, f32>)
//!     -> Result<Handle<Value, f32>, Box<dyn std::error::Error>>
//! {
//!     let bound = Binding::<f64, f32>::convert_precision(operation, vec![input], ExecutionLimits::default()).validate(heap)?;
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let output = bound.publish(candidate)?.into_output();
//!     Ok(output.first().ok_or("output")?.value())
//! }
//! ```
