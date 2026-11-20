// SPDX-License-Identifier: AGPL-3.0-only
//! A movable execution identity keeps the original cumulative ledger private.

use super::LoanError;
use nrz_bindings::{Budget, Limits, readings::algebra::limits::BudgetUsage};

/// The original execution ledger, moved once and never reset by a continuation.
#[derive(Debug)]
pub struct ExecutionAccount {
    /// Ownership identity, independent of observation incarnations.
    pub(crate) identity: u64,
    /// The caller's original ledger, including prior cumulative usage.
    pub(crate) ledger: Budget,
    /// Lazy account-resident delivery reservation; no graph-owned edge can erase it.
    pub(in crate::loan) delivery: Option<std::rc::Rc<std::cell::Cell<usize>>>,
}

impl ExecutionAccount {
    /// Move an existing ledger into its execution owner.
    /// # Errors
    /// Refuses identity exhaustion without constructing a continuation.
    pub fn new(ledger: Budget) -> Result<Self, LoanError> {
        Ok(Self {
            identity: crate::heap::identity::identity()?,
            ledger,
            delivery: None,
        })
    }

    /// Original cumulative usage, including work preceding account creation.
    #[must_use]
    pub const fn usage(&self) -> BudgetUsage {
        self.ledger.usage()
    }

    /// Immutable effective limits; no continuation can replenish its allowance.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        self.ledger.limits()
    }

    /// Authenticate a continuation before charging or accessing its storage.
    pub(super) const fn authenticate(&self, identity: u64) -> Result<(), LoanError> {
        if self.identity == identity {
            Ok(())
        } else {
            Err(LoanError::ForeignAccount)
        }
    }
}
