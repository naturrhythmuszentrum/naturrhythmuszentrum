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
//!
//! Only a binary32 run exposes the explicitly authorized binary64 retry.
//!
//! ```rust
//! use nrz_accuracy::{AccuracyFailure, Limits, Promotion, RecoveryPolicy};
//! use nrz_scalar::ArithmeticFailure;
//! fn example() -> Result<(), AccuracyFailure> {
//!     let policy = RecoveryPolicy::new(0, 1, Promotion::F32ToF64)?;
//!     let mut run = policy.start::<f32>(None, Limits::new(2, 2));
//!     run.begin_attempt(1)?;
//!     run.fail(ArithmeticFailure::NonFinite)?;
//!     let _retry = run.retry_f64()?;
//!     Ok(())
//! }
//! # example().expect("permitted retry");
//! ```
//!
//! ```compile_fail,E0599
//! use nrz_accuracy::{AccuracyFailure, Limits, Promotion, RecoveryPolicy};
//! use nrz_scalar::ArithmeticFailure;
//! fn example() -> Result<(), AccuracyFailure> {
//!     let policy = RecoveryPolicy::new(0, 1, Promotion::F32ToF64)?;
//!     let mut run = policy.start::<f64>(None, Limits::new(2, 2));
//!     run.begin_attempt(1)?;
//!     run.fail(ArithmeticFailure::NonFinite)?;
//!     let _retry = run.retry_f64()?;
//!     Ok(())
//! }
//! # example().expect("permitted retry");
//! ```
//!
//! Budget coefficients enter through the checked domain boundary.
//!
//! ```rust
//! use nrz_accuracy::{AccuracyFailure, Budget};
//! use nrz_scalar::{Enclosure, Finite};
//! fn example() -> Result<(), AccuracyFailure> {
//!     let absolute = Enclosure::singleton(Finite::<f64>::ZERO);
//!     let relative = Enclosure::singleton(Finite::<f64>::ZERO);
//!     let reference_scale = Enclosure::singleton(Finite::<f64>::ONE);
//!     let _budget = Budget::from_enclosures(absolute, relative, reference_scale)?;
//!     Ok(())
//! }
//! # example().expect("valid budget");
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_accuracy::{AccuracyFailure, Budget};
//! use nrz_scalar::{Enclosure, Finite};
//! fn example() -> Result<(), AccuracyFailure> {
//!     let absolute = Enclosure::singleton(Finite::<f64>::ZERO);
//!     let relative = Enclosure::singleton(Finite::<f64>::ZERO);
//!     let reference_scale = Enclosure::singleton(Finite::<f64>::ONE);
//!     let _budget = Budget { absolute, relative, reference_scale };
//!     Ok(())
//! }
//! # example().expect("valid budget");
//! ```
