// SPDX-License-Identifier: AGPL-3.0-only
//! Checked affine lattice bases over the existing exact matrix and proof vocabulary.

mod affine_enclosure;
mod coordinate_enclosure;
pub(crate) mod cover;
mod enclosure;
mod index;
pub(crate) mod selection;
pub use affine_enclosure::{CheckedLatticeBoxEnclosure, LatticeBoxEnclosure};
pub use coordinate_enclosure::{CheckedLatticeCoordinateEnclosure, LatticeCoordinateEnclosure};
pub use cover::LatticeCover;
pub use enclosure::{CheckedRationalBoxEnclosure, RationalBoxEnclosure};

pub use selection::{
    CheckedLatticeAttempt, CheckedLatticeOutputs, LatticeAttemptPolicy, LatticeConvention,
    LatticeEnclosure, LatticeFloorCut, LatticePendingObligation, LatticeReduction,
    LatticeSelection, LatticeSelectionOutcome, LatticeUndecided, LatticeUndecidedReason,
};

pub use index::{CheckedLatticeIndexDomain, LatticeIndexQuestion};

use crate::budget::{Budget, BudgetKind};
use crate::conditions::Conditions;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::proof_model::{Statement, VerifiedWitness};
use crate::readings::{CheckedMapInverse, ExactAffineMap};

/// Lattice admission owns only its additional checks and retained evidence.
const AT: Location = Location::new("lattice basis admission");

/// Original exact syntax and its complete parameter context, before normalization.
///
/// The basis is an explicit nonempty square Real matrix; the offset is an explicit
/// Real column of the same height. Row order declares the positional coordinate frame.
/// This is the basis part of a question, without a target, index range or cell cover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeBasisQuestion {
    /// Retained whole-family declaration and hypothesis context.
    pub context: Handle,
    /// Original row-major column basis, including every unnormalized operand.
    pub basis: Handle,
    /// Original affine offset as a column matrix.
    pub offset: Handle,
}

/// Checked admission or an explicit lack of supported whole-family evidence.
#[derive(Clone, Debug)]
pub enum LatticeBasisOutcome {
    /// Both inverse identities and the determinant certificate are checked.
    Admitted(Box<CheckedLatticeBasis>),
    /// Original definedness or nonsingularity has an undischarged obligation.
    /// No candidate graph records or partial admission are published.
    Unresolved,
}

/// Sealed basis admission retaining both affine identities and all used assumptions.
///
/// The inverse map is exactly `t = A*(q-offset)`. Its two homogeneous identities
/// establish `A*B = B*A = I` and the corresponding translated-origin identities.
/// No index-range, product-cover or tuple-selection theorem is implied.
#[derive(Clone, Debug)]
pub struct CheckedLatticeBasis {
    /// Immutable original question, never its normalized point-set equivalent.
    question: LatticeBasisQuestion,
    /// Exact forward map, preserving original entries and offsets.
    forward: ExactAffineMap,
    /// Shared inverse producer's determinant and right-inverse evidence.
    inverse: CheckedMapInverse,
    /// Independently checked inverse followed by the original map.
    left_inverse: VerifiedWitness,
}

impl CheckedLatticeBasis {
    /// Exact original identities and whole-family context admitted by this witness.
    #[must_use]
    pub const fn question(&self) -> &LatticeBasisQuestion {
        &self.question
    }

    /// Ordered finite dimension, excluding the homogeneous constant coordinate.
    #[must_use]
    pub const fn dimension(&self) -> u32 {
        self.forward.inputs()
    }

    /// Exact forward affine map `q = offset + B*t`.
    #[must_use]
    pub const fn forward(&self) -> &ExactAffineMap {
        &self.forward
    }

    /// Exact inverse with its original right-inverse and nonsingularity witnesses.
    #[must_use]
    pub const fn inverse(&self) -> &CheckedMapInverse {
        &self.inverse
    }

    /// Checked left-inverse identity, with ordinary portable witness replay.
    #[must_use]
    pub const fn left_inverse(&self) -> &VerifiedWitness {
        &self.left_inverse
    }
}

