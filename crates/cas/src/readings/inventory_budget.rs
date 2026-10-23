// SPDX-License-Identifier: AGPL-3.0-only
//! Retained inventory data reduces the temporary allowance of every nested traversal.

use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::size;
use crate::wire_build::{Assignments, Key};

/// Conservative simultaneous payload and monotone copy accounting for one inventory.
#[derive(Default)]
pub(super) struct Meter {
    /// Already retained output and lookup storage.
    bytes: u64,
}

impl Meter {
    /// Charge before copying or retaining another payload or index record.
    pub(super) fn keep(&mut self, bytes: u64, budget: &mut Budget) -> Result<(), Fault> {
        let total = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
        budget.check(BudgetKind::OutputBytes, total, AT)?;
        budget.work(bytes, AT)?;
        self.bytes = total;
        Ok(())
    }

    /// Nested builders share the remaining payload ceiling and the original Work ledger.
    pub(super) fn run<T>(
        &self,
        budget: &mut Budget,
        action: impl FnOnce(&mut Budget) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        let previous = budget.reserve_temporary(self.bytes, AT)?;
        let result = action(budget);
        budget.restore_limits(previous);
        result
    }

    /// Bound a growable record vector before its next fallible reservation.
    pub(super) fn push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        self.keep(size(std::mem::size_of::<T>(), AT)?, budget)?;
        values
            .try_reserve_exact(1)
            .map_err(|_| Fault::allocation(AT))?;
        values.push(value);
        Ok(())
    }
}

/// Account for the already budgeted canonical table while additional inventory views live.
pub(super) fn table_meter(
    records: &[Vec<u8>],
    assigned: &Assignments,
    budget: &mut Budget,
) -> Result<Meter, Fault> {
    let mut meter = Meter::default();
    for record in records {
        meter.keep(size(record.len(), AT)?.saturating_add(32), budget)?;
    }
    for (key, (_, descriptor)) in assigned {
        let key_bytes = match key {
            Key::Object(_) => 32,
            Key::Guard(bytes) | Key::Statement(bytes) | Key::Conclusion(bytes) => {
                size(bytes.len(), AT)?
            }
        };
        meter.keep(
            size(descriptor.len(), AT)?
                .saturating_add(key_bytes)
                .saturating_add(128),
            budget,
        )?;
    }
    Ok(meter)
}

/// Resolve canonical record indices without depending on local allocation order.
pub(super) fn index(
    assigned: &Assignments,
    id: crate::graph::LogicalId,
    budget: &mut Budget,
) -> Result<u32, Fault> {
    budget.work(size(assigned.len(), AT)?, AT)?;
    assigned
        .get(&Key::Object(id))
        .map(|(index, _)| *index)
        .ok_or_else(|| super::invalid(crate::error::InvalidCode::Reference))
}
