// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit coordinate maps and finite root certificates for delta pullbacks.

use crate::graph::Handle;
use crate::number::Rational;

/// The executable delta-pullback fragments; roots are untrusted factorization data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullbackKind {
    /// An invertible affine map, with its inverse constructed and checked.
    Affine,
    /// A rational-root polynomial, with the complete strictly increasing root list supplied.
    SimpleZeros(Vec<Rational>),
}

/// The checked map family, independent of its certificate's untrusted root list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullbackMode {
    /// Affine diffeomorphism between the complete declared intervals.
    Affine,
    /// Polynomial pullback of the source zero delta through finitely many simple roots.
    SimpleZeros,
}

/// Pullback along g: target interval -> source interval, with both distribution spaces retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullbackValue {
    /// Original delta in the source space.
    pub source: Handle,
    /// Exact resulting distribution in the target space.
    pub result: Handle,
    /// Scalar map in the target coordinate.
    pub map: Handle,
    /// Scalar inverse in the source coordinate, required exactly for affine mode.
    pub inverse: Option<Handle>,
    /// Space of the original distribution.
    pub source_space: Handle,
    /// Space of the resulting distribution.
    pub target_space: Handle,
    /// Exact admitted mathematical convention.
    pub mode: PullbackMode,
}

/// Independently checked local change-of-variable data at a simple root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullbackZero {
    /// Exact preimage of the delta support.
    pub point: Rational,
    /// Nonzero derivative of the map at this point.
    pub slope: Rational,
    /// Positive reciprocal absolute Jacobian.
    pub inverse_jacobian: Rational,
}

/// A complete finite factorization and local Jacobian certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullbackCertificate {
    /// Leading coefficient of the actual map polynomial.
    pub leading: Rational,
    /// Every root, including those outside the target interval.
    pub zeros: Vec<PullbackZero>,
}
