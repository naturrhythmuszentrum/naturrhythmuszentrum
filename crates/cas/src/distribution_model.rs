// SPDX-License-Identifier: AGPL-3.0-only
//! Exact distribution statements and finite certificates, separate from scalar claims.

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
            Self::Regularity { space, .. }
            | Self::Embedding { space, .. }
            | Self::Equality { space, .. }
            | Self::Derivative { space, .. }
            | Self::Pairing { space, .. }
            | Self::SupportAction { space, .. } => space,
        }
    }
}
