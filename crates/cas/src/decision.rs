// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational cuts and reusable finite decision partitions.
//!
//! These checks establish cut and partition premises. They admit no enclosure origin,
//! selected value, proof record or certified-membership hypothesis.
//! A consumer must separately establish the target enclosure and complete decision
//! contract before using this calculation in a checked introduction rule.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::number::Rational;

/// An untrusted closed rational interval for one exact declared Real coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateInterval {
    /// The original closed Real free expression under the case's exact header.
    pub coordinate: Handle,
    /// Included lower endpoint; no floating conversion is performed.
    pub lower: Rational,
    /// Included upper endpoint; must be at least the lower endpoint.
    pub upper: Rational,
}

/// An explicit integer option range attached to one cut question.
///
/// The index is the association's exact Real rational literal, checked integral. This range
/// is pure declaration data, not an `Integer` case result, admitted partition or physical count.
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
    /// Nonempty box over the ambient Real coordinates, without duplicates or reversed endpoints.
    pub bounds: Vec<CoordinateInterval>,
    /// Optional declared integer range and half-open cell requirement.
    pub integer: Option<IntegerCellRange>,
}

impl Cas {
    /// Check exact inclusion of a nonempty rational box in a declared case option.
    ///
    /// The closed grammar supports rational affine scalar expressions, strict
    /// comparisons, equality, nonzero, constant integer ranges and total Boolean
    /// region constructors. Both the case domain and selected region must contain
    /// the whole box. An integer question also checks its declared range and
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
