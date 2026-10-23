// SPDX-License-Identifier: AGPL-3.0-only
//! Checked whole-family inverses retain original domains and ordinary replayable equality evidence.

use super::{
    AT, ExactAffineMap, affine::length, determinant, exact_expand, exact_map::extent, invalid,
    reserve,
};
use crate::budget::{Budget, BudgetKind};
use crate::conditions::{Conditions, GuardKind, Obligation};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

/// An exact inverse and the checked whole-family identity `original * inverse = identity`.
///
/// The witness includes every original denominator and all used context hypotheses.
/// Retain it with the inverse: a map expression alone is not a nonsingularity theorem.
#[derive(Clone, Debug)]
pub struct CheckedMapInverse {
    /// Reversed coordinate map, including the translated origin.
    map: ExactAffineMap,
    /// Exact determinant used by every cofactor reciprocal.
    determinant: Handle,
    /// Ordinary portable coefficient equality with its full conditions.
    witness: VerifiedWitness,
    /// Checked exact determinant quotient, retaining its nonzero denominator premise.
    nonsingularity: VerifiedWitness,
}

/// Borrowed exact problem data retained while the second witness is checked.
struct InverseProblem<'a> {
    /// Complete mathematical hypothesis context.
    context: &'a Handle,
    /// Original homogeneous map and coordinate order.
    original: &'a ExactAffineMap,
    /// Common immutable declaration snapshot.
    header: LogicalId,
    /// Original ordered linear coefficients.
    linear: &'a [LogicalId],
    /// Original ordered translation coefficients.
    offsets: &'a [LogicalId],
    /// Checked exact nonzero determinant.
    determinant: LogicalId,
}

impl CheckedMapInverse {
    /// Exact inverse map `z=T^(-1)*(y-d)`.
    #[must_use]
    pub const fn map(&self) -> &ExactAffineMap {
        &self.map
    }

    /// Exact determinant, established nonzero throughout the witness's context.
    #[must_use]
    pub const fn determinant(&self) -> &Handle {
        &self.determinant
    }

    /// Exact determinant certificate `det(original)/determinant=1` with its nonzero guard.
    #[must_use]
    pub const fn nonsingularity(&self) -> &VerifiedWitness {
        &self.nonsingularity
    }

    /// Replayable right-inverse identity; squareness makes it a two-sided inverse.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }
}

