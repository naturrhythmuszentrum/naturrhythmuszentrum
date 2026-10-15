// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted arithmetic decisions and explicit finite evaluation scopes.

use super::fraction::Fraction;
use super::polynomial::Polynomial;
use super::scalar::Scalar;
use crate::error::Fault;
use crate::graph::Handle;
use crate::proof_model::VerifiedWitness;
use crate::solving::SolveReason;

/// Scratch values have an exact graph-owned domain, but no theorem authority.
#[derive(Debug)]
pub(crate) struct Element {
    /// Exact source algebra.
    pub(crate) domain: Handle,
    /// Canonical arithmetic representation.
    pub(crate) value: Value,
    /// The original graph value, when this value was imported without arithmetic.
    pub(crate) origin: Option<Handle>,
}

/// Ground values and canonical rational functions.
#[derive(Debug)]
pub(crate) enum Value {
    /// A rational or Gaussian rational.
    Scalar(Scalar),
    /// A rational function in independent formal parameters.
    Fraction(Fraction),
}

/// A complete scoped evaluation environment; premises remain sealed graph witnesses.
#[derive(Debug)]
pub(crate) struct EvaluationScope<'a> {
    /// Full hypothesis context.
    pub(crate) context: &'a Handle,
    /// Exact coefficient region.
    pub(crate) region: &'a Handle,
    /// Available independently checked evidence.
    pub(crate) premises: &'a [VerifiedWitness],
    /// Finite localization, including denominators used before cancellation.
    pub(crate) localization: &'a [Handle],
}

/// A scratch nonvanishing obligation, to be materialized as a guarded graph predicate.
#[derive(Debug)]
pub(crate) struct GuardedObligation {
    /// Exact coefficient algebra, including the ordered parameter frame.
    pub(crate) domain: Handle,
    /// Original finite localization retained before any arithmetic cancellation.
    pub(crate) localization: Vec<Handle>,
    /// Full hypothesis context; display names never identify assumptions.
    pub(crate) context: Handle,
    /// Region on which the polynomial must be defined and nonzero.
    pub(crate) region: Handle,
    /// Canonical formal polynomial whose regional nonvanishing remains unproved.
    pub(crate) polynomial: Polynomial,
}

/// Arithmetic evidence is intentionally unsealed and requires independent replay.
#[derive(Debug)]
pub(crate) enum CoefficientCertificate {
    /// A ground-field calculation with no parameter-dependent denominator.
    Ground,
    /// A formal zero, on a localization whose denominators have been checked.
    FormalZero,
    /// An exact existing scoped unit theorem was used.
    Premise(Handle),
    /// Scoped nonzero premises supporting a regional arithmetic decision.
    Regional(Vec<Handle>),
}

/// A zero decision never silently upgrades a nonzero formal numerator to a regional unit.
#[derive(Debug)]
pub(crate) enum ZeroDecision {
    /// Exactly zero throughout its defined scope.
    Zero(CoefficientCertificate),
    /// Independently supported regional nonvanishing.
    Nonzero(CoefficientCertificate),
    /// A polynomial nonvanishing obligation remains.
    Undetermined(GuardedObligation),
}

/// Unit inversion distinguishes mathematical zero and pending nonvanishing from faults.
#[derive(Debug)]
pub(crate) enum UnitDecision<E> {
    /// A proposed inverse, with unsealed arithmetic evidence.
    Unit {
        /// Canonical proposed multiplicative inverse.
        inverse: E,
        /// Unsealed arithmetic evidence for independent replay.
        certificate: CoefficientCertificate,
    },
    /// Zero is not invertible.
    Zero(CoefficientCertificate),
    /// No regional nonvanishing evidence is available.
    Undetermined(GuardedObligation),
}

/// Resource/invalid faults and unsupported algebra features remain distinct.
#[derive(Debug)]
pub(crate) enum CoefficientFailure {
    /// Invalid input, capacity or ledger exhaustion.
    Fault(Fault),
    /// An unavailable algebra capability, never a false mathematical assertion.
    Unsupported(SolveReason),
}

impl From<Fault> for CoefficientFailure {
    fn from(value: Fault) -> Self {
        Self::Fault(value)
    }
}

impl std::fmt::Display for CoefficientFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CoefficientFailure {}
