// SPDX-License-Identifier: AGPL-3.0-only
//! Opaque factors and solve certificates cannot substitute for published values.
//!
//! A factor can enter the checked factored evaluator:
//! ```rust
//! use nrz_matrix::{Bound, FactorRef};
//! fn evaluate(bound: &Bound<'_, f64>, factor: &FactorRef<f64>) {
//!     if let Ok(inputs) = bound.normalized_inputs() {
//!         let _result = bound.evaluate_factored(&inputs, factor);
//!     }
//! }
//! ```
//!
//! An ordinary value handle cannot replace that opaque factor:
//! ```compile_fail
//! use nrz_matrix::{Bound, Handle, Value};
//! fn evaluate(bound: &Bound<'_, f64>, factor: &Handle<Value, f64>) {
//!     if let Ok(inputs) = bound.normalized_inputs() {
//!         let _result = bound.evaluate_factored(&inputs, factor);
//!     }
//! }
//! ```
//!
//! A solve proof can be inspected through its read-only methods:
//! ```rust
//! use nrz_matrix::SolveEvidence;
//! fn radius(proof: &SolveEvidence) -> f64 { proof.numerical_radius() }
//! ```
//!
//! The numerical claim cannot be overwritten:
//! ```compile_fail
//! use nrz_matrix::SolveEvidence;
//! fn radius(proof: &mut SolveEvidence) { proof.center = [0.0; 3]; }
//! ```
