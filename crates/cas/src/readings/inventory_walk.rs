// SPDX-License-Identifier: AGPL-3.0-only
//! Propagate exact affine output demands while retaining every original premise separately.

use super::inventory_budget::index;
use super::inventory_build::Assembly;
use super::inventory_model::DependencyPremiseKind;
use super::{AT, AffineMap, EntryIndex, check, invalid, syntax};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{LogicalId, size};
use crate::model::{AtomicSort, Operator, Sort};

impl Assembly<'_> {
    /// Compose the demanded output map before identifying inputs at each named base.
    pub(super) fn trace(&mut self, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        let count = syntax::coordinates(&self.cas.state.expression(root)?.sort)?;
        let mut demand = self
            .meter
            .run(budget, |budget| AffineMap::identity(count, budget))?;
        let mut current = root;
        let mut depth = 0_u64;
        loop {
            depth = depth
                .checked_add(1)
                .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
            budget.check(BudgetKind::Depth, depth, AT)?;
            budget.work(1, AT)?;
            let value = self.cas.state.expression(current)?;
            if !value.scope.is_empty() || value.space.is_some() {
                return Err(invalid(InvalidCode::ReadingDefinition));
            }
            if matches!(
                value.operator,
                Operator::Free(_) | Operator::AffineReading(_)
            ) {
                self.read_base(current, &demand, budget)?;
            }
            if matches!(value.operator, Operator::Free(_)) {
                let node = index(self.assigned, current, budget)?;
                return self.premise(node, DependencyPremiseKind::Family, budget);
            }
            if matches!(value.operator, Operator::Rational(_)) {
                return Ok(());
            }
            let operand = source_operand(&value.operator)?;
            let source = *value
                .operands
                .get(operand)
                .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))?;
            demand = self.advance(current, source, &demand, budget)?;
            current = source;
        }
    }

    /// Preserve the live demand map while constructing and composing the next exact step.
    fn advance(
        &self,
        current: LogicalId,
        source: LogicalId,
        demand: &AffineMap,
        budget: &mut Budget,
    ) -> Result<AffineMap, Fault> {
        if let Operator::AffineReading(map) = &self.cas.state.expression(current)?.operator {
            return self.meter.run(budget, |budget| demand.compose(map, budget));
        }
        self.meter.run(budget, |budget| {
            let bytes = demand.validate(budget)?;
            let previous = budget.reserve_temporary(bytes, AT)?;
            let next = (|| {
                let count = syntax::coordinates(&self.cas.state.expression(source)?.sort)?;
                let identity = AffineMap::identity(count, budget)?;
                let empty_transpose = count == 0
                    && matches!(
                        self.cas.state.expression(current)?.operator,
                        Operator::Transpose
                    );
                if empty_transpose {
                    return Ok(identity);
                }
                check::apply(&self.cas.state, current, &identity, budget)
            })();
            budget.restore_limits(previous);
            demand.compose(&next?, budget)
        })
    }

    /// A declared scalar is one entry; its Real or Complex coordinates share that entry.
    fn read_base(
        &mut self,
        id: LogicalId,
        demand: &AffineMap,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let bytes = self.meter.run(budget, |budget| demand.validate(budget))?;
        let previous = budget.reserve_temporary(bytes, AT)?;
        let result = self.read_base_reserved(id, demand, budget);
        budget.restore_limits(previous);
        result
    }

    /// Nonzero composed coefficients identify reads; offsets contain no heap entries.
    fn read_base_reserved(
        &mut self,
        id: LogicalId,
        demand: &AffineMap,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let base = self.ensure_base(id, budget)?;
        let value = self.cas.state.expression(id)?;
        let scalar = matches!(value.sort, Sort::Value(AtomicSort::Scalar(_)));
        let rows = if scalar {
            1
        } else {
            super::selection::rectangle(&value.sort)?.1
        };
        budget.work(
            u64::from(demand.inputs()).saturating_mul(u64::from(demand.outputs())),
            AT,
        )?;
        for column in 0..demand.inputs() {
            let mut read = false;
            for row in 0..demand.outputs() {
                read |= !demand.coefficient(row, column)?.is_zero();
            }
            if read {
                let entry = if scalar {
                    EntryIndex { row: 0, column: 0 }
                } else {
                    EntryIndex {
                        row: column
                            .checked_rem(rows)
                            .ok_or_else(|| invalid(InvalidCode::SelectionShape))?,
                        column: column
                            .checked_div(rows)
                            .ok_or_else(|| invalid(InvalidCode::SelectionShape))?,
                    }
                };
                let retained = &mut self
                    .bases
                    .get_mut(base)
                    .ok_or_else(|| invalid(InvalidCode::Reference))?
                    .value
                    .entries;
                budget.work(size(retained.len(), AT)?, AT)?;
                if !retained.contains(&entry) {
                    self.meter.push(retained, entry, budget)?;
                }
            }
        }
        Ok(())
    }
}

/// Only the checked reading fragment supplies exact affine coordinate transport.
const fn source_operand(operator: &Operator) -> Result<usize, Fault> {
    match operator {
        Operator::AffineReading(_) => Ok(1),
        Operator::Select(_)
        | Operator::RealPart
        | Operator::ImagPart
        | Operator::Conjugate
        | Operator::Transpose => Ok(0),
        _ => Err(invalid(InvalidCode::ReadingDefinition)),
    }
}
