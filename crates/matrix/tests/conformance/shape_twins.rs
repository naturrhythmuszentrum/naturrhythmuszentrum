// SPDX-License-Identifier: AGPL-3.0-only
//! A checked shape is inspected through accessors, not fabricated fields.
//!
//! ```rust
//! use nrz_matrix::CheckedShape;
//! fn inspect(shape: &CheckedShape) {
//!     let _ = shape.domain();
//! }
//! let _ = inspect;
//! ```
//!
//! ```compile_fail,E0616
//! use nrz_matrix::CheckedShape;
//! fn inspect(shape: &CheckedShape) {
//!     let _ = shape.domain;
//! }
//! let _ = inspect;
//! ```
//!
//! A draft cannot occupy a checked contract boundary.
//!
//! ```rust
//! use nrz_matrix::{CheckedShape, ShapeDraft};
//! fn consume(_: &CheckedShape) {}
//! fn use_shape(checked: &CheckedShape, draft: &ShapeDraft) {
//!     let _ = draft;
//!     consume(checked);
//! }
//! let _ = use_shape;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{CheckedShape, ShapeDraft};
//! fn consume(_: &CheckedShape) {}
//! fn use_shape(checked: &CheckedShape, draft: &ShapeDraft) {
//!     let _ = draft;
//!     consume(draft);
//! }
//! let _ = use_shape;
//! ```
