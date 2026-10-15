// SPDX-License-Identifier: AGPL-3.0-only
//! Pairing refusals identify the target side without pretending to know heap indices.

use crate::UnitError;

/// Ordered target side of a covariance or sensitivity relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingSide {
    /// Primary uncertainty target or sensitivity output.
    First,
    /// Other covariance target or sensitivity input.
    Second,
}

/// A layout-free descriptor relationship could not be established.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairingError {
    /// Existing scalar descriptor admission or exact arithmetic refused the relation.
    #[error(transparent)]
    Units(#[from] UnitError),
    /// A contraction requires a formal sensitivity on each map side.
    #[error("pairing contraction requires a sensitivity entry")]
    ExpectedSensitivity,
    /// Only variance and covariance may supply a paired statistical parameter.
    #[error("pairing contraction requires variance or covariance")]
    ExpectedCovariance,
    /// A statistical standard uncertainty cannot supply a deterministic radius.
    #[error("pairing contraction requires an absolute deterministic radius")]
    ExpectedRadius,
    /// A side's exact schema, including coordinates and role, differs from its target.
    #[error("pairing target schema mismatch on the {side:?} side")]
    TargetMismatch {
        /// Side whose captured schema did not match.
        side: PairingSide,
    },
    /// Relative coordinates refer to one explicit reference declaration.
    #[error("pairing named reference mismatch")]
    ReferenceMismatch,
    /// Numerical evaluation error cannot be retargeted to another parameter or value.
    #[error("evaluation radius belongs to a different parameter pairing")]
    EvaluationTargetMismatch,
}
