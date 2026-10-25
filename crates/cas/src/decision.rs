// SPDX-License-Identifier: AGPL-3.0-only
//! Exact box inclusion in an existing native case association.
//!
//! These checks establish only the cut premise. They admit no enclosure origin,
//! partition, selected value, proof record or certified-membership hypothesis.
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
