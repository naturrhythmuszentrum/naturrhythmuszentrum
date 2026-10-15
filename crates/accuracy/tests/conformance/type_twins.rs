// SPDX-License-Identifier: AGPL-3.0-only
//! Public compile twins for the nonnegative and borrowed-vector invariants.
//!
//! A finite magnitude is obtained through validation.
//!
//! ```rust
//! use nrz_accuracy::{AccuracyFailure, Nonnegative};
//! use nrz_scalar::Finite;
//! fn example() -> Result<(), AccuracyFailure> {
//!     let value = Finite::new(1.0_f64)?;
//!     let _bound = Nonnegative::new(value.get())?;
//!     Ok(())
//! }
//! # example().expect("valid magnitude");
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_accuracy::{AccuracyFailure, Nonnegative};
//! use nrz_scalar::Finite;
//! fn example() -> Result<(), AccuracyFailure> {
//!     let value = Finite::new(1.0_f64)?;
//!     let _bound = Nonnegative { value };
//!     Ok(())
//! }
//! # example().expect("valid magnitude");
//! ```
//!
//! Plain data must pass the immutable vector constructor.
//!
//! ```rust
//! use nrz_accuracy::{AccuracyFailure, BoundVectorRef};
//! fn example() -> Result<(), AccuracyFailure> {
//!     let values = [1.0_f64];
//!     let _vector = BoundVectorRef::new(&values)?;
//!     Ok(())
//! }
//! # example().expect("valid entries");
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_accuracy::{AccuracyFailure, BoundVectorRef};
//! fn example() -> Result<(), AccuracyFailure> {
//!     let values = [1.0_f64];
//!     let _vector = BoundVectorRef { values: &values };
//!     Ok(())
//! }
//! # example().expect("valid entries");
//! ```
