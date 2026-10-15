// SPDX-License-Identifier: AGPL-3.0-only
//! Public compile-success and single-boundary compile-failure twins.
//!
//! These cases are included by rustdoc from the public conformance subtree.
//!
//! A `Companion` locator cannot occupy a value slot.
//!
//! ```rust
//! use nrz_matrix::{Companion, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Companion, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(value)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Companion, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Companion, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(other)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! A `Factor` locator cannot occupy a value slot.
//!
//! ```rust
//! use nrz_matrix::{Factor, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Factor, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(value)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Factor, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Factor, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(other)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! A `Workspace` locator cannot occupy a value slot.
//!
//! ```rust
//! use nrz_matrix::{Workspace, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Workspace, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(value)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Workspace, Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, value: Handle<Value, f64>, other: Handle<Workspace, f64>) -> Result<(), HeapError> {
//!     let _ = other;
//!     heap.value(other)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! Precision is part of the locator type.
//!
//! ```rust
//! use nrz_matrix::{Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, single: Handle<Value, f32>, double: Handle<Value, f64>) -> Result<(), HeapError> {
//!     let _ = double;
//!     heap.value::<f32>(single)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Handle, Heap, HeapError, Value};
//! fn read(heap: &Heap, single: Handle<Value, f32>, double: Handle<Value, f64>) -> Result<(), HeapError> {
//!     let _ = double;
//!     heap.value::<f32>(double)?;
//!     Ok(())
//! }
//! let _ = read;
//! ```
//!
//! Copying a locator does not expose its private fields.
//!
//! ```rust
//! use nrz_matrix::{Handle, Value};
//! fn copy(value: Handle<Value, f64>) {
//!     let _ = value;
//! }
//! let _ = copy;
//! ```
//!
//! ```compile_fail,E0616
//! use nrz_matrix::{Handle, Value};
//! fn copy(value: Handle<Value, f64>) {
//!     let _ = value.scope;
//! }
//! let _ = copy;
//! ```
//!
//! A borrowed region prevents heap growth until its last use.
//!
//! ```rust
//! use nrz_matrix::{Extent, Heap, HeapError, HeapLimits};
//! use nrz_units::Descriptor;
//! fn example() -> Result<(), HeapError> {
//!     let mut heap = Heap::new(HeapLimits::default())?;
//!     let extent = Extent::new(1, 1)?;
//!     let quantity = Descriptor::ratio();
//!     let bundle = heap.import_represented(extent, &quantity, &[1.0_f64])?;
//!     let view = heap.value(bundle.value())?;
//!     let _ = view.region().get(0, 0)?;
//!     heap.import_represented(extent, &quantity, &[2.0_f64])?;
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! ```compile_fail,E0502
//! use nrz_matrix::{Extent, Heap, HeapError, HeapLimits};
//! use nrz_units::Descriptor;
//! fn example() -> Result<(), HeapError> {
//!     let mut heap = Heap::new(HeapLimits::default())?;
//!     let extent = Extent::new(1, 1)?;
//!     let quantity = Descriptor::ratio();
//!     let bundle = heap.import_represented(extent, &quantity, &[1.0_f64])?;
//!     let view = heap.value(bundle.value())?;
//!     heap.import_represented(extent, &quantity, &[2.0_f64])?;
//!     let _ = view.region().get(0, 0)?;
//!     Ok(())
//! }
//! let _ = example;
//! ```
//!
//! Only the sealed primitive scalar formats can enter an arena.
//!
//! ```rust
//! use nrz_matrix::{Extent, Heap, HeapError};
//! use nrz_units::Descriptor;
//! #[derive(Clone, Copy, Debug)]
//! struct External;
//! fn import(heap: &mut Heap, data: &[f64]) -> Result<(), HeapError> {
//!     heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), data)?;
//!     Ok(())
//! }
//! let _ = import;
//! let _ = External;
//! ```
//!
//! ```compile_fail,E0277
//! use nrz_matrix::{Extent, Heap, HeapError};
//! use nrz_units::Descriptor;
//! #[derive(Clone, Copy, Debug)]
//! struct External;
//! fn import(heap: &mut Heap, data: &[External]) -> Result<(), HeapError> {
//!     heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), data)?;
//!     Ok(())
//! }
//! let _ = import;
//! let _ = External;
//! ```
