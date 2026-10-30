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
    /// Maximum magnitude of an executed analytic reduction exponent.
    pub reduction_ceiling: u32,
    /// Maximum included terms in each analytic series.
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
    /// A computed argument bound has not yet separated from the logarithm domain boundary.
    DomainBoundary,
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
    /// Finite analytic instances, present only for the logarithm family.
    pub logarithms: Vec<super::LogEvidence>,
    /// Finite exponential instances, absent from earlier family wire formats.
    pub exponentials: Vec<super::ExpEvidence>,
    /// Route and component bounds for original positive-power nodes.
    pub powers: Vec<super::PowerEvidence>,
}

/// Sealed evidence admitted only after the independent checker has completed.
#[derive(Clone, Debug)]
pub struct CheckedEnclosure {
    /// Whether independent replay encountered a checked root-grid instance.
    pub(super) roots: bool,
    /// Whether replay encountered a sigmoid composition premise.
    pub(super) sigmoids: bool,
    /// Whether independent replay encountered hyperbolic tangent.
    pub(super) tangents: bool,
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
            super::EnclosureLaw::PositivePower => {
                let index = usize::from(self.roots)
                    | (usize::from(!self.certificate.logarithms.is_empty()) << 1)
                    | (usize::from(!self.certificate.exponentials.is_empty()) << 2)
                    | (usize::from(self.sigmoids) << 3)
                    | (usize::from(self.tangents) << 4);
                let (schemas, count) = &POWER_SCHEMAS[index];
                &schemas[..*count]
            }
            super::EnclosureLaw::RationalInclusion => &super::ENCLOSURE_SCHEMAS[..1],
            super::EnclosureLaw::RootGrid => &super::ENCLOSURE_SCHEMAS[..2],
            super::EnclosureLaw::LogAtanh if self.roots => &super::ENCLOSURE_SCHEMAS[..3],
            super::EnclosureLaw::LogAtanh => &LOG_SCHEMAS,
            super::EnclosureLaw::ExpTaylor
                if self.roots && !self.certificate.logarithms.is_empty() =>
            {
                &super::ENCLOSURE_SCHEMAS[..4]
            }
            super::EnclosureLaw::ExpTaylor if self.roots => &EXP_ROOT_SCHEMAS,
            super::EnclosureLaw::ExpTaylor if !self.certificate.logarithms.is_empty() => {
                &EXP_LOG_SCHEMAS
            }
            super::EnclosureLaw::ExpTaylor => &EXP_SCHEMAS,
            super::EnclosureLaw::TanhCompose => {
                TANH_SCHEMAS[usize::from(self.roots)
                    | (usize::from(!self.certificate.logarithms.is_empty()) << 1)
                    | (usize::from(self.sigmoids) << 2)]
            }
            super::EnclosureLaw::SigmoidCompose => {
                match (self.roots, self.certificate.logarithms.is_empty()) {
                    (true, false) => &super::ENCLOSURE_SCHEMAS[..5],
                    (true, true) => &SIGMOID_ROOT_SCHEMAS,
                    (false, false) => &SIGMOID_LOG_SCHEMAS,
                    (false, true) => &SIGMOID_SCHEMAS,
                }
            }
        }
    }

    /// Formal premise kinds of the implemented checker, including original domain checks.
    #[must_use]
    pub const fn premise_kinds(&self) -> &'static [crate::proof::HypothesisKind] {
        use crate::proof::HypothesisKind;
        if matches!(
            self.certificate.law,
            super::EnclosureLaw::LogAtanh
                | super::EnclosureLaw::ExpTaylor
                | super::EnclosureLaw::SigmoidCompose
                | super::EnclosureLaw::TanhCompose
                | super::EnclosureLaw::PositivePower
        ) {
            return &[
                HypothesisKind::DomainGuard,
                HypothesisKind::Equality,
                HypothesisKind::NonZero,
                HypothesisKind::Enclosure,
                HypothesisKind::NonNegative,
                HypothesisKind::IntegerRange,
                HypothesisKind::StrictSign,
            ];
        }
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

/// The exact schema inventory for logarithms without a root operand.
const LOG_SCHEMAS: [crate::emptiness::LemmaSchema; 2] =
    [super::ENCLOSURE_SCHEMAS[0], super::ENCLOSURE_SCHEMAS[2]];

