// SPDX-License-Identifier: AGPL-3.0-only
//! Dense rational affine maps compose by ordered matrix contraction.

use super::{AT, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::size;
use crate::number::Rational;

/// The exact ordered map `y = T*z + d`, with row-major coefficients of `T`.
///
/// Coordinates of shaped values are in logical column order. Repeated and negated
/// coordinates are explicit rows, never independent copies or numerical premises.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineMap {
    /// Ordered source coordinate count.
    pub(super) inputs: u32,
    /// Ordered result coordinate count.
    pub(super) outputs: u32,
    /// Exact row-major coefficient vector.
    pub(super) coefficients: Vec<Rational>,
    /// Exact ordered translation vector.
    pub(super) offsets: Vec<Rational>,
}

impl AffineMap {
    /// Check every original coefficient and offset before admitting the map.
    /// # Errors
    /// Refuses dimension mismatches and exhausted entry, payload or arithmetic limits.
    pub fn new(
        inputs: u32,
        outputs: u32,
        coefficients: Vec<Rational>,
        offsets: Vec<Rational>,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let result = Self {
            inputs,
            outputs,
            coefficients,
            offsets,
        };
        result.validate(budget)?;
        Ok(result)
    }

    /// Number of ordered input coordinates.
    #[must_use]
    pub const fn inputs(&self) -> u32 {
        self.inputs
    }

    /// Number of ordered output coordinates.
    #[must_use]
    pub const fn outputs(&self) -> u32 {
        self.outputs
    }

    /// Row-major exact coefficient array.
    #[must_use]
    pub const fn coefficients(&self) -> &[Rational] {
        self.coefficients.as_slice()
    }

    /// Ordered exact translation vector.
    #[must_use]
    pub const fn offsets(&self) -> &[Rational] {
        self.offsets.as_slice()
    }

    /// Construct the identity over exactly this coordinate order.
    /// # Errors
    /// Refuses entry, payload or work exhaustion before allocating.
    pub fn identity(count: u32, budget: &mut Budget) -> Result<Self, Fault> {
        let mut coefficients = reserve(length(count, count)?, budget)?;
        for row in 0..count {
            for column in 0..count {
                coefficients.push(if row == column {
                    Rational::one()
                } else {
                    Rational::zero()
                });
            }
        }
        let mut offsets = reserve(length(count, 1)?, budget)?;
        offsets.resize_with(length(count, 1)?, Rational::zero);
        Self::new(count, count, coefficients, offsets, budget)
    }

