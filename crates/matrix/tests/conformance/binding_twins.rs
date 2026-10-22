// SPDX-License-Identifier: AGPL-3.0-only
//! Public transaction type-state twins use a single changed boundary.
//!
//! Only a checked native operation can construct a binding.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, OperationDraft, Value};
//! fn bind(checked: CheckedOperation, draft: OperationDraft, input: Handle<Value, f64>) {
//!     let _ = draft;
//!     let _ = Binding::new(checked, vec![input], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, OperationDraft, Value};
//! fn bind(checked: CheckedOperation, draft: OperationDraft, input: Handle<Value, f64>) {
//!     let _ = draft;
//!     let _ = Binding::new(draft, vec![input], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! Only validation grants normalized view authority.
//!
//! ```rust
//! use nrz_matrix::{Binding, Heap, OperationError};
//! fn read(binding: Binding<f64>, heap: &mut Heap) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let _ = bound.normalized_inputs()?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0599
//! use nrz_matrix::{Binding, Heap, OperationError};
//! fn read(binding: Binding<f64>, heap: &mut Heap) -> Result<(), OperationError> {
//!     let bound = binding;
//!     let _ = bound.normalized_inputs()?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! A Companion cannot occupy a value input slot.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, Companion, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Companion, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![value], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, CheckedOperation, Companion, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Companion, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![other], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! A Factor cannot occupy a value input slot.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, Factor, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Factor, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![value], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, CheckedOperation, Factor, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Factor, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![other], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! A Workspace cannot occupy a value input slot.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, Workspace, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Workspace, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![value], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, CheckedOperation, Workspace, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, value: Handle<Value, f64>, other: Handle<Workspace, f64>) {
//!     let _ = other;
//!     let _ = Binding::<f64>::new(checked, vec![other], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! A binary64 locator cannot occupy a binary32 execution slot.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, single: Handle<Value, f32>, double: Handle<Value, f64>) {
//!     let _ = double;
//!     let _ = Binding::<f32>::new(checked, vec![single], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Binding, CheckedOperation, ExecutionLimits, Handle, Value};
//! fn bind(checked: CheckedOperation, single: Handle<Value, f32>, double: Handle<Value, f64>) {
//!     let _ = double;
//!     let _ = Binding::<f32>::new(checked, vec![double], ExecutionLimits::default());
//! }
//! let _ = bind;
//! ```
//!
//! Normalized input evidence cannot be forged with a struct literal.
//!
//! ```rust
//! use nrz_matrix::NormalizedInputs;
//! fn inspect(inputs: NormalizedInputs<'_, f64>) {
//!     let _ = inputs;
//! }
//! let _ = inspect;
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_matrix::NormalizedInputs;
//! fn inspect(inputs: NormalizedInputs<'_, f64>) {
//!     let _ = NormalizedInputs::<f64> { execution: inputs.execution(), views: Vec::new() };
//! }
//! let _ = inspect;
//! ```
//!
//! A live normalized borrow excludes heap mutation until the execution is released.
//!
//! ```rust
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     drop(inputs);
//!     drop(bound);
//!     heap.compact()?;
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! ```compile_fail,E0499
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     heap.compact()?;
//!     drop(inputs);
//!     drop(bound);
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! A live normalized borrow excludes heap mutation until the execution is released.
//!
//! ```rust
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     drop(inputs);
//!     drop(bound);
//!     heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! ```compile_fail,E0499
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
//!     drop(inputs);
//!     drop(bound);
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! A live normalized borrow excludes heap mutation until the execution is released.
//!
//! ```rust
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     drop(inputs);
//!     drop(bound);
//!     heap.replace_represented(value, version, &[2.0_f64])?;
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! ```compile_fail,E0499
//! use nrz_matrix::{Binding, Extent, Handle, Heap, OperationError, Value, ValueVersion};
//! use nrz_bindings::Descriptor;
//! fn example(heap: &mut Heap, binding: Binding<f64>, value: Handle<Value, f64>, version: ValueVersion) -> Result<(), OperationError> {
//!     let bound = binding.validate(heap)?;
//!     let inputs = bound.normalized_inputs()?;
//!     let _ = inputs.views().len();
//!     heap.replace_represented(value, version, &[2.0_f64])?;
//!     drop(inputs);
//!     drop(bound);
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
