// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical covariance errors cannot become physical-source bound handles.
//!
//! ```rust
//! use nrz_matrix::{Companion, Handle, Heap, HeapError, ParameterErrorView};
//! fn physical_bound(heap: &Heap, bound: Handle<Companion, f64>, error: ParameterErrorView<'_, f64>) -> Result<(), HeapError> {
//!     let _ = error;
//!     heap.companion(bound)?;
//!     Ok(())
//! }
//! let _ = physical_bound;
//! ```
//!
//! ```compile_fail,E0308
//! use nrz_matrix::{Companion, Handle, Heap, HeapError, ParameterErrorView};
//! fn physical_bound(heap: &Heap, bound: Handle<Companion, f64>, error: ParameterErrorView<'_, f64>) -> Result<(), HeapError> {
//!     let _ = bound;
//!     heap.companion(error.target())?;
//!     Ok(())
//! }
//! let _ = physical_bound;
//! ```
//!
//! Sufficient PSD evidence has no public field constructor.
//!
//! ```rust
//! use nrz_matrix::{DenseCovariance, PsdCertificate};
//! fn inspect(value: DenseCovariance<f64>) { let _ = value.certificate(); }
//! let _ = inspect;
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_matrix::{DenseCovariance, PsdCertificate};
//! fn inspect(value: DenseCovariance<f64>) {
//!     let _ = DenseCovariance { order: 1, centers: vec![1.0], family: value.family().to_vec(), errors: vec![0.0], certificate: PsdCertificate::DiagonalDominance };
//! }
//! let _ = inspect;
//! ```
