// SPDX-License-Identifier: AGPL-3.0-only
//! Independently executed exact reversible row operations.

use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::Operator;
use crate::solving::RowOperation;
use crate::solving::coefficients::storage::vector;
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::{AT, invalid, premise};
use crate::solving::elementary::value::Value;

/// Row-major exact expressions, never a numerical matrix certificate.
pub(crate) struct Matrix {
    /// Fixed row count.
    pub(crate) rows: usize,
    /// Fixed column count.
    pub(crate) columns: usize,
    /// Strictly defined exact entries.
    pub(crate) entries: Vec<Value>,
}

impl Matrix {
    /// Every matrix entry is interpreted in the same coefficient context.
    pub(crate) fn read(
        evaluation: &Evaluation<'_>,
        handle: &Handle,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let stored = evaluation.cas.state.expression(handle.logical_id())?;
        let Operator::Matrix { rows, columns } = stored.operator else {
            return Err(invalid());
        };
        let rows = usize::try_from(rows).map_err(|_| invalid())?;
        let columns = usize::try_from(columns).map_err(|_| invalid())?;
        if rows.checked_mul(columns) != Some(stored.operands.len()) {
            return Err(invalid());
        }
        let mut entries = vector(stored.operands.len(), budget)?;
        for id in &stored.operands {
            entries.push(evaluation.expression(*id, true, budget)?);
        }
        Ok(Self {
            rows,
            columns,
            entries,
        })
    }

    /// Checked indexing keeps malformed extents outside the arithmetic kernel.
    pub(crate) fn at(&self, row: usize, column: usize) -> Result<&Value, Fault> {
        if row >= self.rows || column >= self.columns {
            return Err(invalid());
        }
        self.entries
            .get(
                row.checked_mul(self.columns)
                    .and_then(|index| index.checked_add(column))
                    .ok_or_else(invalid)?,
            )
            .ok_or_else(invalid)
    }

    /// Replace one checked entry without assuming caller row validity.
    fn put(&mut self, row: usize, column: usize, value: Value) -> Result<(), Fault> {
        if row >= self.rows || column >= self.columns {
            return Err(invalid());
        }
        let index = row
            .checked_mul(self.columns)
            .and_then(|index| index.checked_add(column))
            .ok_or_else(invalid)?;
        *self.entries.get_mut(index).ok_or_else(invalid)? = value;
        Ok(())
    }

    /// A scale is reversible only after an independently checked nonvanishing obligation.
    fn scale(
        &mut self,
        evaluation: &Evaluation<'_>,
        row: u32,
        value: &Handle,
        unit: u32,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        premise(evaluation.premises, unit)?;
        budget.solving(BudgetKind::LinearPivots, 1, AT)?;
        let factor = evaluation.expression(value.logical_id(), true, budget)?;
        evaluation.nonzero(&factor, budget)?;
        let row = usize::try_from(row).map_err(|_| invalid())?;
        if row >= self.rows {
            return Err(invalid());
        }
        for column in 0..self.columns {
            self.put(
                row,
                column,
                self.at(row, column)?
                    .mul(&factor, &evaluation.engine, budget)?,
            )?;
        }
        Ok(())
    }

    /// Adding a multiple of a distinct row has its explicit negative-multiple inverse.
    fn add(
        &mut self,
        evaluation: &Evaluation<'_>,
        destination: u32,
        source: u32,
        factor: &Handle,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let destination = usize::try_from(destination).map_err(|_| invalid())?;
        let source = usize::try_from(source).map_err(|_| invalid())?;
        if destination == source || destination >= self.rows || source >= self.rows {
            return Err(invalid());
        }
        let factor = evaluation.expression(factor.logical_id(), true, budget)?;
        for column in 0..self.columns {
            let multiple = self
                .at(source, column)?
                .mul(&factor, &evaluation.engine, budget)?;
            self.put(
                destination,
                column,
                self.at(destination, column)?
                    .add(&multiple, &evaluation.engine, budget)?,
            )?;
        }
        Ok(())
    }

    /// Swaps are their own inverses; all operations preserve the full augmented row.
    pub(crate) fn operation(
        &mut self,
        evaluation: &Evaluation<'_>,
        operation: &RowOperation<Handle>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        match operation {
            RowOperation::Swap { left, right } => {
                let left = usize::try_from(*left).map_err(|_| invalid())?;
                let right = usize::try_from(*right).map_err(|_| invalid())?;
                if left == right || left >= self.rows || right >= self.rows {
                    return Err(invalid());
                }
                for column in 0..self.columns {
                    budget.work(1, AT)?;
                    self.entries
                        .swap(left * self.columns + column, right * self.columns + column);
                }
                Ok(())
            }
            RowOperation::Scale { row, value, unit } => {
                self.scale(evaluation, *row, value, *unit, budget)
            }
            RowOperation::AddMultiple {
                destination,
                source,
                value,
            } => self.add(evaluation, *destination, *source, value, budget),
        }
    }

    /// A supplied reduced matrix must equal every independently reconstructed entry.
    pub(crate) fn equal(
        &self,
        evaluation: &Evaluation<'_>,
        other: &Self,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if (self.rows, self.columns) != (other.rows, other.columns) {
            return Err(invalid());
        }
        for (left, right) in self.entries.iter().zip(&other.entries) {
            evaluation.zero(&left.sub(right, &evaluation.engine, budget)?, budget)?;
        }
        Ok(())
    }

    /// Echelon pivots give a nonzero triangular minor and zero rows give the independent upper bound.
    pub(crate) fn rank(
        &self,
        evaluation: &Evaluation<'_>,
        pivots: &[u32],
        minor: &Handle,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if pivots.len() > self.rows {
            return Err(invalid());
        }
        let mut determinant = Value::integer(1, &evaluation.engine, budget)?;
        let mut previous = None;
        for (row, pivot) in pivots.iter().enumerate() {
            if previous.is_some_and(|column| column >= *pivot) {
                return Err(invalid());
            }
            let column = usize::try_from(*pivot).map_err(|_| invalid())?;
            budget.solving(BudgetKind::LinearPivots, 1, AT)?;
            evaluation.nonzero(self.at(row, column)?, budget)?;
            determinant = determinant.mul(self.at(row, column)?, &evaluation.engine, budget)?;
            for before in 0..column {
                evaluation.zero(self.at(row, before)?, budget)?;
            }
            previous = Some(*pivot);
        }
        for row in pivots.len()..self.rows {
            for column in 0..self.columns {
                evaluation.zero(self.at(row, column)?, budget)?;
            }
        }
        let supplied = evaluation.expression(minor.logical_id(), true, budget)?;
        evaluation.zero(
            &determinant.sub(&supplied, &evaluation.engine, budget)?,
            budget,
        )
    }
}