impl Cas {
    /// Produce and independently check the inverse of an exact square reading map.
    ///
    /// A positivity or nonzero hypothesis remains a qualified witness hypothesis.
    /// Unknown nonsingularity refuses rather than restricting the family silently.
    /// # Errors
    /// Returns `ReadingSingular` for an identically zero determinant,
    /// `ReadingNonsingularity` for missing family/domain evidence, and the ordinary
    /// shape, foreign-handle or cumulative resource refusal without publication.
    pub fn invert_reading_map(
        &mut self,
        context: &Handle,
        map: &ExactAffineMap,
        budget: &mut Budget,
    ) -> Result<CheckedMapInverse, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let home = cas.checked(context)?;
                let original = cas.checked(&map.expression)?;
                if map.inputs != map.outputs {
                    return Err(invalid(InvalidCode::ReadingShape));
                }
                let header = cas.state.context(home)?.header_id;
                let entries = exact_expand::expand(cas, original, budget)?;
                let retained = size(entries.len(), AT)?.saturating_mul(128);
                let previous = budget.reserve_temporary(retained, AT)?;
                let result = cas.inverse_entries(context, map, &entries, header, budget);
                budget.restore_limits(previous);
                result
            })
        })
    }

    /// Build the adjugate, translated origin and independently checked identity.
    fn inverse_entries(
        &mut self,
        context: &Handle,
        original: &ExactAffineMap,
        entries: &[LogicalId],
        header: LogicalId,
        budget: &mut Budget,
    ) -> Result<CheckedMapInverse, Fault> {
        let count = original.inputs;
        let columns = extent(count)?;
        let mut linear = reserve(length(count, count)?, budget)?;
        let mut offsets = reserve(length(count, 1)?, budget)?;
        for row in 0..count {
            let start = length(row, columns)?;
            let end = start.saturating_add(length(count, 1)?);
            linear.extend_from_slice(
                entries
                    .get(start..end)
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
            );
            offsets.push(
                *entries
                    .get(end)
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
            );
        }
        let proposed = determinant::determinant(self, header, count, &linear, budget)?;
        let determinant = crate::polynomial::normalize(self, proposed, budget)?;
        self.inverse_domain(context, original, determinant, budget)?;
        let nonsingularity = self.determinant_witness(context, original, determinant, budget)?;
        let problem = InverseProblem {
            context,
            original,
            header,
            linear: &linear,
            offsets: &offsets,
            determinant,
        };
        let retained = size(nonsingularity.bytes.len(), AT)?
            .saturating_mul(4)
            .saturating_add(1024);
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = self.inverse_candidate(&problem, budget);
        budget.restore_limits(previous);
        let (map, witness) = result?;
        let output = size(nonsingularity.bytes.len(), AT)?
            .saturating_add(size(witness.bytes.len(), AT)?)
            .saturating_add(256);
        budget.check(BudgetKind::OutputBytes, output, AT)?;
        Ok(CheckedMapInverse {
            map,
            determinant: self.handle(determinant),
            witness,
            nonsingularity,
        })
    }

    /// Retain the determinant witness while building and checking the complete inverse map.
    fn inverse_candidate(
        &mut self,
        problem: &InverseProblem<'_>,
        budget: &mut Budget,
    ) -> Result<(ExactAffineMap, VerifiedWitness), Fault> {
        let count = problem.original.inputs;
        let coefficients = self.adjugate(
            problem.header,
            count,
            problem.linear,
            problem.determinant,
            budget,
        )?;
        let coefficients = self.normalize_map_entries(coefficients, budget)?;
        let shifts = self.inverse_offsets(
            problem.header,
            count,
            &coefficients,
            problem.offsets,
            budget,
        )?;
        let shifts = self.normalize_map_entries(shifts, budget)?;
        let map = self.map_from_entries(
            problem.header,
            (count, count),
            &coefficients,
            &shifts,
            budget,
        )?;
        let product = self.compose_reading_maps(problem.original, &map, budget)?;
        let identity = self.exact_identity(problem.header, count, budget)?;
        let witness = self.prove_reading_equality(
            problem.context,
            &product.expression,
            &identity.expression,
            budget,
        )?;
        if !witness.is_discharged() {
            return Err(invalid(InvalidCode::ReadingNonsingularity));
        }
        Ok((map, witness))
    }

    /// Elimination independently checks the determinant proposal and its nonzero denominator.
    fn determinant_witness(
        &mut self,
        context: &Handle,
        original: &ExactAffineMap,
        determinant: LogicalId,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let header = self.state.context(self.checked(context)?)?.header_id;
        let matrix = self.checked(&original.expression)?;
        let actual = self.map_scalar(
            header,
            Operator::MatrixMap(crate::model::MatrixOperation::Determinant),
            &[matrix],
            budget,
        )?;
        let quotient = self.map_scalar(header, Operator::Div, &[actual, determinant], budget)?;
        let one = self.map_scalar(header, Operator::Rational(Rational::one()), &[], budget)?;
        let step = ApplicationDraft {
            rule: Primitive::RationalIdentity,
            context: context.clone(),
            conclusion: Statement::Equality {
                left: self.handle(quotient),
                right: self.handle(one),
                domain: self.admissible_region(context)?,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        let witness = self.check_proof(context, &[step], budget)?;
        if !witness.is_discharged() {
            return Err(invalid(InvalidCode::ReadingNonsingularity));
        }
        Ok(witness)
    }

    /// Canonical scalar coefficients remain proposals until the original matrix identity is checked.
    fn normalize_map_entries(
        &mut self,
        entries: Vec<LogicalId>,
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let mut result = reserve(entries.len(), budget)?;
        for entry in entries {
            result.push(crate::polynomial::normalize(self, entry, budget)?);
        }
        Ok(result)
    }

    /// Require original definedness and whole-family determinant nonzero evidence.
    fn inverse_domain(
        &self,
        context: &Handle,
        original: &ExactAffineMap,
        determinant: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if matches!(&self.state.expression(determinant)?.operator, Operator::Rational(value) if value.is_zero())
        {
            return Err(invalid(InvalidCode::ReadingSingular));
        }
        let home = self.checked(context)?;
        let mut conditions = Conditions::default();
        conditions.endpoint(
            &self.state,
            home,
            self.checked(&original.expression)?,
            budget,
        )?;
        conditions.require(
            &self.state,
            home,
            Obligation {
                kind: GuardKind::NonZero,
                term: determinant,
            },
            budget,
        )?;
        if !conditions.residuals.is_empty() {
            return Err(invalid(InvalidCode::ReadingNonsingularity));
        }
        Ok(())
    }

    /// Transpose the signed cofactor matrix and divide by the certified determinant.
    fn adjugate(
        &mut self,
        header: LogicalId,
        count: u32,
        entries: &[LogicalId],
        determinant: LogicalId,
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let mut result = reserve(length(count, count)?, budget)?;
        for row in 0..count {
            for column in 0..count {
                let minor = determinant::minor(entries, count, column, row, budget)?;
                let mut cofactor = determinant::determinant(
                    self,
                    header,
                    count.saturating_sub(1),
                    &minor,
                    budget,
                )?;
                if (row % 2) != (column % 2) {
                    cofactor = self.map_scalar(header, Operator::Neg, &[cofactor], budget)?;
                }
                result.push(self.map_scalar(
                    header,
                    Operator::Div,
                    &[cofactor, determinant],
                    budget,
                )?);
            }
        }
        Ok(result)
    }

    /// Translate by the exact negative inverse applied to the original origin.
    fn inverse_offsets(
        &mut self,
        header: LogicalId,
        count: u32,
        coefficients: &[LogicalId],
        offsets: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let mut result = reserve(length(count, 1)?, budget)?;
        for row in 0..count {
            let mut sum =
                self.map_scalar(header, Operator::Rational(Rational::zero()), &[], budget)?;
            for column in 0..count {
                budget.work(1, AT)?;
                let coefficient = *coefficients
                    .get(length(row, count)?.saturating_add(length(column, 1)?))
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
                let offset = *offsets
                    .get(length(column, 1)?)
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
                let product =
                    self.map_scalar(header, Operator::Mul, &[coefficient, offset], budget)?;
                sum = self.map_scalar(header, Operator::Add, &[sum, product], budget)?;
            }
            result.push(self.map_scalar(header, Operator::Neg, &[sum], budget)?);
        }
        Ok(result)
    }

    /// Build the complete homogeneous identity without an unused-axis loop.
    pub(super) fn exact_identity(
        &mut self,
        header: LogicalId,
        count: u32,
        budget: &mut Budget,
    ) -> Result<ExactAffineMap, Fault> {
        let zero = self.map_scalar(header, Operator::Rational(Rational::zero()), &[], budget)?;
        let one = self.map_scalar(header, Operator::Rational(Rational::one()), &[], budget)?;
        let mut coefficients = reserve(length(count, count)?, budget)?;
        for row in 0..count {
            for column in 0..count {
                coefficients.push(if row == column { one } else { zero });
            }
        }
        let mut offsets = reserve(length(count, 1)?, budget)?;
        offsets.resize(length(count, 1)?, zero);
        self.map_from_entries(header, (count, count), &coefficients, &offsets, budget)
    }
}
