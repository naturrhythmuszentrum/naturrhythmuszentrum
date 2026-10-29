// SPDX-License-Identifier: AGPL-3.0-only
//! Exact decision prerequisites and checked local membership introductions.
//!
//! Standalone cut, partition and closed rational enclosure checks return caller-retained values.
//! `DecisionIntroduction` combines their exact contracts with whole-family coordinate equalities
//! in an auxiliary checked proof; `ContextDraft::Certified` retains that proof in a local frame.
//! Selection and local definedness remain separate checked obligations.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::number::Rational;

/// An untrusted closed rational interval for one exact declared Real or Integer coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateInterval {
    /// The original closed Real or Integer free expression under the case's exact header.
    pub coordinate: Handle,
    /// Included lower endpoint; Integer coordinates require an exact integer.
    pub lower: Rational,
    /// Included upper endpoint; ordered and integral for Integer coordinates.
    pub upper: Rational,
}

/// An explicit integer option range attached to one cut question.
///
/// A cut uses the association's exact `Integer`-sorted rational literal; a family
/// uses its explicit Integer binder. Integral Real options refuse. The range alone
/// admits neither a partition nor a physical count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerCellRange {
    /// Coordinate whose box must lie in the selected integer's half-open cell.
    pub coordinate: Handle,
    /// Included integer lower bound; absence means no lower bound.
    pub lower: Option<Rational>,
    /// Excluded integer upper bound; absence means no upper bound.
    pub upper: Option<Rational>,
}

/// An untrusted inclusion question retaining an actual native association identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseCut {
    /// Earlier admitted finite native case expression.
    pub function: Handle,
    /// Exact case-arm handle, which must occur in the function's arm inventory.
    pub association: Handle,
    /// Nonempty box exhausting the Real and Integer ambient coordinates, without duplicates.
    pub bounds: Vec<CoordinateInterval>,
    /// Optional declared integer range and half-open cell requirement.
    pub integer: Option<IntegerCellRange>,
}

impl Cas {
    /// Check exact inclusion of a nonempty rational box in a declared case option.
    ///
    /// The closed grammar supports rational affine scalar expressions, strict
    /// comparisons, equality, nonzero, affine integer ranges, explicit Integer
    /// embeddings and total Boolean region constructors. Integer axes have integral
    /// endpoints, so every accepted box has at least one assignment. Both the case
    /// domain and selected region must contain the whole box. An integer question
    /// also checks its declared range and
    /// `n <= lower <= upper < n + 1`. No graph or hypothesis is published.
    ///
    /// # Errors
    /// Refuses foreign or mismatched identities, malformed boxes, invalid integer
    /// ranges, boundary intersections and boxes wholly outside a required region.
    /// Unsupported predicates, placements, non-rational cuts and origin arguments
    /// have no authority here and return `UnsupportedCertificate`. Work remains
    /// consumed on refusal under the caller's cumulative ledger.
    pub fn check_case_cut(&self, cut: &CaseCut, budget: &mut Budget) -> Result<(), Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = crate::decision_cut::check(self, cut, budget);
        budget.restore_limits(previous);
        result
    }
}

/// An untrusted finite partition question over an actual native case revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasePartition {
    /// Exact native case, including its domain and complete association inventory.
    pub function: Handle,
    /// Distinct closed Real coordinates exhausting the domain's ambient axes.
    pub coordinates: Vec<Handle>,
    /// Optional finite integer family; both endpoints must be present and integral.
    pub integer: Option<IntegerCellRange>,
}

/// A sealed, reusable check of one immutable case partition and optional integer family.
///
/// This caller-owned value retains its exact input contract, not a membership fact.
/// Reuse checks store and case identity without reconstructing the common partition.
/// It has no wire representation and cannot be used as a proof or cover witness.
#[derive(Clone, Debug)]
pub struct CheckedCasePartition {
    /// Exact immutable case revision, ordered axes and optional finite integer range.
    pub(crate) contract: CasePartition,
    /// Prepaid conservative ownership bound for this returned contract.
    pub(crate) bytes: u64,
}