impl Cas {
    /// Check a full-rank affine basis without enumerating any lattice indices.
    ///
    /// Original entries are checked before cancellation. Exact determinant and inverse
    /// construction reuse the reading-map checker; both multiplication orders are checked.
    /// Matrix entries, rational components, proof bytes and cumulative Work keep their
    /// existing ceilings. Unknown family evidence returns `Unresolved` atomically.
    /// # Errors
    /// Returns `ReadingSingular` for a singular basis, `ReadingShape` for unsupported
    /// rectangles, `RefutedDomain` for undefined entries, and ordinary identity/resource
    /// faults. Refusals publish no graph records and never restore spent Work.
    pub fn admit_lattice_basis(
        &mut self,
        question: &LatticeBasisQuestion,
        budget: &mut Budget,
    ) -> Result<LatticeBasisOutcome, Fault> {
        let result = self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| cas.lattice_basis(question, budget))
        });
        match result {
            Ok(admitted) => Ok(LatticeBasisOutcome::Admitted(Box::new(admitted))),
            Err(Fault {
                kind: FaultKind::Invalid(InvalidCode::ReadingNonsingularity),
                ..
            }) => Ok(LatticeBasisOutcome::Unresolved),
            Err(fault) => Err(fault),
        }
    }

    /// Authenticate a local admission against the complete original basis question.
    ///
    /// Equal lattice point sets, changed offsets and changed contexts cannot reuse it.
    /// # Errors
    /// Refuses foreign handles, changed identities and exhausted cumulative Work.
    pub fn validate_lattice_basis(
        &self,
        question: &LatticeBasisQuestion,
        admitted: &CheckedLatticeBasis,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(7, AT)?;
        for handle in [
            &question.context,
            &question.basis,
            &question.offset,
            &admitted.question.context,
            &admitted.question.basis,
            &admitted.question.offset,
        ] {
            self.checked(handle)?;
        }
        if question != &admitted.question {
            return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
        }
        Ok(())
    }

    /// Validate the original rectangles and domains before any inverse proposal.
    fn lattice_basis(
        &mut self,
        question: &LatticeBasisQuestion,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeBasis, Fault> {
        budget.work(3, AT)?;
        let home = self.checked(&question.context)?;
        let header = self.state.context(home)?.header_id;
        let basis = self.checked(&question.basis)?;
        let offset = self.checked(&question.offset)?;
        let (rows, columns) = self.lattice_rectangle(basis, header)?;
        if rows == 0 || rows != columns || self.lattice_rectangle(offset, header)? != (rows, 1) {
            return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
        }
        let count = u64::from(rows).saturating_mul(u64::from(rows));
        budget.check(BudgetKind::MatrixEntries, count, AT)?;
        let slots = count.saturating_add(u64::from(rows));
        let bytes = slots.saturating_mul(size(std::mem::size_of::<Handle>(), AT)?);
        budget.with_retained_payload(bytes, AT, |budget| {
            let mut domains = Conditions::default();
            domains.endpoint(&self.state, home, basis, budget)?;
            domains.endpoint(&self.state, home, offset, budget)?;
            if !domains.residuals.is_empty() {
                return Err(Fault::invalid(InvalidCode::ReadingNonsingularity, AT));
            }
            let coefficients = self.lattice_entries(basis, budget)?;
            let offsets = self.lattice_entries(offset, budget)?;
            let forward = self.exact_reading_map(
                &question.context,
                (rows, rows),
                &coefficients,
                &offsets,
                budget,
            )?;
            self.lattice_inverse(question, forward, budget)
        })
    }

    /// Accept explicit Real matrices only, under the identical retained declaration.
    fn lattice_rectangle(&self, id: LogicalId, header: LogicalId) -> Result<(u32, u32), Fault> {
        let term = self.state.expression(id)?;
        let Operator::Matrix { rows, columns } = term.operator else {
            return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
        };
        let expected = Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        });
        if term.header != header
            || term.sort != expected
            || !term.scope.is_empty()
            || term.binding.is_some()
            || term.space.is_some()
            || term.domain.is_some()
            || term.attachment.is_some()
            || !term.arms.is_empty()
        {
            return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
        }
        Ok((rows, columns))
    }

    /// Copy only bounded original entry handles, preserving row-major ordering.
    fn lattice_entries(&self, id: LogicalId, budget: &mut Budget) -> Result<Vec<Handle>, Fault> {
        let entries = &self.state.expression(id)?.operands;
        budget.work(size(entries.len(), AT)?, AT)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(entries.len())
            .map_err(|_| Fault::allocation(AT))?;
        result.extend(entries.iter().map(|entry| self.handle(*entry)));
        Ok(result)
    }

    /// Reuse the exact inverse, then independently check the other multiplication order.
    fn lattice_inverse(
        &mut self,
        question: &LatticeBasisQuestion,
        forward: ExactAffineMap,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeBasis, Fault> {
        let inverse = self.invert_reading_map(&question.context, &forward, budget)?;
        let retained = size(inverse.witness().bytes.len(), AT)?
            .saturating_add(size(inverse.nonsingularity().bytes.len(), AT)?);
        budget.with_retained_payload(
            retained.saturating_mul(4).saturating_add(1_024),
            AT,
            |budget| {
                let Statement::Equality {
                    right: identity, ..
                } = &inverse.witness().judgment().statement
                else {
                    return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
                };
                let reverse = self.compose_reading_maps(inverse.map(), &forward, budget)?;
                let left_inverse = self.prove_reading_equality(
                    &question.context,
                    reverse.expression(),
                    identity,
                    budget,
                )?;
                if !left_inverse.is_discharged() {
                    return Err(Fault::invalid(InvalidCode::ReadingNonsingularity, AT));
                }
                let proof_bytes = retained.saturating_add(size(left_inverse.bytes.len(), AT)?);
                budget.check(BudgetKind::WitnessBytes, proof_bytes, AT)?;
                budget.check(BudgetKind::OutputBytes, proof_bytes.saturating_add(512), AT)?;
                Ok(CheckedLatticeBasis {
                    question: question.clone(),
                    forward,
                    inverse,
                    left_inverse,
                })
            },
        )
    }
}
