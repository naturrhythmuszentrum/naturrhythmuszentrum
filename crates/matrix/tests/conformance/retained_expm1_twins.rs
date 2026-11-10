// SPDX-License-Identifier: AGPL-3.0-only
//! Only owned preparation and charged retention create law uses.
//!
//! A use can move without acquiring an edge.
//! ```rust
//! use nrz_matrix::RetainedExpm1;
//! fn transfer(law: RetainedExpm1) -> RetainedExpm1 { law }
//! let _transfer = transfer;
//! ```
//!
//! A use has no external field constructor.
//! ```compile_fail,E0451
//! use nrz_matrix::RetainedExpm1;
//! fn forge() { let _law = RetainedExpm1 { record: panic!() }; }
//! ```
//!
//! A borrow cannot create an uncharged independent use.
//! ```compile_fail,E0308
//! use nrz_matrix::RetainedExpm1;
//! fn duplicate(law: &RetainedExpm1) -> RetainedExpm1 { law.clone() }
//! ```
//!
//! Frozen graphs cannot be mutated or extracted.
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! fn mutate(mut law: RetainedExpm1) { let _graph = law.graph_mut(); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! fn extract(law: RetainedExpm1) { let _graph = law.into_cas(); }
//! ```
//!
//! A separately supplied authority, program, key or reading grants no constructor.
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! use nrz_bindings::readings::algebra::enclosure::prepared_expm1::Prepared;
//! fn substitute(authority: Box<Prepared>) { let _law = RetainedExpm1::from_prepared(authority); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! use nrz_scalar::Expm1Program;
//! fn substitute(program: Expm1Program) { let _law = RetainedExpm1::from_program(program); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! fn substitute(key: &str) { let _law = RetainedExpm1::from_key(key); }
//! ```
//! ```compile_fail,E0599
//! use nrz_matrix::RetainedExpm1;
//! use nrz_scalar::Expm1Output;
//! fn substitute(reading: Expm1Output) { let _law = RetainedExpm1::from_reading(reading); }
//! ```
//!
//! Evaluated authority cannot escape its retaining use.
//! ```compile_fail,E0515
//! use nrz_matrix::{RetainedExpm1, Expm1Evaluation};
//! use nrz_scalar::{ArithmeticContext, Expm1Input, Expm1Failure};
//! fn escape(law: RetainedExpm1, input: Expm1Input, context: &ArithmeticContext)
//!     -> Result<Expm1Evaluation<'static>, Expm1Failure> { law.evaluate(input, context) }
//! ```