impl CheckedCasePartition {
    /// Borrow the exact contract whose complete partition was checked.
    #[must_use]
    pub const fn contract(&self) -> &CasePartition {
        &self.contract
    }

    /// Conservative logical bytes owned by this returned value, excluding shared graph syntax.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Cas {
    /// Check total predicates, nonempty domain, pairwise disjointness and exact cover once.
    ///
    /// Supported atoms are rational affine comparisons involving at most one axis
    /// after exact cancellation. Their common Cartesian cells retain every singleton
    /// boundary. An integer family additionally matches each literal option to its
    /// exact half-open cell and the case domain to the declared finite range.
    /// # Errors
    /// Refuses missing members, overlaps, options outside the domain, empty domains,
    /// mismatched identities, malformed integer families and unsupported predicates.
    /// Refusal publishes no graph state or returned check; spent Work remains spent.
    pub fn check_case_partition(
        &self,
        contract: &CasePartition,
        budget: &mut Budget,
    ) -> Result<CheckedCasePartition, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = crate::decision_cut::partition::check(self, contract, budget);
        budget.restore_limits(previous);
        result
    }

    /// Check a cut against an already checked, identical partition contract.
    ///
    /// Exact case, ordered coordinates and integer-range identities must agree.
    /// The sealed partition avoids repeating cover and disjointness checks; the cut
    /// itself is checked anew on the same cumulative ledger. No target enclosure,
    /// certified membership, local frame or selected value is introduced.
    /// # Errors
    /// Refuses foreign checks, changed contracts and every refusal of `check_case_cut`.
    pub fn check_partition_cut(
        &self,
        partition: &CheckedCasePartition,
        cut: &CaseCut,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = crate::decision_cut::partition::reuse(self, partition, cut, budget);
        budget.restore_limits(previous);
        result
    }
}

/// A closed replay origin; numerical kernel and unverified claims have no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnclosureOrigin {
    /// Replay every original operand through the existing exact-constant checker.
    ExactRational {
        /// Exact expression being replayed; must be identical to the requested target.
        expression: Handle,
        /// Untrusted rational result, checked against exact CAS normalization.
        value: Rational,
    },
    /// Closed interval replay with an independently checked finite Machin expansion.
    PiBound {
        /// Exact original expression, identical to the requested target.
        expression: Handle,
        /// Untrusted constant theorem instance, retained and rechecked on every replay.
        certificate: Box<crate::pi_bounds::PiBoundCertificate>,
    },
    /// Scalar numerical certificates require a separate, presently unsupported checker.
    ScalarKernel,
    /// Matrix numerical certificates require a separate, presently unsupported checker.
    MatrixKernel,
    /// A provenance label, success flag or finite radius alone establishes no enclosure.
    Unverified,
}

/// An untrusted rational enclosure of an exact target in its original context.
///
/// Replay supports exact rational results and closed pi arithmetic under an explicit
/// checked Machin certificate. Its bound holds throughout the stated family without
/// using an input assumption. Free-input interval arguments remain unsupported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalEnclosure {
    /// Exact original expression, including operands that normalization could erase.
    pub target: Handle,
    /// Original context, retained without closing or discharging its hypotheses.
    pub context: Handle,
    /// Exact admitted region of that context; a substituted or widened region refuses.
    pub input_family: Handle,
    /// Included lower rational endpoint.
    pub lower: Rational,
    /// Included upper rational endpoint.
    pub upper: Rational,
    /// Original-expression replay and its untrusted candidate result.
    pub origin: EnclosureOrigin,
}

