// SPDX-License-Identifier: AGPL-3.0-only
//! Exact distribution statements and finite certificates, separate from scalar claims.
//!
//! # Distribution judgments and calculus
//!
//! `EmbedRegular { space }` certifies finite polynomial cells before constructing a regular distribution. `DistributionStatement` groups regularity, embedding, equality, derivative, pairing and supported-action judgments under `Statement::Distribution`. Every judgment retains its exact real coordinate, open interval and test family, including scalar-valued pairing results. Regular representatives may differ at certified finite points; their pointwise ownership remains distinct.
//!
//! The first nine distribution schemas are `dist.regular`, `dist.ae`, `dist.linear`, `dist.derivative`, `dist.delta-pair`, `dist.smooth`, `dist.delta-multiply`, `dist.support-action` and `dist.jump`. The jump checker independently compares polynomial derivatives and right-minus-left traces on the complete partition. Higher derivatives repeat the first-order rule and increment existing delta orders. Ordinary declared tests produce symbolic derivative evaluations; `PairNearSupport` instead requires a checked finite delta support and a polynomial germ on an explicit neighborhood. It evaluates `PairNearSupport(δ₂, x² + 3) = 7` and `PairNearSupport(δ′₂, x² + 3) = −4` without classifying a polynomial as a compactly supported test.
//!
//! The derived integration-by-parts, delta-primitive and classical-bridge recipes retain primitive expansions and add no registry keys. Named and expanded routes enter both queues before completed-result dominance. Native equality shortcuts retain replayable expansions and the older frozen cost manifest; strict improvement is checked in the complete typed theorem class without scalar projection. Partition reuse applies during generation and independent replay.
//!
//! `ApplyAt` refuses distribution point values; `Product` refuses delta-square and a multiplier proved discontinuous at the delta support. The corresponding public reasons are `DistributionPointValue`, `DistributionProductUndefined` and `TestSpaceMismatch`. Unsupported smoothness remains `PendingPremise`. The independent `delta_order`, `test_nodes` and cumulative `pairing_steps` limits cover these operations.

use crate::graph::Handle;
use crate::number::Rational;
use crate::region::RegionCertificate;

/// Regularity proved by a finite rational polynomial certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegularityKind {
    /// Locally integrable despite arbitrary finite exceptional values.
    LocallyIntegrable,
    /// Polynomial on a complete finite family of open cells.
    PiecewisePolynomial,
    /// One polynomial throughout the interval.
    Smooth,
}

/// A statement whose scalar actions retain the exact distribution space.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DistributionStatement {
    /// Delta pullback retaining both spaces, the map and its affine inverse when applicable.
    Pullback(Box<crate::pullback_model::PullbackValue>),
    /// The difference of two distributions with equal derivatives is constant on this interval.
    PrimitiveUnique {
        /// First primitive.
        left: Handle,
        /// Second primitive.
        right: Handle,
        /// Exact connected open interval space.
        space: Handle,
    },
    /// A checked analytic property of a pointwise function.
    Regularity {
        /// Pointwise function.
        function: Handle,
        /// Exact interval, axis, field and test family.
        space: Handle,
        /// Independently established property.
        property: RegularityKind,
    },
    /// Explicit checked regular embedding.
    Embedding {
        /// Pointwise representative.
        function: Handle,
        /// Distribution constructor.
        embedding: Handle,
        /// Exact distribution space.
        space: Handle,
    },
    /// Equality of distributions in one exact space.
    Equality {
        /// Left distribution.
        left: Handle,
        /// Right distribution.
        right: Handle,
        /// Exact distribution space.
        space: Handle,
    },
    /// An iterated distribution derivative, built from first-order rules.
    Derivative {
        /// Original distribution.
        distribution: Handle,
        /// Computed distribution.
        derivative: Handle,
        /// Exact distribution space.
        space: Handle,
        /// Positive number of differentiations.
        order: u32,
    },
    /// Scalar action, with no change of mathematical layer.
    Pairing {
        /// Ordinary or supported pairing syntax.
        pairing: Handle,
        /// Scalar value, possibly a symbolic test evaluation.
        value: Handle,
        /// Exact space of the action.
        space: Handle,
    },
    /// Cutoff-independent action of a smooth germ near finite support.
    SupportAction {
        /// Structurally supported distribution.
        distribution: Handle,
        /// Smooth scalar germ.
        function: Handle,
        /// Open neighborhood of the reconstructed support.
        neighborhood: Handle,
        /// Exact distribution space.
        space: Handle,
    },
}

/// A finite polynomial interpretation with every partition row retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegularCertificate {
    /// Complete structural selector and domain inventory.
    pub regions: Vec<Handle>,
    /// Independently reconstructed exact cells.
    pub partition: RegionCertificate,
    /// Polynomial coefficients for each open cell; absent only outside the interval.
    pub polynomials: Vec<Option<Vec<Rational>>>,
}

/// Closed distribution evidence; no acceptance flags occur on the wire.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DistributionCertificate {
    /// Complete finite polynomial cell certificate.
    Regular(Box<RegularCertificate>),
    /// Exact finite exceptional set and its complement partition.
    AlmostEverywhere {
        /// All exceptions, in increasing rational order.
        exceptions: Vec<Rational>,
        /// Complete complement and endpoint decomposition.
        partition: RegionCertificate,
    },
    /// Structurally reconstructed finite support, with no omitted point.
    Support(Vec<Rational>),
}

impl DistributionStatement {
    /// Borrow the exact space retained by every statement, including scalar actions.
    #[must_use]
    pub const fn space(&self) -> &Handle {
        match self {
            Self::Pullback(value) => &value.target_space,
            Self::PrimitiveUnique { space, .. }
            | Self::Regularity { space, .. }
            | Self::Embedding { space, .. }
            | Self::Equality { space, .. }
            | Self::Derivative { space, .. }
            | Self::Pairing { space, .. }
            | Self::SupportAction { space, .. } => space,
        }
    }
}