/// Rational and exponential laws without other analytic operands.
const EXP_SCHEMAS: [crate::emptiness::LemmaSchema; 2] =
    [super::ENCLOSURE_SCHEMAS[0], super::ENCLOSURE_SCHEMAS[3]];
/// Root operands add their independently checked law.
const EXP_ROOT_SCHEMAS: [crate::emptiness::LemmaSchema; 3] = [
    super::ENCLOSURE_SCHEMAS[0],
    super::ENCLOSURE_SCHEMAS[1],
    super::ENCLOSURE_SCHEMAS[3],
];
/// Logarithm operands add their independently checked law.
const EXP_LOG_SCHEMAS: [crate::emptiness::LemmaSchema; 3] = [
    super::ENCLOSURE_SCHEMAS[0],
    super::ENCLOSURE_SCHEMAS[2],
    super::ENCLOSURE_SCHEMAS[3],
];

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

/// Composition retains its actual exponential schema and rational premise law.
const SIGMOID_SCHEMAS: [crate::emptiness::LemmaSchema; 3] = [
    super::ENCLOSURE_SCHEMAS[0],
    super::ENCLOSURE_SCHEMAS[3],
    super::ENCLOSURE_SCHEMAS[4],
];
/// Root-bearing sigmoid transcripts retain their root law too.
const SIGMOID_ROOT_SCHEMAS: [crate::emptiness::LemmaSchema; 4] = [
    super::ENCLOSURE_SCHEMAS[0],
    super::ENCLOSURE_SCHEMAS[1],
    super::ENCLOSURE_SCHEMAS[3],
    super::ENCLOSURE_SCHEMAS[4],
];
/// Log-bearing sigmoid transcripts retain their logarithm law too.
const SIGMOID_LOG_SCHEMAS: [crate::emptiness::LemmaSchema; 4] = [
    super::ENCLOSURE_SCHEMAS[0],
    super::ENCLOSURE_SCHEMAS[2],
    super::ENCLOSURE_SCHEMAS[3],
    super::ENCLOSURE_SCHEMAS[4],
];

/// Exact inventories indexed by encountered root, logarithm and sigmoid premises.
const TANH_SCHEMAS: [&[crate::emptiness::LemmaSchema]; 8] = [
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[1],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[2],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[1],
        super::ENCLOSURE_SCHEMAS[2],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[4],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[1],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[4],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[2],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[4],
        super::ENCLOSURE_SCHEMAS[5],
    ],
    &[
        super::ENCLOSURE_SCHEMAS[0],
        super::ENCLOSURE_SCHEMAS[1],
        super::ENCLOSURE_SCHEMAS[2],
        super::ENCLOSURE_SCHEMAS[3],
        super::ENCLOSURE_SCHEMAS[4],
        super::ENCLOSURE_SCHEMAS[5],
    ],
];

/// Exact component inventories are compiled once, never allocated during inspection.
static POWER_SCHEMAS: [([crate::emptiness::LemmaSchema; 7], usize); 32] = [
    power_schemas(0),
    power_schemas(1),
    power_schemas(2),
    power_schemas(3),
    power_schemas(4),
    power_schemas(5),
    power_schemas(6),
    power_schemas(7),
    power_schemas(8),
    power_schemas(9),
    power_schemas(10),
    power_schemas(11),
    power_schemas(12),
    power_schemas(13),
    power_schemas(14),
    power_schemas(15),
    power_schemas(16),
    power_schemas(17),
    power_schemas(18),
    power_schemas(19),
    power_schemas(20),
    power_schemas(21),
    power_schemas(22),
    power_schemas(23),
    power_schemas(24),
    power_schemas(25),
    power_schemas(26),
    power_schemas(27),
    power_schemas(28),
    power_schemas(29),
    power_schemas(30),
    power_schemas(31),
];

/// Unused array slots are inaccessible through each inventory's exact count.
const fn power_schemas(mask: usize) -> ([crate::emptiness::LemmaSchema; 7], usize) {
    let mut schemas = [super::ENCLOSURE_SCHEMAS[0]; 7];
    let mut count = 1;
    let mut index = 1;
    while index < 6 {
        if mask & (1 << (index - 1)) != 0 {
            schemas[count] = super::ENCLOSURE_SCHEMAS[index];
            count += 1;
        }
        index += 1;
    }
    schemas[count] = super::ENCLOSURE_SCHEMAS[6];
    (schemas, count + 1)
}
