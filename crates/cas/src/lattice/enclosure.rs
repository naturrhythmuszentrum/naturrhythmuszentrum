// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered original-target boxes reuse the closed scalar replay checker.

use crate::budget::{Budget, BudgetKind};
use crate::decision::RationalEnclosure;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};

/// Only the vector wrapper pays for its identities and returned ownership.
const AT: Location = Location::new("original rational box enclosure");

/// An untrusted closed rational box for one original explicit Real column.
///
/// Coordinate order is the original matrix operand order. Every scalar contract
/// must name that exact operand, this context and this context's actual region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalBoxEnclosure {
    /// Original nonempty Real column, including all unnormalized entry syntax.
    pub target: Handle,
    /// Original declaration and hypothesis context shared by every coordinate.
    pub context: Handle,
    /// The actual admitted region of the original context.
    pub input_family: Handle,
    /// Complete ordered coordinate boxes and their scalar replay origins.
    pub coordinates: Vec<RationalEnclosure>,
}

/// Sealed local evidence that the entire original vector lies in its rational box.
///
/// Every coordinate retains its exact original target and any Machin certificate.
/// This receipt creates no portable witness, membership hypothesis or selection.
#[derive(Clone, Debug)]
pub struct CheckedRationalBoxEnclosure {
    /// Complete checked vector identity and every original scalar certificate.
    contract: RationalBoxEnclosure,
    /// Conservative prepaid ownership, excluding shared immutable graph records.
    bytes: u64,
}

impl CheckedRationalBoxEnclosure {
    /// Borrow the complete original-target judgment and its ordered evidence.
    #[must_use]
    pub const fn contract(&self) -> &RationalBoxEnclosure {
        &self.contract
    }

    /// Conservative logical ownership retained by this local receipt.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Cas {
    /// Check one closed rational box against its exact original Real column.
    ///
    /// Each original coordinate is independently replayed by the existing scalar
    /// checker. All coordinates must succeed before any owned evidence is returned.
    /// New checked scalar judgments publish only with the complete returned vector.
    /// # Errors
    /// Refuses changed target order, context or family, malformed vector shapes,
    /// unsupported scalar origins, invalid original operands and exhausted resources.
    pub fn check_rational_box_enclosure(
        &mut self,
        contract: &RationalBoxEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedRationalBoxEnclosure, Fault> {
        self.with_enclosure_attempt(budget, |attempt, budget| {
            attempt.check_rational_box_enclosure(contract, budget)
        })
    }

    /// Prepay the complete result after validating every coordinate in place.
    pub(crate) fn rational_box_enclosure(
        &self,
        contract: &RationalBoxEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedRationalBoxEnclosure, Fault> {
        let bytes = validate(self, contract, budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
        budget.work(bytes, AT)?;
        Ok(CheckedRationalBoxEnclosure {
            contract: contract.clone(),
            bytes,
        })
    }
}

/// Share original-box validation with the later affine and selection checkers.
pub(super) fn validate(
    cas: &Cas,
    contract: &RationalBoxEnclosure,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    budget.work(4, AT)?;
    let context = cas.state.context(cas.checked(&contract.context)?)?;
    let target = cas.checked(&contract.target)?;
    if cas.checked(&contract.input_family)? != context.admissible {
        return Err(mismatch());
    }
    let (rows, columns) = cas.lattice_rectangle(target, context.header_id)?;
    if rows == 0 || columns != 1 || size(contract.coordinates.len(), AT)? != u64::from(rows) {
        return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
    }
    budget.check(BudgetKind::MatrixEntries, u64::from(rows), AT)?;
    let entries = &cas.state.expression(target)?.operands;
    let mut bytes = 256_u64;
    for (entry, coordinate) in entries.iter().zip(&contract.coordinates) {
        budget.work(3, AT)?;
        if cas.checked(&coordinate.target)? != *entry
            || coordinate.context != contract.context
            || coordinate.input_family != contract.input_family
        {
            return Err(mismatch());
        }
        let scalar = budget.with_retained_payload(bytes, AT, |budget| {
            crate::decision_enclosure::validate_closed(cas, coordinate, budget)
        })?;
        bytes = bytes.saturating_add(scalar);
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    }
    Ok(bytes)
}

/// A changed original vector or a scalar certificate from another frame cannot be reused.
const fn mismatch() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
