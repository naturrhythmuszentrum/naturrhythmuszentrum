// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit coordinate maps and finite root certificates for delta pullbacks.
//!
//! # Checked delta pullbacks
//!
//! `Task::Pullback { source_space, target_space, map_kind }` consumes an original delta and a scalar map from target to source. `PullbackKind::Affine` checks a nonzero scale, the complete interval image and the inverse scalar expression in the source coordinate. The output uses the reciprocal absolute Jacobian: `δ(2x−1) = ½δ_(1/2)` and `δ(−3x+6) = ⅓δ_2`. Map syntax permits positive powers and division by nonzero constant polynomials; strict zero powers and hidden denominator holes refuse. Both spaces and both maps remain in `DistributionStatement::Pullback`.
//!
//! `PullbackKind::SimpleZeros(roots)` requires the source zero delta on the whole real axis and a complete sorted rational-root list. The checker verifies the entire factorization, each root, each nonzero slope and its reciprocal absolute value. Thus `δ(x²−1) = ½δ_(-1) + ½δ_1`; a repeated root or omitted factor returns `PullbackHypotheses`. Roots outside the target interval remain in the certificate but contribute no delta. `pullback_certificate` exposes this checked evidence; `pullback_zeros` bounds all supplied roots before copying and replay allocation. General root solving and arbitrary nonlinear distribution pullbacks remain outside this fragment.

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
