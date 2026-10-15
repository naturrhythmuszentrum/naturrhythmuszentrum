// SPDX-License-Identifier: AGPL-3.0-only
//! Public compile twins for sealed formats and private finite state.
//!
//! An admitted primitive can supply an enclosure.
//!
//! ```rust
//! use nrz_scalar::{Finite, Enclosure, ArithmeticFailure};
//! fn example() -> Result<(), ArithmeticFailure> {
//!     let value = Finite::new(1.0_f64)?;
//!     let _ = Enclosure::singleton(value);
//!     Ok(())
//! }
//! # example().expect("finite");
//! ```
//!
//! ```compile_fail,E0451
//! use nrz_scalar::{Finite, Enclosure, ArithmeticFailure};
//! fn example() -> Result<(), ArithmeticFailure> {
//!     let value = Finite { value: 1.0_f64 };
//!     let _ = Enclosure::singleton(value);
//!     Ok(())
//! }
//! # example().expect("finite");
//! ```
//!
//! Only a sealed scalar can satisfy a numeric boundary.
//!
//! ```rust
//! use nrz_scalar::Scalar;
//! #[derive(Clone, Copy, Debug)]
//! struct External;
//! fn accept<P: Scalar>(_: P) {}
//! accept(1.0_f64);
//! ```
//!
//! ```compile_fail,E0277
//! use nrz_scalar::Scalar;
//! #[derive(Clone, Copy, Debug)]
//! struct External;
//! fn accept<P: Scalar>(_: P) {}
//! accept(External);
//! ```