/// Sealed caller-owned evidence for one closed rational enclosure contract.
///
/// This has no proof, wire, hypothesis or case-selection authority. The context
/// remains part of its exact contract; no consistency or nonemptiness is inferred.
#[derive(Clone, Debug)]
pub struct CheckedRationalEnclosure {
    /// Complete checked target, context, family, box and replay identity.
    pub(crate) contract: RationalEnclosure,
    /// Conservative prepaid returned ownership, excluding shared immutable graph syntax.
    pub(crate) bytes: u64,
}

impl CheckedRationalEnclosure {
    /// The constant theorem retained by this checked enclosure, when pi replay was used.
    #[must_use]
    pub const fn constant_schema(&self) -> Option<&'static crate::proof::LemmaSchema> {
        match &self.contract.origin {
            EnclosureOrigin::PiBound { .. } => Some(&crate::proof::CONSTANT_SCHEMAS[0]),
            _ => None,
        }
    }

    /// Borrow the exact original contract whose replay and rational bounds were checked.
    #[must_use]
    pub const fn contract(&self) -> &RationalEnclosure {
        &self.contract
    }

    /// Conservative logical bytes retained by this caller-owned check.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Cas {
    /// Replay a closed original expression and check its exact result against a rational box.
    ///
    /// Existing exact-constant rules check finiteness before normalization and
    /// independently check the normalized equality. `ExactRational` requires an
    /// exactly rational result; `PiBound` additionally checks a finite Machin theorem
    /// and propagates outward rational intervals through the closed expression.
    /// All original operands are checked even when cancellation could hide them.
    /// # Errors
    /// Refuses foreign identities, changed targets or input families, false results,
    /// reversed or excluding boxes, unsupported origins or constants, free inputs,
    /// invalid pi certificates and unproved divisor domains. No caller graph state changes.
    pub fn check_rational_enclosure(
        &self,
        contract: &RationalEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedRationalEnclosure, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = crate::decision_enclosure::check(self, contract, budget);
        budget.restore_limits(previous);
        result
    }
}

/// Replayable enclosure-to-membership evidence, consumed by `Primitive::RegionDecide`.
///
/// Ordered proof premises establish each coordinate equal to the corresponding target
/// throughout the exact source context's admitted family. No input is trusted by name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionIntroduction {
    /// Complete finite native partition and its exact coordinate inventory.
    pub partition: CasePartition,
    /// Exact selected association and rational coordinate box.
    pub cut: CaseCut,
    /// Original target replay for each coordinate, in the identical order.
    pub enclosures: Vec<RationalEnclosure>,
}

/// A projected floor-family proposition over an explicit native `(q,n)` frame.
///
/// For every assignment in `domain` with the `index` coordinate omitted, exactly
/// one Integer index belongs to the declared range and the native case relation.
/// Bounds are exact Integer expressions; absence is the corresponding unbounded
/// constructor. Nonliteral Integer-affine endpoints require ordered exact binding
/// proofs on the whole source domain; their used hypotheses remain visible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerFamily {
    /// Finite native Integer case whose sole association returns `index`.
    pub function: Handle,
    /// Declared Real coordinate projected by the floor relation.
    pub coordinate: Handle,
    /// Declared Integer coordinate explicitly bound by this proposition.
    pub index: Handle,
    /// Exact projected input domain, independent of the bound index.
    pub domain: Handle,
    /// Included Integer endpoint, or no lower bound.
    pub lower: Option<Handle>,
    /// Excluded Integer endpoint, or no upper bound.
    pub upper: Option<Handle>,
    /// Optional declared count domain intersected with the endpoint range.
    pub count: Option<Box<IntegerCellRange>>,
}

/// Exact unique Integer projection selected under its retained source correspondence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerSelection {
    /// Original finite family and its complete declared range.
    pub family: IntegerFamily,
    /// Exact Integer literal selected by the checked enclosure and family theorem.
    pub value: Handle,
}

