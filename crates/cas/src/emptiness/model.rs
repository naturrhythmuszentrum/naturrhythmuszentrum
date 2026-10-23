// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted emptiness evidence names exact regions and explicit earlier mathematical facts.

use crate::graph::Handle;

/// A child-before-parent proof of emptiness within one application context and premise frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmptinessDag {
    /// Reachable nodes in dependency order; the final node is the asserted empty region.
    pub nodes: Vec<EmptyNode>,
}

/// One exact subproblem; declarations and relation syntax are retained by its region and context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmptyNode {
    /// Region whose points are excluded, without widening its original stratum.
    pub region: Handle,
    /// Explicit evidence, never a producer's success flag.
    pub evidence: EmptyEvidence,
}

/// Structural covers, independently checked local contradictions and exact lemma citations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmptyEvidence {
    /// An earlier checked cover and one earlier empty node per ordered cover member.
    Split {
        /// Local application premise containing the exact cover.
        cover: u32,
        /// Earlier node indices in precisely the cover's retained member order.
        children: Vec<u32>,
    },
    /// A local algebraic or sign contradiction.
    Leaf(Box<EmptyLeaf>),
    /// An earlier checked empty-region theorem in the identical application context.
    Lemma {
        /// Local application premise, including its complete retained conditions.
        premise: u32,
    },
}

/// The only new leaves are zero contradictions from exact nonvanishing and full factor identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmptyLeaf {
    /// The region asserts zero of an expression proved nonzero or strictly signed there.
    Sign {
        /// Exact zero subject retained by a true equality guard of the region.
        subject: Handle,
        /// Checked nonzero or strict sign fact for precisely this expression.
        nonzero: EmptyFact,
    },
    /// A checked equality reconstructs a nonzero scalar times every nonzero factor.
    FactorIdentity(Box<EmptyFactorIdentity>),
}

/// Nonvanishing comes from a checked premise or a literal guard of an enclosing region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmptyFact {
    /// Earlier checked regional predicate or ordered-real strict sign.
    Premise(u32),
    /// Exact ancestor region whose constructor asserts a true positive/nonzero guard.
    Guard(Handle),
}

/// Full factor reconstruction excludes zero without a new polynomial search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmptyFactorIdentity {
    /// Exact expression asserted zero by this subproblem's region.
    pub subject: Handle,
    /// Exact displayed product in the independently checked equality premise.
    pub product: Handle,
    /// Leftmost scalar of the displayed product.
    pub scalar: Handle,
    /// Nonvanishing evidence for that scalar.
    pub scalar_nonzero: EmptyFact,
    /// Ordered factors and their positive multiplicities; none may be omitted.
    pub factors: Vec<EmptyFactor>,
    /// Local premise proving the complete defined equality of subject and product.
    pub identity: u32,
}

/// One exact factor occurrence with an explicitly positive multiplicity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmptyFactor {
    /// Original expression used as the displayed factor.
    pub subject: Handle,
    /// Positive power, encoded and decoded at the same width.
    pub multiplicity: u32,
    /// Exact nonvanishing fact for this factor on the subproblem region.
    pub nonzero: EmptyFact,
}
