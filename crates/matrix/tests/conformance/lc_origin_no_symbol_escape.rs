// SPDX-License-Identifier: AGPL-3.0-only
//! Actual public surfaces have success twins and single-boundary symbol-escape failures.
//!
//! Prepared ownership can move with only the original inventory lifetime.
//! ```rust
//! use nrz_matrix::{LcOmegaPrepared, LcOmegaPrepareFailure, LcOmegaOutput, LcShapeError};
//! fn transfer<'i>(owner: LcOmegaPrepared<'i, f32, 5>) -> LcOmegaPrepared<'i, f32, 5> { owner }
//! fn recover<'i>(failure: LcOmegaPrepareFailure<'i, f64, 5>) -> (LcOmegaOutput<'i, f64, 5>, LcShapeError) { failure.into_parts() }
//! ```
//!
//! `LcOmegaOutput<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0624
//! use nrz_matrix::LcOmegaOutput;
//! fn escape(owner: LcOmegaOutput<'_, f32, 5>) { let _ = owner.symbols(); }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.symbols(); }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.declaration; }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.correspondence; }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.receipt; }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.clone(); }
//! ```
//!
//! `LcOmegaPrepared<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f32, 5>) { let _ = owner.publish(); }
//! ```
//!
//! `LcOmegaPrepareFailure<f32>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepareFailure;
//! fn escape(owner: LcOmegaPrepareFailure<'_, f32, 5>) { let _ = owner.declaration; }
//! ```
//!
//! `LcOmegaOutput<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0624
//! use nrz_matrix::LcOmegaOutput;
//! fn escape(owner: LcOmegaOutput<'_, f64, 5>) { let _ = owner.symbols(); }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.symbols(); }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.declaration; }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.correspondence; }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.receipt; }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.clone(); }
//! ```
//!
//! `LcOmegaPrepared<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0599
//! use nrz_matrix::LcOmegaPrepared;
//! fn escape(owner: LcOmegaPrepared<'_, f64, 5>) { let _ = owner.publish(); }
//! ```
//!
//! `LcOmegaPrepareFailure<f64>` supplies no escaping capability through this route.
//! ```compile_fail,E0616
//! use nrz_matrix::LcOmegaPrepareFailure;
//! fn escape(owner: LcOmegaPrepareFailure<'_, f64, 5>) { let _ = owner.declaration; }
//! ```
