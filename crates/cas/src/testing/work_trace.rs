// SPDX-License-Identifier: AGPL-3.0-only
//! Test-only attribution observes charged Work without changing the ledger or proof.

use std::cell::{Cell, RefCell};

use crate::error::{Fault, InvalidCode, Location};
use crate::testing::common::TestResult;

/// An exclusive accounting bucket; nested scopes replace, rather than duplicate, charges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    /// Primitive key, or the surrounding replay operation.
    pub(crate) rule: &'static str,
    /// Mathematical obligation or nonmathematical reconstruction phase.
    pub(crate) phase: &'static str,
    /// Original charged operation, independent of the attribution scope.
    pub(crate) location: &'static str,
    /// Successfully consumed Work in this bucket.
    pub(crate) work: u64,
}

thread_local! {
    static CURRENT: Cell<(&'static str, &'static str)> = const { Cell::new(("replay", "overhead")) };
    static ROWS: RefCell<Option<Vec<Row>>> = const { RefCell::new(None) };
}

/// Restore the containing attribution on every ordinary return, including refusal.
pub(crate) struct Scope {
    /// Enclosing primitive and phase.
    previous: (&'static str, &'static str),
}

impl Scope {
    /// Enter one primitive's complete validation boundary.
    pub(crate) fn rule(rule: &'static str) -> Self {
        Self {
            previous: CURRENT.with(|current| current.replace((rule, "validation"))),
        }
    }

    /// Attribute a suboperation to its innermost mathematical purpose.
    pub(crate) fn phase(phase: &'static str) -> Self {
        Self {
            previous: CURRENT.with(|current| current.replace((current.get().0, phase))),
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.previous));
    }
}

/// Observation failures are test harness failures, never new mathematical claims.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, Location::new("test Work attribution"))
}

/// Observe only a successful charge; exhaustion itself consumes no additional Work.
pub(crate) fn record(amount: u64, at: Location) -> Result<(), Fault> {
    ROWS.with(|cell| {
        let mut active = cell.try_borrow_mut().map_err(|_| invalid())?;
        let Some(rows) = active.as_mut() else {
            return Ok(());
        };
        let (rule, phase) = CURRENT.with(Cell::get);
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.rule == rule && row.phase == phase && row.location == at.operation)
        {
            row.work = row.work.checked_add(amount).ok_or_else(invalid)?;
        } else {
            rows.try_reserve(1).map_err(|_| invalid())?;
            rows.push(Row {
                rule,
                phase,
                location: at.operation,
                work: amount,
            });
        }
        Ok(())
    })
}

/// Keep refusal results and their consumed charges, then disarm the thread-local observer.
pub(crate) fn measured<T>(run: impl FnOnce() -> T) -> TestResult<(T, Vec<Row>)> {
    ROWS.with(|cell| -> TestResult {
        let mut active = cell.try_borrow_mut()?;
        if active.is_some() {
            return Err("nested Work observation".into());
        }
        *active = Some(Vec::new());
        Ok(())
    })?;
    let result = run();
    let mut rows = ROWS.with(|cell| -> TestResult<Vec<Row>> {
        cell.try_borrow_mut()?
            .take()
            .ok_or_else(|| "inactive Work observation".into())
    })?;
    rows.sort_by_key(|row| (row.rule, row.phase, row.location));
    Ok((result, rows))
}
