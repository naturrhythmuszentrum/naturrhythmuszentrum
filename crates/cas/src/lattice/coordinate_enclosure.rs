// SPDX-License-Identifier: AGPL-3.0-only
//! Direct original-affine-expression replay has a distinct contract from whole-box transport.

use super::{CheckedLatticeBasis, LatticeBasisQuestion, RationalBoxEnclosure};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::Operator;

/// Direct basis-coordinate replay owns only its additional correspondence checks.
const AT: Location = Location::new("lattice coordinate box enclosure");

/// A direct rational box for the original exact affine expression `A*(q-phi)`.
///
/// Each coordinate expression is the ordered left fold starting at the inverse
/// translation entry and adding every `inverse[row,column] * q[column]` in order.
/// No coefficient or original operand, including zero products, may be omitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeCoordinateEnclosure {
    /// Exact admitted basis, offset and context.
    pub basis: LatticeBasisQuestion,
    /// Exact original nonempty Real column q.
    pub target: Handle,
    /// Exact admitted homogeneous inverse.
    pub inverse: Handle,
    /// The explicit affine-coordinate column and its ordered original-expression boxes.
    pub coordinates: RationalBoxEnclosure,
}

/// Sealed local evidence for a direct original-affine-expression box.
///
/// This proves no inclusion for a separately supplied original-coordinate box E.
/// It creates no portable vector witness or selected tuple.
#[derive(Clone, Debug)]
pub struct CheckedLatticeCoordinateEnclosure {
    /// Complete original target and exact affine-expression replay contract.
    contract: LatticeCoordinateEnclosure,
    /// Retained determinant and both original inverse identities.
    basis_proofs: [Handle; 3],
    /// Conservative returned ownership excluding shared graph records.
    bytes: u64,
}

impl CheckedLatticeCoordinateEnclosure {
    /// Borrow the original-target judgment, its exact affine expression and closed box.
    #[must_use]
    pub const fn contract(&self) -> &LatticeCoordinateEnclosure {
        &self.contract
    }

    /// Borrow determinant, right-inverse and left-inverse proof roots in that order.
    #[must_use]
    pub const fn basis_proofs(&self) -> &[Handle; 3] {
        &self.basis_proofs
    }

    /// Conservative prepaid local evidence ownership.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Cas {
    /// Replay a direct basis-coordinate box against the original target and exact inverse.
    ///
    /// Structural correspondence includes every original operand before scalar replay
    /// performs normalization. A sharper H concerns q itself and does not shrink an E.
    /// # Errors
    /// Refuses changed basis, original target, inverse, context or affine-expression
    /// correspondence, invalid scalar evidence and exhausted cumulative resources.
    pub fn check_lattice_coordinate_enclosure(
        &self,
        basis: &CheckedLatticeBasis,
        contract: &LatticeCoordinateEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeCoordinateEnclosure, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.lattice_coordinate_enclosure(basis, contract, budget);
        budget.restore_limits(previous);
        result
    }

    /// Reserve the complete returned contract after original correspondence and replay.
    fn lattice_coordinate_enclosure(
        &self,
        basis: &CheckedLatticeBasis,
        contract: &LatticeCoordinateEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeCoordinateEnclosure, Fault> {
        self.validate_lattice_basis(&contract.basis, basis, budget)?;
        budget.work(4, AT)?;
        self.checked(&contract.inverse)?;
        if contract.inverse != *basis.inverse().map().expression()
            || contract.coordinates.context != contract.basis.context
            || size(contract.coordinates.coordinates.len(), AT)? != u64::from(basis.dimension())
        {
            return Err(mismatch());
        }
        let home = self.state.context(self.checked(&contract.basis.context)?)?;
        let target = self.checked(&contract.target)?;
        if self.lattice_rectangle(target, home.header_id)? != (basis.dimension(), 1) {
            return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
        }
        let original = &self.state.expression(target)?.operands;
        let inverse = &self
            .state
            .expression(self.checked(&contract.inverse)?)?
            .operands;
        budget.check(BudgetKind::MatrixEntries, size(inverse.len(), AT)?, AT)?;
        let width = original.len().saturating_add(1);
        for (row, coordinate) in contract.coordinates.coordinates.iter().enumerate() {
            let start = row.saturating_mul(width);
            let coefficients = inverse
                .get(start..start.saturating_add(width))
                .ok_or_else(mismatch)?;
            correspondence(
                self,
                self.checked(&coordinate.target)?,
                coefficients,
                original,
                budget,
            )?;
        }
        let bytes =
            super::enclosure::validate(self, &contract.coordinates, budget)?.saturating_add(512);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
        budget.work(bytes, AT)?;
        Ok(CheckedLatticeCoordinateEnclosure {
            contract: contract.clone(),
            basis_proofs: [
                basis.inverse().nonsingularity().proof().clone(),
                basis.inverse().witness().proof().clone(),
                basis.left_inverse().proof().clone(),
            ],
            bytes,
        })
    }
}

/// The exact syntactic affine identity is checked before any cancellation can erase operands.
fn correspondence(
    cas: &Cas,
    mut root: LogicalId,
    coefficients: &[LogicalId],
    original: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (offset, linear) = coefficients.split_last().ok_or_else(mismatch)?;
    for (coefficient, target) in linear.iter().zip(original).rev() {
        budget.work(4, AT)?;
        let sum = cas.state.expression(root)?;
        let [previous, product] = sum.operands.as_slice() else {
            return Err(mismatch());
        };
        if sum.operator != Operator::Add {
            return Err(mismatch());
        }
        let product = cas.state.expression(*product)?;
        if product.operator != Operator::Mul || product.operands != [*coefficient, *target] {
            return Err(mismatch());
        }
        root = *previous;
    }
    if root != *offset {
        return Err(mismatch());
    }
    Ok(())
}

/// Equivalent normalized values do not replace original affine correspondence.
const fn mismatch() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
