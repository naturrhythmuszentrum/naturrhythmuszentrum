// SPDX-License-Identifier: AGPL-3.0-only
//! Closed symbolic identities and full-box derivative bounds are distinct premises.

mod check;
mod produce;
mod wire;

pub(super) use check::check;
pub(super) use produce::produce;
pub(super) use wire::{decode, encode};

use super::{AT, Interval};
use crate::graph::{Cas, LogicalId};
use crate::{Fault, InvalidCode};

/// The closed first-derivative identity at one original expression node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivativeRule {
    /// A rational literal has derivative zero.
    Constant,
    /// The original differentiation coordinate has derivative one.
    Coordinate,
    /// A different independent coordinate has derivative zero.
    Parameter,
    /// Negation preserves the ordered chain.
    Negate,
    /// Ordered sum rule.
    Sum,
    /// Ordered difference rule.
    Difference,
    /// Ordered product rule.
    Product,
    /// Quotient rule with a nonzero denominator over the complete box.
    Quotient,
    /// Exponential chain rule.
    Exponential,
    /// Logarithm chain rule on a positive input box.
    Logarithm,
    /// Logistic chain rule.
    Sigmoid,
    /// Hyperbolic tangent chain rule.
    Tanh,
    /// Positive-base power chain rule with the fixed original exponent.
    Power,
    /// Square-root chain rule requiring a strictly positive input box.
    Root,
}

/// A proposed symbolic identity and its separately checked full-box enclosure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivativeStep {
    /// Original operator identity reconstructed by the receiver.
    pub rule: DerivativeRule,
    /// Proposed derivative enclosure over the complete original box.
    pub bound: Interval,
}

/// Evidence for the derivative of one original subtree in its original coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivativeEvidence {
    /// Original derivative node's record index.
    pub record: u32,
    /// First record of its original operand subtree.
    pub first: u32,
    /// Original declared differentiation coordinate, never a caller callback.
    pub coordinate: String,
    /// Ordered symbolic identities and full-box bounds for every original operand node.
    pub steps: Vec<DerivativeStep>,
}

/// A view of original graph records, never a second expression tree.
#[derive(Clone, Copy)]
pub(super) struct Segment<'a> {
    /// Owning source graph.
    pub cas: &'a Cas,
    /// Independently constructed original postorder slice.
    pub schedule: &'a [LogicalId],
    /// Value enclosures at the same original indices.
    pub values: &'a [Interval],
}

/// One earlier original value and its derivative enclosure.
#[derive(Clone, Copy)]
struct Input<'a> {
    /// Original full-box value.
    value: &'a Interval,
    /// Previously reconstructed derivative bound.
    derivative: &'a Interval,
}

/// Bound every retained derivative identity and interval before copying.
pub(super) fn bytes(evidence: &[DerivativeEvidence]) -> u64 {
    evidence.iter().fold(0_u64, |total, item| {
        item.steps.iter().fold(
            total
                .saturating_add(64)
                .saturating_add(u64::try_from(item.coordinate.len()).unwrap_or(u64::MAX)),
            |sum, step| {
                sum.saturating_add(16)
                    .saturating_add(super::interval_bytes(&step.bound))
            },
        )
    })
}

/// A source identity, regularity premise or reconstructed bound failed.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
