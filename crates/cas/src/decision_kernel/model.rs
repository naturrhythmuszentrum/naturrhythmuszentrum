// SPDX-License-Identifier: AGPL-3.0-only
//! Pure exact records for the closed scalar certificate checker.

use crate::{Handle, Rational};

/// One closed scalar law; indices refer only to earlier complete records.
///
/// Each referenced record supplies its entire real interval, so all arithmetic
/// relations cover the full Cartesian input box. No floating value is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarKernelLaw {
    /// The independently checked exact rational constant, before numerical import.
    Rational(Rational),
    /// Exact real addition of the ordered earlier records.
    Sum {
        /// Earlier left operand.
        left: u32,
        /// Earlier right operand.
        right: u32,
    },
    /// Exact real product of the ordered earlier records.
    Product {
        /// Earlier left operand.
        left: u32,
        /// Earlier right operand.
        right: u32,
    },
    /// Exact reciprocal of an earlier interval proved away from zero.
    Reciprocal {
        /// Earlier divisor record.
        input: u32,
    },
    /// Nonnegative square root of an earlier interval proved strictly positive.
    SquareRoot {
        /// Earlier positive radicand record.
        input: u32,
    },
}

/// An untrusted exact endpoint claim for one operation in a finite scalar trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelRecord {
    /// The closed mathematical operation and its complete operand references.
    pub law: ScalarKernelLaw,
    /// Included rational lower endpoint, exported from a scalar dyadic.
    pub lower: Rational,
    /// Included rational upper endpoint, exported from a scalar dyadic.
    pub upper: Rational,
}

/// Pure scalar trace under the exact original context and admitted input family.
///
/// Version one has rational leaves and closed sums, products, reciprocals and
/// positive square roots. The last record is the original target. A quotient
/// uses reciprocal then product; every record is checked before any result.
/// Closed leaves establish coverage throughout the retained family without a
/// new input assumption. General physical input families remain unsupported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelCertificate {
    /// Independently versioned closed law and endpoint-reduction grammar.
    pub version: u32,
    /// Exact original expression, including every operand before normalization.
    pub expression: Handle,
    /// Original context; a nominal substitute cannot stand for another scope.
    pub context: Handle,
    /// The exact admitted region of that original context.
    pub input_family: Handle,
    /// Nonempty child-before-parent records; the final record is the result.
    pub records: Vec<ScalarKernelRecord>,
}
