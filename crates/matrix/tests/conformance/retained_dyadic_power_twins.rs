// SPDX-License-Identifier: AGPL-3.0-only
//! Only the owned original preparation route and separately charged retention create uses.
//!
//! A funded use moves without acquiring another edge.
//! ```rust
//! use nrz_matrix::RetainedDyadicPower;
//! fn transfer(law: RetainedDyadicPower) -> RetainedDyadicPower { law }
//! let _transfer = transfer;
//! ```
//!
//! Neither the use nor the bindings preparation has an external field constructor.
//! ```compile_fail,E0451
//! use nrz_matrix::RetainedDyadicPower;
//! fn forge() { let _law = RetainedDyadicPower { record: panic!() }; }
//! ```
//! ```compile_fail,E0451
//! use nrz_bindings::DyadicPowerPreparation;
//! fn forge() { let _preparation = DyadicPowerPreparation {
//!     checked: panic!(), policy: panic!(), proof_bytes: panic!()
//! }; }
//! ```
//!
//! A borrowed use and a bindings preparation cannot be cloned into authority.
//! ```compile_fail,E0308
//! use nrz_matrix::RetainedDyadicPower;
//! fn duplicate(law: &RetainedDyadicPower) -> RetainedDyadicPower { law.clone() }
//! ```
//! ```compile_fail,E0308
//! use nrz_bindings::DyadicPowerPreparation;
//! fn duplicate(preparation: &DyadicPowerPreparation) -> DyadicPowerPreparation { preparation.clone() }
//! ```
//!
//! The producing graph has no mutable or extracting accessor.
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! fn mutate(mut law: RetainedDyadicPower) { let _graph = law.graph_mut(); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! fn extract(law: RetainedDyadicPower) { let _graph = law.into_cas(); }
//! ```
//!
//! Supplied judgment, program, key and reading cannot construct the owner.
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! use nrz_bindings::DyadicPowerPreparation;
//! fn substitute(authority: DyadicPowerPreparation) { let _ = RetainedDyadicPower::from_prepared(authority); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! use nrz_scalar::DyadicPowerProgram;
//! fn substitute(program: DyadicPowerProgram) { let _ = RetainedDyadicPower::from_program(program); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! fn substitute(key: &[u8]) { let _ = RetainedDyadicPower::from_key(key); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedDyadicPower;
//! use nrz_scalar::DyadicPowerOutput;
//! fn substitute(reading: DyadicPowerOutput) { let _ = RetainedDyadicPower::from_reading(reading); }
//! ```
//!
//! Evaluation authority cannot outlive the borrowed funded use.
//! ```compile_fail,E0515
//! use nrz_matrix::{DyadicPowerEvaluation, RetainedDyadicPower};
//! use nrz_scalar::{ArithmeticContext, DyadicPowerFailure};
//! fn escape(law: RetainedDyadicPower, context: &ArithmeticContext)
//!     -> Result<DyadicPowerEvaluation<'static>, DyadicPowerFailure> { law.evaluate(1.0, context) }
//! ```