    /// Compose `self(inner(z))` in that order, retaining every translation term.
    /// # Errors
    /// Refuses incompatible dimensions and exhausted cumulative resources.
    pub fn compose(&self, inner: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let outer_bytes = self.validate(budget)?;
        let inner_bytes = inner.validate(budget)?;
        if self.inputs != inner.outputs {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        let retained = outer_bytes
            .checked_add(inner_bytes)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = self.compose_rows(inner, budget);
        budget.restore_limits(previous);
        result
    }

    /// Reserve every retained partial output before the next exact contraction.
    fn compose_rows(&self, inner: &Self, budget: &mut Budget) -> Result<Self, Fault> {
        let mut coefficients = reserve(length(self.outputs, inner.inputs)?, budget)?;
        let mut offsets = reserve(length(self.outputs, 1)?, budget)?;
        let mut retained = 0;
        for row in 0..self.outputs {
            for column in 0..inner.inputs {
                let value = self.contract_reserved(inner, row, Some(column), retained, budget)?;
                retain(&value, &mut retained, budget)?;
                coefficients.push(value);
            }
            let value = self.contract_reserved(inner, row, None, retained, budget)?;
            retain(&value, &mut retained, budget)?;
            offsets.push(value);
        }
        Self::new(inner.inputs, self.outputs, coefficients, offsets, budget)
    }

    /// Nested arithmetic shares the remaining payload allowance and the original Work ledger.
    fn contract_reserved(
        &self,
        inner: &Self,
        row: u32,
        column: Option<u32>,
        retained: u64,
        budget: &mut Budget,
    ) -> Result<Rational, Fault> {
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = self.contraction(inner, row, column, budget);
        budget.restore_limits(previous);
        result
    }

    /// Select output rows in the supplied exact order, without sorting or merging them.
    /// # Errors
    /// Refuses out-of-range indices and exhausted cumulative resources.
    pub fn project(&self, rows: &[u32], budget: &mut Budget) -> Result<Self, Fault> {
        let retained = self.validate(budget)?;
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = self.project_rows(rows, budget);
        budget.restore_limits(previous);
        result
    }

    /// Account for repeated large coefficients before each additional owned copy.
    fn project_rows(&self, rows: &[u32], budget: &mut Budget) -> Result<Self, Fault> {
        budget.work(size(rows.len(), AT)?, AT)?;
        if rows.iter().any(|row| *row >= self.outputs) {
            return Err(invalid(InvalidCode::SelectionOutOfRange));
        }
        let outputs = u32::try_from(rows.len()).map_err(|_| invalid(InvalidCode::ReadingShape))?;
        let mut coefficients = reserve(length(outputs, self.inputs)?, budget)?;
        let mut offsets = reserve(rows.len(), budget)?;
        let mut retained = 0;
        for row in rows {
            for column in 0..self.inputs {
                let value = self.coefficient(*row, column)?;
                retain(value, &mut retained, budget)?;
                coefficients.push(value.clone());
            }
            let value = self.offset(*row)?;
            retain(value, &mut retained, budget)?;
            offsets.push(value.clone());
        }
        Self::new(self.inputs, outputs, coefficients, offsets, budget)
    }

    /// Bound the complete owned coefficient payload before any copies or comparisons.
    pub(super) fn validate(&self, budget: &mut Budget) -> Result<u64, Fault> {
        let expected = length(self.inputs, self.outputs)?;
        if self.coefficients.len() != expected || self.offsets.len() != length(self.outputs, 1)? {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        let count = expected
            .checked_add(self.offsets.len())
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        budget.check(BudgetKind::MatrixEntries, size(count, AT)?, AT)?;
        budget.work(size(count, AT)?, AT)?;
        let mut out = Encoder::new(budget, AT);
        super::wire::affine(self, &mut out)?;
        let bytes = size(out.finish().len(), AT)?;
        let payload = bytes
            .checked_mul(4)
            .and_then(|n| n.checked_add(size(count, AT).ok()?.checked_mul(64)?))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, payload, AT)?;
        budget.work(payload, AT)?;
        Ok(payload)
    }

    /// One ordered dot product, including the affine term for an offset column.
    fn contraction(
        &self,
        inner: &Self,
        row: u32,
        column: Option<u32>,
        budget: &mut Budget,
    ) -> Result<Rational, Fault> {
        let mut result =
            column.map_or_else(|| self.offset(row).cloned(), |_| Ok(Rational::zero()))?;
        for coordinate in 0..self.inputs {
            budget.work(1, AT)?;
            let right = column.map_or_else(
                || inner.offset(coordinate),
                |column| inner.coefficient(coordinate, column),
            )?;
            let product = self.coefficient(row, coordinate)?.mul(right, budget)?;
            result = result.add(&product, budget)?;
        }
        Ok(result)
    }

    /// Resolve a checked ordered coefficient without unchecked indexing.
    pub(super) fn coefficient(&self, row: u32, column: u32) -> Result<&Rational, Fault> {
        let index = length(row, self.inputs)?
            .checked_add(length(column, 1)?)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        self.coefficients
            .get(index)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))
    }

    /// Resolve one translation entry without unchecked indexing.
    fn offset(&self, row: u32) -> Result<&Rational, Fault> {
        self.offsets
            .get(length(row, 1)?)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))
    }
}

/// Dimension arithmetic cannot wrap into a smaller admitted array.
pub(crate) fn length(rows: u32, columns: u32) -> Result<usize, Fault> {
    usize::try_from(u64::from(rows) * u64::from(columns))
        .map_err(|_| invalid(InvalidCode::ReadingShape))
}

/// Charge the owned rational bytes before retaining or copying another coefficient.
fn retain(value: &Rational, retained: &mut u64, budget: &mut Budget) -> Result<(), Fault> {
    let previous = budget.reserve_temporary(*retained, AT)?;
    let measured = value.structural_bytes(budget);
    budget.restore_limits(previous);
    let bytes = size(measured?.len(), AT)?
        .checked_mul(4)
        .and_then(|count| count.checked_add(64))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    *retained = retained
        .checked_add(bytes)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, *retained, AT)?;
    budget.work(bytes, AT)
}
