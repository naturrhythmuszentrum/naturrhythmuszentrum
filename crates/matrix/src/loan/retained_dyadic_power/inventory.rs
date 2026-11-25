// SPDX-License-Identifier: AGPL-3.0-only
//! Charged observation separates shared header, CAS evidence, retained key buffers and uses.

use super::{RetainedDyadicPower, retention::authenticate};
use crate::{
    Heap,
    loan::{
        ExecutionAccount, LoanError, charge,
        registry::{AllocationKind, DebitOwner},
    },
};

/// One charged snapshot creates no new use or authority edge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetainedDyadicPowerInventory {
    /// Runtime identity shared by all funded uses of this owner.
    pub owner_identity: u64,
    /// Identity of the exact use supplied for inspection.
    pub use_identity: u64,
    /// Aligned unique shared owner header, including inline CAS and native program.
    pub owner_bytes: usize,
    /// CAS-reported conservative logical checked-evidence allowance.
    pub evidence_bytes: usize,
    /// Retained proof and private key backing capacities, each counted once.
    pub key_bytes: usize,
    /// Complete logical producing graph inventory in its separate CAS scope.
    pub graph_bytes: u64,
    /// All live or conservatively expired use-header allocations.
    pub use_bytes: usize,
    /// Number of funded use debits still awaiting reconciliation.
    pub uses: usize,
}

impl Heap {
    /// Inspect this exact owner and its individual use debits without acquiring another edge.
    /// # Errors
    /// Refuses foreign account/heap, invalid witnesses, overflow or exhausted Work.
    pub fn retained_dyadic_power_inventory(
        &self,
        law: &RetainedDyadicPower,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedDyadicPowerInventory, LoanError> {
        let owner = law.owner()?;
        account.authenticate(owner.identity.account)?;
        account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "dyadic power law inventory",
            |budget| {
                charge(budget, 4)?;
                authenticate(self, law)?;
                let mut result = RetainedDyadicPowerInventory {
                    owner_identity: owner.identity.owner,
                    use_identity: law.record.token.key,
                    graph_bytes: law.frozen()?.graph_bytes,
                    ..RetainedDyadicPowerInventory::default()
                };
                for entry in &self.loans.debits {
                    charge(budget, 1)?;
                    if let Some(debit) = entry
                        && debit.owner == DebitOwner::Dyadic(owner.identity)
                    {
                        match &debit.kind {
                            AllocationKind::DyadicOwner { allowance, .. } => {
                                result.evidence_bytes = owner.evidence;
                                result.key_bytes = owner.keys;
                                result.owner_bytes = debit
                                    .bytes
                                    .checked_sub(*allowance)
                                    .ok_or(LoanError::ByteOverflow)?;
                            }
                            AllocationKind::DyadicUse(_) => {
                                result.use_bytes = result
                                    .use_bytes
                                    .checked_add(debit.bytes)
                                    .ok_or(LoanError::ByteOverflow)?;
                                result.uses =
                                    result.uses.checked_add(1).ok_or(LoanError::CountOverflow)?;
                            }
                            AllocationKind::Numerical
                            | AllocationKind::Tracking
                            | AllocationKind::Owner
                            | AllocationKind::Pin
                            | AllocationKind::Filled
                            | AllocationKind::PreparedOwner { .. }
                            | AllocationKind::PreparedUse(_)
                            | AllocationKind::Constant(_)
                            | AllocationKind::LcPending { .. }
                            | AllocationKind::LcClosure(_)
                            | AllocationKind::LcRows(_)
                            | AllocationKind::LcBundle(_)
                            | AllocationKind::LcUse { .. } => return Err(LoanError::WrongLoan),
                        }
                    }
                }
                Ok(result)
            },
        )
    }
}
