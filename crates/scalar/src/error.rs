// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit refusals of finite arithmetic claims.

/// A failed admission or an operation without a finite enclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArithmeticFailure {
    /// An input is infinite or not a number.
    #[error("a nonfinite value cannot be admitted")]
    NonFinite,
    /// The proposed lower endpoint exceeds its upper endpoint.
    #[error("the lower endpoint exceeds the upper endpoint")]
    InvertedEndpoints,
    /// Division is undefined somewhere in the divisor enclosure.
    #[error("the divisor enclosure contains zero")]
    ZeroInDivisor,
    /// Real square root requires a nonnegative lower endpoint.
    #[error("the square root enclosure includes negative inputs")]
    NegativeRoot,
    /// A rounded result or required outward neighbour is nonfinite.
    #[error("the result cannot be enclosed by finite endpoints")]
    Overflow,
    /// Numeric text does not follow the bounded literal grammar.
    #[error("invalid numeric literal")]
    InvalidLiteral,
    /// Literal evaluation needs an unbounded intermediate or result.
    #[error("numeric literal exceeds the supported evaluation range")]
    LiteralRange,
    /// A literal's digit, exponent or work limit was exceeded.
    #[error("numeric literal resource limit exceeded")]
    ResourceLimit,
    /// Dot-product slices have different lengths.
    #[error("dot-product lengths differ")]
    LengthMismatch,
    /// The target or selected floating environment probes are unsupported.
    #[error("the floating arithmetic environment is unsupported")]
    UnsupportedEnvironment,
}
