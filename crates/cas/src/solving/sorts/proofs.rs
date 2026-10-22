// SPDX-License-Identifier: AGPL-3.0-only
//! Closed finite certificate shapes; representation never enables an unavailable checker.

use crate::graph::Handle;

/// A reconstructed Real sign or equality conclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealSignConclusion {
    /// Strictly negative expression.
    Negative,
    /// Zero expression or a proved equality predicate.
    Zero,
    /// Strictly positive expression.
    Positive,
    /// Nonnegative expression, including zero.
    NonNegative,
    /// Nonpositive expression, including zero.
    NonPositive,
    /// Nonzero expression whose strict sign need not be known.
    NonZero,
}

/// Closed ordered-field rule vocabulary; each use retains its exact earlier premises.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealSignRule {
    /// Direct exact rational comparison.
    Rational,
    /// Transport through a checked same-Real identity.
    Identity,
    /// Sign of a negation.
    Negation,
    /// Compatible signs of a sum.
    Sum,
    /// Product of known signs.
    Product,
    /// Sign of a square or positive even power.
    EvenPower,
    /// Reflection by a specified positive odd power.
    OddPowerReflection,
    /// Exact local trichotomy hypothesis.
    Hypothesis,
    /// Earlier Real root equation and independently established sign.
    Root,
}

/// One node in an ordered sign derivation; argument indices must point backward.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealSignStep<R = Handle> {
    /// Closed rule.
    pub rule: RealSignRule,
    /// Exact Real expression or equality predicate.
    pub subject: R,
    /// Claimed sign, reconstructed by the checker.
    pub conclusion: RealSignConclusion,
    /// Earlier sign-node indices in the same bounded derivation.
    pub arguments: Vec<u32>,
    /// Earlier application-premise slots.
    pub premises: Vec<u32>,
    /// Actual power for a power rule; zero for other rules.
    pub exponent: u32,
}

/// Agreement of a finite same-sort evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementCoefficientEvaluation<R = Handle> {
    /// Exact finite evaluation object.
    pub evaluation: R,
    /// Original finite polynomial expression.
    pub left: R,
    /// Claimed evaluated expression.
    pub right: R,
    /// Exact evaluation region.
    pub region: R,
}

/// Finite evaluation evidence with every prerequisite explicitly indexed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateCoefficientEvaluation<R = Handle> {
    /// Exact finite evaluation object.
    pub evaluation: R,
    /// Original finite polynomial expression.
    pub left: R,
    /// Claimed evaluated expression.
    pub right: R,
    /// Ordered finite polynomial support.
    pub support: Vec<R>,
    /// Image-definedness premise slots in source-slot order.
    pub definedness: Vec<u32>,
    /// Localization-unit premise slots in localization order.
    pub units: Vec<u32>,
    /// Prior selected-root-existence premise slots.
    pub existence: Vec<u32>,
    /// Exact evaluation region.
    pub region: R,
}

/// Exact Real sign on one coefficient stratum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRealSign<R = Handle> {
    /// Real expression or equality predicate.
    pub subject: R,
    /// Exact coefficient stratum.
    pub region: R,
    /// Reconstructed sign conclusion.
    pub conclusion: RealSignConclusion,
}

/// A bounded ordered-field sign derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRealSign<R = Handle> {
    /// Real expression or equality predicate.
    pub subject: R,
    /// Exact coefficient stratum.
    pub region: R,
    /// Ordered finite sign derivation.
    pub steps: Vec<RealSignStep<R>>,
    /// Final sign-node index.
    pub root: u32,
}

/// Exact counts in ordered cells and the exterior of their union.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRealRootCount<R = Handle> {
    /// Original Real root relation.
    pub relation: R,
    /// Exact coefficient stratum.
    pub region: R,
    /// Ordered Real cells.
    pub cells: Vec<R>,
    /// Root counts in the same cell order.
    pub counts: Vec<u32>,
    /// Count below the cell inventory.
    pub exterior_left: u32,
    /// Count above the cell inventory.
    pub exterior_right: u32,
}

/// Finite signed-remainder, endpoint and infinity evidence for Sturm counting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRealRootCount<R = Handle> {
    /// Original Real root relation.
    pub relation: R,
    /// Exact coefficient stratum.
    pub region: R,
    /// Ordered Real cells.
    pub cells: Vec<R>,
    /// Square-free polynomial support.
    pub square_free: R,
    /// Exact derivative of the original polynomial, before square-free reduction.
    pub derivative: R,
    /// Terminal derivative gcd.
    pub gcd: R,
    /// Signed remainder polynomials in descending degree order.
    pub chain: Vec<R>,
    /// Original derivative divided by the gcd, two ordered Bezout multipliers, then signed-chain quotients.
    pub quotients: Vec<R>,
    /// Optional positive coefficient rescalings for chain entries from index two, in that order.
    pub rescalings: Vec<R>,
    /// Additional identity premise slots; empty when finite identities are reconstructed directly.
    pub identities: Vec<u32>,
    /// Leading-unit premise slots.
    pub units: Vec<u32>,
    /// Rescaling signs, both infinity leading-sign rows, then each cell's order and endpoint derivative signs.
    pub signs: Vec<u32>,
    /// Claimed counts in the same cell order.
    pub counts: Vec<u32>,
    /// Claimed count below the inventory.
    pub exterior_left: u32,
    /// Claimed count above the inventory.
    pub exterior_right: u32,
}

/// Existence and uniqueness in an independently defined count-one Real cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementRealCellExists<R = Handle> {
    /// Original Real root relation.
    pub relation: R,
    /// Exact Real cell.
    pub cell: R,
    /// Exact selection predicate.
    pub predicate: R,
    /// Exact coefficient stratum.
    pub region: R,
}

/// Earlier count-one and endpoint evidence for a Real selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateRealCellExists<R = Handle> {
    /// Original Real root relation.
    pub relation: R,
    /// Exact Real cell.
    pub cell: R,
    /// Exact selection predicate.
    pub predicate: R,
    /// Exact coefficient stratum.
    pub region: R,
    /// Earlier count-one premise slot.
    pub count: u32,
    /// Independent finite-endpoint-definedness premise slots.
    pub definedness: Vec<u32>,
    /// Endpoint-order and non-root premise slots.
    pub order: Vec<u32>,
}
