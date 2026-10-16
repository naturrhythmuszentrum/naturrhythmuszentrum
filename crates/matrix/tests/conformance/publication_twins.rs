// SPDX-License-Identifier: AGPL-3.0-only
//! Public normalized-execution and consuming-publication type twins.
//!
//! Evaluation accepts only the binding-stamped normalized input bundle.
//!
//! ```rust
//! use nrz_matrix::{Bound, PhysicalView};
//! fn evaluate(bound: Bound<'_, f64>, physical: PhysicalView<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = physical;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = bound.evaluate(inputs)?;
//!     Ok(())
//! }
//! let _ = evaluate;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Bound, PhysicalView};
//! fn evaluate(bound: Bound<'_, f64>, physical: PhysicalView<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = physical;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = bound.evaluate(physical)?;
//!     Ok(())
//! }
//! let _ = evaluate;
//! ```
//!
//! A published value can enter the next binding; its private candidate cannot.
//!
//! ```rust
//! use nrz_matrix::{Binding, Candidate, CheckedOperation, ExecutionLimits, PublishedBundle};
//! fn next(operation: CheckedOperation, published: PublishedBundle<f64>, candidate: Candidate<f64>) {
//!     let _ = candidate;
//!     let _ = Binding::new(operation, vec![published.value()], ExecutionLimits::default());
//! }
//! let _ = next;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, Candidate, CheckedOperation, ExecutionLimits, PublishedBundle};
//! fn next(operation: CheckedOperation, published: PublishedBundle<f64>, candidate: Candidate<f64>) {
//!     let _ = candidate;
//!     let _ = Binding::new(operation, vec![candidate], ExecutionLimits::default());
//! }
//! let _ = next;
//! ```
//!
//! Private candidate evidence cannot be forged by a struct literal.
//!
//! ```rust
//! use nrz_matrix::Candidate;
//! fn inspect(candidate: Candidate<f64>) {
//!     let _ = candidate;
//! }
//! let _ = inspect;
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_matrix::Candidate;
//! fn inspect(candidate: Candidate<f64>) {
//!     let _ = Candidate::<f64> { execution: candidate.execution(), token: 0, revision: 1, outputs: Vec::new(), evidence: Vec::new(), statistics: Vec::new() };
//! }
//! let _ = inspect;
//! ```
//!
//! Publication consumes the binding only after its last normalized view is released.
//!
//! ```rust
//! use nrz_matrix::Bound;
//! fn publish(bound: Bound<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let views = bound.normalized_inputs()?;
//!     drop(views);
//!     let _ = bound.publish(candidate)?;
//!     Ok(())
//! }
//! let _ = publish;
//! ```
//!
//! ```compile_fail,E0505
//! use nrz_matrix::Bound;
//! fn publish(bound: Bound<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let views = bound.normalized_inputs()?;
//!     let _ = bound.publish(candidate)?;
//!     drop(views);
//!     Ok(())
//! }
//! let _ = publish;
//! ```
//!
//! The execution and its candidate token are consumed exactly once.
//!
//! ```rust
//! use nrz_matrix::Bound;
//! fn publish(bound: Bound<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let outcome = bound.publish(candidate)?;
//!     let _ = outcome;
//!     Ok(())
//! }
//! let _ = publish;
//! ```
//!
//! ```compile_fail,E0382
//! use nrz_matrix::Bound;
//! fn publish(bound: Bound<'_, f64>) -> Result<(), Box<dyn std::error::Error>> {
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let outcome = bound.publish(candidate)?;
//!     let _ = bound.publish(candidate)?;
//!     Ok(())
//! }
//! let _ = publish;
//! ```
//!
//! Retirement's raw ownership snapshot cannot enter a public evidence path.
//!
//! ```rust
//! use nrz_matrix::{Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>) -> Result<(), HeapError> {
//!     let _ = heap.value(value)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0624
//! use nrz_matrix::{Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>) -> Result<(), HeapError> {
//!     let _ = heap.retirement_snapshot(value)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
