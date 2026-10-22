// SPDX-License-Identifier: AGPL-3.0-only
//! Exact companion maps distinguish stored-copy bounds from common-source covariance.

use super::{PhysicalReading, ReadingError, support};
use nrz_cas::{
    Cas, Rational,
    limits::{Budget, BudgetKind},
    readings::AffineMap,
    syntax::Operator,
};

impl PhysicalReading {
    /// Compose the complete expansion, selection and reading over the original free coordinates.
    ///
    /// This is the statistical lineage map. Stored-copy error bounds still use
    /// `Self::map` and that copy's selected companions rather than this reduction.
    /// # Errors
    /// Refuses missing definitions, inconsistent families and cumulative exact limits.
    pub fn family_map(&self, cas: &Cas, budget: &mut Budget) -> Result<AffineMap, ReadingError> {
        let mut current = self.family.node().clone();
        let mut chain = Vec::new();
        while &current != self.family.root() {
            support::capacity::<AffineMap>(chain.len() + 1, budget)?;
            budget.check(
                BudgetKind::Depth,
                u64::try_from(chain.len() + 1).map_err(|_| ReadingError::Shape)?,
                support::AT,
            )?;
            let (_, operator, operands, _) = support::inspect(cas, &current, budget)?;
            let Operator::AffineReading(map) = operator else {
                return Err(ReadingError::Definition);
            };
            current = operands.get(1).ok_or(ReadingError::Definition)?.clone();
            chain
                .try_reserve(1)
                .map_err(|_| nrz_cas::Fault::allocation(support::AT))?;
            chain.push(*map);
        }
        let count =
            u32::try_from(self.family.root_entries.len()).map_err(|_| ReadingError::Shape)?;
        let mut expansion = AffineMap::identity(count, budget)?;
        for outer in chain.iter().rev() {
            expansion = outer.compose(&expansion, budget)?;
        }
        let selected = self.selection.compose(&expansion, budget)?;
        Ok(self.map.compose(&selected, budget)?)
    }
}

/// Radii and numerical errors use the absolute linear map, with no affine offset.
pub(super) fn radius(map: &AffineMap, budget: &mut Budget) -> Result<AffineMap, ReadingError> {
    let mut coefficients = support::reserve(map.coefficients().len(), budget)?;
    for value in map.coefficients() {
        coefficients.push(if value.is_negative() {
            value.negated(budget)?
        } else {
            value.clone()
        });
    }
    let mut offsets = support::reserve(
        usize::try_from(map.outputs()).map_err(|_| ReadingError::Shape)?,
        budget,
    )?;
    offsets.resize_with(
        usize::try_from(map.outputs()).map_err(|_| ReadingError::Shape)?,
        Rational::zero,
    );
    Ok(AffineMap::new(
        map.inputs(),
        map.outputs(),
        coefficients,
        offsets,
        budget,
    )?)
}

/// Column-ordered covariance contraction is the exact linear map for `T*U*Q^T`.
pub(super) fn covariance(
    left: &AffineMap,
    right: &AffineMap,
    budget: &mut Budget,
) -> Result<AffineMap, ReadingError> {
    let inputs = left
        .inputs()
        .checked_mul(right.inputs())
        .ok_or(ReadingError::Shape)?;
    let outputs = left
        .outputs()
        .checked_mul(right.outputs())
        .ok_or(ReadingError::Shape)?;
    let count = u64::from(inputs) * u64::from(outputs);
    let mut coefficients = support::reserve(
        usize::try_from(count).map_err(|_| ReadingError::Shape)?,
        budget,
    )?;
    for column in 0..right.outputs() {
        for row in 0..left.outputs() {
            for source_column in 0..right.inputs() {
                for source_row in 0..left.inputs() {
                    budget.work(1, support::AT)?;
                    coefficients.push(
                        coefficient(left, row, source_row)?
                            .mul(coefficient(right, column, source_column)?, budget)?,
                    );
                }
            }
        }
    }
    let mut offsets = support::reserve(
        usize::try_from(outputs).map_err(|_| ReadingError::Shape)?,
        budget,
    )?;
    offsets.resize_with(
        usize::try_from(outputs).map_err(|_| ReadingError::Shape)?,
        Rational::zero,
    );
    Ok(AffineMap::new(
        inputs,
        outputs,
        coefficients,
        offsets,
        budget,
    )?)
}

/// Resolve one checked row-major coefficient without unchecked dimension arithmetic.
fn coefficient(map: &AffineMap, row: u32, column: u32) -> Result<&Rational, ReadingError> {
    let position = u64::from(row) * u64::from(map.inputs()) + u64::from(column);
    map.coefficients()
        .get(usize::try_from(position).map_err(|_| ReadingError::Shape)?)
        .ok_or(ReadingError::Shape)
}