/// Closed auxiliary inference register; no ordinary cost-policy entry is added.
pub static DECISION_SCHEMAS: [crate::proof::LemmaSchema; 9] = [
    crate::proof::LemmaSchema {
        name: "decision.enclosure-membership",
        label: "thm:cas:decision-enclosure-membership",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "Exact target enclosures and box inclusion establish membership in the declared option.",
        premises: "Complete partition, exact original-target replay, cut and ordered coordinate-to-target equalities on the whole source family, retaining every used hypothesis.",
    },
    crate::proof::LemmaSchema {
        name: "decision.membership-nonzero",
        label: "thm:cas:decision-membership-nonzero",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "Certified strict positive or negative membership makes the identical free coordinate defined and nonzero.",
        premises: "Exact certified membership slot and its checked local membership premise, strict selected guard and unchanged subject, retaining every source hypothesis.",
    },
    crate::proof::LemmaSchema {
        name: "decision.nonzero-discharge",
        label: "thm:cas:decision-nonzero-discharge",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A checked certified sign consequence discharges one original nonzero requirement without changing the local equality or its retained requirements.",
        premises: "Original conditional defined-and-equal statement with exactly one pointwise nonzero residual and a checked decision.membership-nonzero fact on the identical context, region and predicate, retaining every source hypothesis.",
    },
    crate::proof::LemmaSchema {
        name: "decision.integer-floor-family",
        label: "thm:cas:decision-integer-floor-family",
        version: 2,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "Every admitted Real coordinate has exactly one Integer index in the declared floor relation and range.",
        premises: "Exact finite native case on the explicit mixed frame, Integer result identical to its bound index, complete half-open floor relation, nonempty exact range intersected with the declared count domain, identical projected input domain and exact endpoint binding proofs retaining every used hypothesis.",
    },
    crate::proof::LemmaSchema {
        name: "decision.integer-family-selection",
        label: "thm:cas:decision-integer-family-selection",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The unique projected result of the declared floor family is the selected exact Integer.",
        premises: "Checked identical floor-family theorem, whole-domain coordinate-to-target equality, exact original-target enclosure wholly inside the selected half-open cell and checked index-range membership, retaining every used hypothesis.",
    },
    crate::proof::LemmaSchema {
        name: "decision.lattice-floor-cover",
        label: "thm:cas:decision-lattice-floor-cover",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "Every point in the exact affine projected domain has exactly one ordered Integer tuple in the declared half-open product range.",
        premises: "Original homogeneous basis and offset, checked nonzero determinant and both inverse identities, followed by one checked scalar floor family per distinct coordinate under the common independent parameter frame, with exact projected domains and all binding hypotheses retained.",
    },
    crate::proof::LemmaSchema {
        name: "decision.lattice-floor-selection",
        label: "thm:cas:decision-lattice-floor-selection",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The original lattice target belongs to the unique half-open cell indexed by the exact ordered Integer tuple.",
        premises: "Identical checked affine cover, complete original-target affine enclosure, exact tuple membership in every declared index range and strict whole-box floor cuts, retaining all qualified cover hypotheses.",
    },
    crate::proof::LemmaSchema {
        name: "decision.lattice-orthogonal-nearest-selection",
        label: "thm:cas:decision-lattice-orthogonal-nearest-selection",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The original target belongs to the nearest lattice cell of the exact ordered Integer tuple, with simultaneous ties owned by the greatest original index.",
        premises: "Identical checked affine basis and range evidence, exact positive diagonal Gram matrix, complete original-target enclosure and whole-box shifted floor cuts, retaining all qualified hypotheses.",
    },
    crate::proof::LemmaSchema {
        name: "decision.lattice-reduced-nearest-selection",
        label: "thm:cas:decision-lattice-reduced-nearest-selection",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The original target belongs to the nearest lattice cell of the exact original Integer tuple after a checked unimodular orthogonal reduction, with original-index tie ownership.",
        premises: "Identical checked affine basis and range evidence, integral two-sided inverse matrices U and W, positive diagonal Gram matrix of B*U, original k = U*m, complete original-target enclosure transported through W, and whole-box nearest cuts oriented by the first nonzero original entry of each U column.",
    },
];
