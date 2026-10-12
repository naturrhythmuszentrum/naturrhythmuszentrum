// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod expression;
mod normal;
mod rational;

pub use expression::{Error, Expr, Limits, Node, Span, Symbol};
pub use normal::NormalForm;
pub use rational::{ArithmeticError, Rational};
