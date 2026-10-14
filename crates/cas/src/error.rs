// SPDX-License-Identifier: AGPL-3.0-only
//! Structured failures without expression formatting or panic conversion.

use std::fmt;

use crate::budget::BudgetKind;

/// A stable operation location, optionally naming a logical graph object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    /// Static operation name; never contains caller-owned source text.
    pub operation: &'static str,
    /// Logical identifier when an operation concerns a graph object.
    pub logical_id: Option<u64>,
}

impl Location {
    /// Name an operation without a graph object.
    #[must_use]
    pub const fn new(operation: &'static str) -> Self {
        Self {
            operation,
            logical_id: None,
        }
    }
}

/// The reason a value cannot be admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCode {
    /// The exact requested cover cannot be decided in the supported fragment.
    UnknownTotality,
    /// A valid operation still requires an explicit supported premise.
    PendingPremise,
    /// Applicable native associations disagree on a proved nonempty region.
    OverlapConflict,
    /// A requested finite cover omits a proved point or interval.
    CoverageHole,
    /// A numeric spelling does not follow the exact literal grammar.
    Literal,
    /// A rational denominator or divisor is zero.
    ZeroDenominator,
    /// Strict power semantics leave this power undefined.
    UndefinedPower,
    /// A span lies outside its source or splits a UTF-8 scalar.
    Span,
    /// A handle belongs to a different store.
    ForeignHandle,
    /// A reference is missing, forward, or of the wrong vertex kind.
    Reference,
    /// Bound-variable frames or free declarations do not agree.
    Scope,
    /// Mathematical sorts or operator arities do not agree.
    Sort,
    /// Context declarations or assumptions are inconsistent.
    Context,
    /// A policy or collection of limits is invalid.
    Policy,
    /// A source key was reused with different bytes.
    SourceConflict,
    /// An occurrence is reused by different original parents.
    OccurrenceParent,
    /// A receipt does not certify this root.
    Receipt,
    /// An imported representation is not canonical.
    Encoding,
    /// A primitive schema or an ordered premise does not match.
    Rule,
    /// Replay proved a different theorem than the caller requested.
    EndpointMismatch,
    /// Declared residuals or hypotheses differ from checked conditions.
    ConditionMismatch,
    /// The wire format or exact primitive semantic version is unknown.
    UnsupportedVersion,
    /// A valid mathematical representation is outside this executable checker.
    UnsupportedFragment,
    /// The input does not denote the admitted univariate polynomial fragment.
    PolynomialFragment,
    /// The coefficient field is not exactly rational.
    CoefficientField,
    /// The zero polynomial has no finite square-free multiplicity decomposition.
    ZeroPolynomial,
    /// A required domain condition was refuted, so no equality is admitted.
    RefutedDomain,
    /// A requested function result has an exactly empty admissible domain.
    EmptyDomain,
}

/// Classification of a resource refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exhaustion {
    /// An ordinary service counter or size was exceeded.
    Limit,
    /// A rational component or bigint intermediate exceeded its bit budget.
    ArithmeticRange,
    /// A fallible reservation could not allocate memory.
    Allocation,
    /// A machine index or checked size calculation could not be represented.
    Capacity,
}

/// An invalid value or a resource refusal, never a mathematical inequality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultKind {
    /// A malformed or inconsistent input.
    Invalid(InvalidCode),
    /// Work stopped before exceeding a resource ceiling.
    Exhausted {
        /// Resource-refusal classification.
        reason: Exhaustion,
        /// The named ceiling involved.
        budget: BudgetKind,
        /// The permitted quantity.
        limit: u64,
        /// The requested quantity, or `u64::MAX` when it could not be represented.
        needed: u64,
    },
}

/// A small structured error that never embeds an expanded expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fault {
    /// Failure classification and bounded metadata.
    pub kind: FaultKind,
    /// The operation that refused the value.
    pub location: Location,
}

impl Fault {
    /// Construct an invalid-input error.
    #[must_use]
    pub const fn invalid(code: InvalidCode, location: Location) -> Self {
        Self {
            kind: FaultKind::Invalid(code),
            location,
        }
    }

    /// Construct a budget refusal, classifying bit ceilings as arithmetic range.
    #[must_use]
    pub const fn exhausted(
        budget: BudgetKind,
        limit: u64,
        needed: u64,
        location: Location,
    ) -> Self {
        let reason = match budget {
            BudgetKind::ComponentBits | BudgetKind::IntermediateBits => Exhaustion::ArithmeticRange,
            _ => Exhaustion::Limit,
        };
        Self {
            kind: FaultKind::Exhausted {
                reason,
                budget,
                limit,
                needed,
            },
            location,
        }
    }

    /// Construct a checked host-capacity refusal.
    #[must_use]
    pub const fn capacity(budget: BudgetKind, location: Location) -> Self {
        Self {
            kind: FaultKind::Exhausted {
                reason: Exhaustion::Capacity,
                budget,
                limit: u64::MAX,
                needed: u64::MAX,
            },
            location,
        }
    }

    /// Construct a fallible allocation refusal.
    #[must_use]
    pub const fn allocation(location: Location) -> Self {
        Self {
            kind: FaultKind::Exhausted {
                reason: Exhaustion::Allocation,
                budget: BudgetKind::PayloadBytes,
                limit: 0,
                needed: 0,
            },
            location,
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location.operation)
    }
}

impl std::error::Error for Fault {}
