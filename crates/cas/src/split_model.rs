// SPDX-License-Identifier: AGPL-3.0-only
//! Finite split calculus retains its bounds, extension convention and endpoint evidence.

use crate::graph::Handle;
use crate::number::Rational;
use crate::region::RegionCertificate;

/// A named coordinate and exact finite bounds; summation includes both endpoints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitRange {
    /// Declared real coordinate, restricted to integer arguments for a sum.
    pub coordinate: String,
    /// Oriented initial bound.
    pub lower: Rational,
    /// Oriented final bound.
    pub upper: Rational,
}

/// The mathematical operation established by a split certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitKind {
    /// Riemann integral of any finite-point extension of the polynomial open cells.
    Integral,
    /// Inclusive finite sum; every integer argument must have a value.
    Sum,
}

/// A finite exact value, with no assertion about improper convergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitValue {
    /// Integral or inclusive integer sum.
    pub kind: SplitKind,
    /// Original complete function syntax.
    pub function: Handle,
    /// Closed interval containing the complete calculation.
    pub domain: Handle,
    /// Explicit bounds and coordinate identity.
    pub range: SplitRange,
    /// Exact oriented result.
    pub value: Rational,
}

/// A polynomial interval with a derivative or forward-difference witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitPiece {
    /// Position in the independently reconstructed cell partition.
    pub cell: u32,
    /// Lower endpoint; included for a sum.
    pub lower: Rational,
    /// Upper endpoint; included for a sum.
    pub upper: Rational,
    /// Canonical ascending coefficients of the local function.
    pub polynomial: Vec<Rational>,
    /// Canonical coefficients of F with F' = p or F(x+1)-F(x) = p(x).
    pub primitive: Vec<Rational>,
    /// F(lower).
    pub start: Rational,
    /// F(upper) for an integral, F(upper+1) for a sum.
    pub end: Rational,
}

/// Actual ownership at an integration cut, including certified absence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitPoint {
    /// Exact cut location, including the two closed endpoints.
    pub point: Rational,
    /// Actual value, or absence permitting a finite-point extension.
    pub value: Option<Rational>,
}

/// Untrusted geometry, finite exceptions and polynomial endpoint witnesses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitCertificate {
    /// Complete exact selector partition in structural region order.
    pub partition: RegionCertificate,
    /// Ordered nonempty intervals or disjoint inclusive integer ranges.
    pub pieces: Vec<SplitPiece>,
    /// Every inhabited singleton of an integral; empty for a sum.
    pub points: Vec<SplitPoint>,
}
