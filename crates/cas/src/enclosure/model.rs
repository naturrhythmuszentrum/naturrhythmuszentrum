// SPDX-License-Identifier: AGPL-3.0-only
//! Requests preserve the original mathematical scope independently of evaluation policy.

use super::Interval;
use crate::budget::BudgetUsage;
use crate::graph::Handle;
use crate::number::Rational;

/// One ordered coordinate restriction; this defines the queried box, not a global assumption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateBound {
    /// Original free real coordinate in the target's header.
    pub coordinate: Handle,
    /// Closed rational coordinate range.
    pub interval: Interval,
}

/// Exact predicate on the target, or a request only for a narrow bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnclosureGoal {
    /// Request a width; missing it never refutes a mathematical predicate.
    Width,
    /// Target is at most this exact threshold.
    AtMost(Rational),
    /// Target is strictly less than this exact threshold.
    LessThan(Rational),
    /// Target equals this exact value; overlap is not equality.
    Equal(Rational),
}

/// Original graph scope, ordered box and mathematical question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnclosureRequest {
    /// Original, unsimplified scalar expression.
    pub target: Handle,
    /// Exact context, including every retained assumption.
    pub context: Handle,
    /// Ordered, distinct free coordinates; empty for a closed request.
    pub coordinates: Vec<CoordinateBound>,
    /// Width or exact threshold question.
    pub goal: EnclosureGoal,
}

/// Explicit bounded schedule; existing store and caller limits remain authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinementPolicy {
    /// Supported policy is version one.
    pub version: u32,
    /// Requested absolute width is two to minus this exponent.
    pub precision: u32,
    /// Largest permitted working-grid exponent.
    pub grid_ceiling: u32,
    /// Maximum number of complete refinement attempts.
    pub rounds: u32,
    /// Maximum permitted reduction exponent for later analytic families.
    pub reduction_ceiling: u32,
    /// Maximum permitted series terms for later analytic families.
    pub term_ceiling: u32,
    /// Work reserved from generation for independent checking and publication.
    pub checking_work: u64,
}

/// A reason an available bound did not decide the requested predicate or width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndecidedReason {
    /// The target's checked range crosses the decision boundary.
    Straddle,
    /// No allowed refinement attains the requested width.
    WidthLimit,
    /// Work ended after an earlier complete checked bound.
    WorkExhausted,
}

/// Honest mathematical outcome, separate from achieved width and accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnclosureOutcome {
    /// Complete checked evidence proves the requested predicate.
    Proved,
    /// Complete checked evidence proves the predicate's negation.
    Refuted,
    /// Checked evidence is insufficient for the request.
    Undecided(UndecidedReason),
    /// A width-only request has a sufficiently narrow bound and asserts no predicate.
    Bound,
    /// Resource exhaustion before any complete checked bound.
    Exhausted,
    /// The original grammar or policy has no available checker.
    Unsupported,
    /// Cooperative interruption at a charged boundary, with no mathematical verdict.
    Interrupted,
}

/// Portable proposed evidence, never a checked receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnclosureCertificate {
    /// Closed law family; root transcripts also replay their rational operand steps.
    pub law: super::EnclosureLaw,
    /// Semantic version of the selected law.
    pub version: u32,
    /// Complete canonical original target, context, box and goal key.
    pub scope: Vec<u8>,
    /// Working grid used by every nonliteral operation.
    pub grid: u32,
    /// Final outward presentation grid, bound to the request's precision policy.
    pub presentation: u32,
    /// Child-before-parent bounds in original operand traversal order.
    pub records: Vec<Interval>,
}

/// Sealed evidence admitted only after the independent checker has completed.
#[derive(Clone, Debug)]
pub struct CheckedEnclosure {
    /// Authenticated original request, including ordered coordinates.
    pub(super) request: EnclosureRequest,
    /// Independently replayed endpoint transcript.
    pub(super) certificate: EnclosureCertificate,
    /// Final checked outward presentation.
    pub(super) interval: Interval,
}

impl CheckedEnclosure {
    /// Original request whose complete identity qualifies this result.
    #[must_use]
    pub const fn request(&self) -> &EnclosureRequest {
        &self.request
    }

    /// Independently checked outward bound.
    #[must_use]
    pub const fn interval(&self) -> &Interval {
        &self.interval
    }

    /// Portable proposed evidence for independent checking in a receiving store.
    #[must_use]
    pub const fn certificate(&self) -> &EnclosureCertificate {
        &self.certificate
    }

    /// Registered mathematical law used by this instance.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::emptiness::LemmaSchema {
        self.certificate.law.formal_schema()
    }

    /// Complete schema inventory used by this transcript; roots compose rational inclusion.
    #[must_use]
    pub fn formal_schemas(&self) -> &'static [crate::emptiness::LemmaSchema] {
        match self.certificate.law {
            super::EnclosureLaw::RationalInclusion => &super::ENCLOSURE_SCHEMAS[..1],
            super::EnclosureLaw::RootGrid => &super::ENCLOSURE_SCHEMAS,
        }
    }

    /// Formal premise kinds of the implemented checker, including original domain checks.
    #[must_use]
    pub const fn premise_kinds(&self) -> &'static [crate::proof::HypothesisKind] {
        use crate::proof::HypothesisKind;
        if matches!(self.certificate.law, super::EnclosureLaw::RootGrid) {
            return &[
                HypothesisKind::DomainGuard,
                HypothesisKind::Equality,
                HypothesisKind::NonZero,
                HypothesisKind::Enclosure,
                HypothesisKind::NonNegative,
                HypothesisKind::IntegerRange,
            ];
        }
        &[
            HypothesisKind::DomainGuard,
            HypothesisKind::Equality,
            HypothesisKind::NonZero,
            HypothesisKind::Enclosure,
        ]
    }
}

/// Result with actual Work and achieved precision; no stopping reason supplies evidence.
#[derive(Debug)]
pub struct EnclosureReport {
    /// Mathematical verdict or explicit absence of one.
    pub outcome: EnclosureOutcome,
    /// Last completely checked bound, if any.
    pub checked: Option<CheckedEnclosure>,
    /// Whether the checked width meets the requested precision.
    pub precision_met: bool,
    /// Number of completed checked rounds in this call.
    pub completed_rounds: u32,
    /// Work consumed during this call, including unsuccessful attempts.
    pub work: u64,
    /// Cumulative caller accounting after this call.
    pub usage: BudgetUsage,
    /// Exact resource refusal, when a ceiling stopped evaluation; never a verdict.
    pub resource_fault: Option<crate::Fault>,
}
