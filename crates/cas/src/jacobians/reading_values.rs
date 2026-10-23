// SPDX-License-Identifier: AGPL-3.0-only
//! Simultaneous reading replacements preserve exact source identity and original map domains.

use super::{AT, JacobianReadings, SourceCoordinates, invalid, storage};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::VerifiedWitness;
use crate::substitution::Substitution;
use std::collections::BTreeSet;

/// Ordered replacements, complete reading/source column and original map theorem.
type ReadingValues = (Vec<Substitution>, Handle, VerifiedWitness);

impl Cas {
    /// Retain the explicit original-map equality before constructing any source replacements.
    pub(super) fn jacobian_reading_values(
        &mut self,
        context: &Handle,
        coordinates: &SourceCoordinates,
        readings: &JacobianReadings,
        budget: &mut Budget,
    ) -> Result<ReadingValues, Fault> {
        let original = self.checked(readings.map.expression())?;
        self.reading_coefficients_independent(original, &readings.variables, budget)?;
        let mut conditions = crate::conditions::Conditions::default();
        conditions.endpoint(&self.state, self.checked(context)?, original, budget)?;
        if !conditions.residuals.is_empty() {
            return Err(invalid(InvalidCode::JacobianDomain));
        }
        let mut entries = crate::readings::exact_expand::expand(self, original, budget)?;
        storage::retaining(
            size(entries.len(), AT)?.saturating_mul(128),
            budget,
            |budget| {
                for entry in &mut entries {
                    *entry = crate::polynomial::normalize(self, *entry, budget)?;
                }
                let (rows, columns) = crate::polynomial::matrix::shape(&self.state, original)?;
                let explicit = self.jacobian_rectangle(context, rows, columns, &entries, budget)?;
                let witness = self.prove_reading_equality(
                    context,
                    readings.map.expression(),
                    &explicit,
                    budget,
                )?;
                if !witness.is_discharged() {
                    return Err(invalid(InvalidCode::JacobianDomain));
                }
                self.replay_jacobian_fact(&witness, budget)?;
                storage::retaining(
                    size(witness.bytes.len(), AT)?.saturating_mul(8),
                    budget,
                    |budget| {
                        let (replacements, mapping) = self.reading_replacements(
                            context,
                            coordinates,
                            readings,
                            &entries,
                            budget,
                        )?;
                        Ok((replacements, mapping, witness))
                    },
                )
            },
        )
    }

    /// Reading symbols cannot appear in their own simultaneous map coefficients or offsets.
    fn reading_coefficients_independent(
        &self,
        root: LogicalId,
        readings: &[String],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            let term = self.state.expression(id)?;
            let count = visited
                .len()
                .saturating_add(pending.len())
                .saturating_add(term.operands.len())
                .saturating_add(1);
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(count, AT)?.saturating_mul(128),
                AT,
            )?;
            budget.work(
                size(visited.len().saturating_add(1), AT)?.saturating_add(
                    size(readings.len(), AT)?.saturating_mul(budget.limits().name_bytes),
                ),
                AT,
            )?;
            if !visited.insert(id) {
                continue;
            }
            if let Operator::Free(key) = &term.operator
                && readings.contains(key)
            {
                return Err(invalid(InvalidCode::JacobianReadingCycle));
            }
            pending
                .try_reserve(term.operands.len())
                .map_err(|_| Fault::allocation(AT))?;
            pending.extend_from_slice(&term.operands);
        }
        Ok(())
    }

    /// Every reading contracts against the same declared scalar source handles.
    fn reading_replacements(
        &mut self,
        context: &Handle,
        coordinates: &SourceCoordinates,
        readings: &JacobianReadings,
        coefficients: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<(Vec<Substitution>, Handle), Fault> {
        let header = self.state.context(self.checked(context)?)?.header_id;
        let mut sources = storage::reserve(coordinates.sources.len(), budget)?;
        for key in &coordinates.sources {
            budget.work(size(key.len(), AT)?, AT)?;
            sources.push(self.jacobian_scalar(header, Operator::Free(key.clone()), &[], budget)?);
        }
        let count = readings.variables.len().saturating_add(sources.len());
        let width =
            size(self.state.context(self.checked(context)?)?.header.len(), AT)?.saturating_add(256);
        storage::retaining(size(count, AT)?.saturating_mul(width), budget, |budget| {
            let mut replacements = storage::reserve(readings.variables.len(), budget)?;
            let mut mapped = storage::reserve(count, budget)?;
            for (row, key) in readings.variables.iter().enumerate() {
                let expression = self.reading_row(header, row, &sources, coefficients, budget)?;
                replacements.push(Substitution {
                    key: key.clone(),
                    replacement: self.handle(expression),
                });
                mapped.push(expression);
            }
            mapped.extend_from_slice(&sources);
            let rows =
                u32::try_from(mapped.len()).map_err(|_| invalid(InvalidCode::ReadingShape))?;
            let mapping = self.jacobian_rectangle(context, rows, 1, &mapped, budget)?;
            Ok((replacements, mapping))
        })
    }

    /// Do not elide zero coefficients before checking their full original operands.
    fn reading_row(
        &mut self,
        header: LogicalId,
        row: usize,
        sources: &[LogicalId],
        entries: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let columns = sources
            .len()
            .checked_add(1)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let start = row
            .checked_mul(columns)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let offset = *entries
            .get(start.saturating_add(sources.len()))
            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
        let mut sum = if self.jacobian_literal_zero(offset)? {
            None
        } else {
            Some(offset)
        };
        for (column, source) in sources.iter().enumerate() {
            budget.work(1, AT)?;
            let coefficient = *entries
                .get(start.saturating_add(column))
                .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
            if self.jacobian_literal_zero(coefficient)? {
                continue;
            }
            let term =
                self.jacobian_scalar(header, Operator::Mul, &[coefficient, *source], budget)?;
            sum = Some(if let Some(previous) = sum {
                self.jacobian_scalar(header, Operator::Add, &[previous, term], budget)?
            } else {
                term
            });
        }
        Ok(sum.unwrap_or(offset))
    }

    /// The original whole-map equality has already checked every normalized literal zero.
    fn jacobian_literal_zero(&self, expression: LogicalId) -> Result<bool, Fault> {
        Ok(
            matches!(&self.state.expression(expression)?.operator, Operator::Rational(value) if value.is_zero()),
        )
    }

    /// All source coordinates and contraction terms share the caller's declaration header.
    fn jacobian_scalar(
        &mut self,
        header: LogicalId,
        operator: Operator,
        operands: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        self.generated(
            header,
            operator,
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands,
            budget,
        )
    }
}
