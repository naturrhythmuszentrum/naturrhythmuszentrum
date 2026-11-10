// SPDX-License-Identifier: AGPL-3.0-only
//! Observations separate measured wrapper layout, logical evidence and the retained graph scope.

use super::{RetainedExpm1, retention::authenticate};
use crate::{
    Heap,
    loan::{
        ExecutionAccount, LoanError, charge,
        registry::{AllocationKind, DebitOwner},
    },
};

/// An inventory snapshot creates no use edge and cannot reacquire a law.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetainedExpm1Inventory {
    /// Runtime owner identity shared by all its independently funded uses.
    pub owner_identity: u64,
    /// Exact identity of the use supplied to this observation.
    pub use_identity: u64,
    /// Aligned unique shared owner header, including inline CAS and native program.
    pub owner_bytes: usize,
    /// Conservative logical exact-evidence allowance, not a physical exact-number census.
    pub evidence_bytes: usize,
    /// Complete logical producing graph inventory, separately scoped by CAS.
    pub graph_bytes: u64,
    /// Live and conservatively expired individual use headers awaiting reconciliation.
    pub use_bytes: usize,
    /// Number of individually charged uses, including unreconciled expired records.
    pub uses: usize,
}
impl Heap {
    /// Inspect this exact owner and each charged use without acquiring another edge.
    /// # Errors
    /// Refuses foreign account/heap, invalid authority, arithmetic overflow or exhausted Work.
    pub fn retained_expm1_inventory(
        &self,
        law: &RetainedExpm1,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedExpm1Inventory, LoanError> {
        let owner = law.owner()?;
        account.authenticate(owner.identity.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "prepared law inventory", |budget| {
                charge(budget, 4)?;
                authenticate(self, law)?;
                let mut result = RetainedExpm1Inventory {
                    owner_identity: owner.identity.owner,
                    use_identity: law.record.token.key,
                    graph_bytes: law.frozen()?.graph_bytes,
                    ..RetainedExpm1Inventory::default()
                };
                for entry in &self.loans.debits {
                    charge(budget, 1)?;
                    if let Some(debit) = entry
                        && debit.owner == DebitOwner::Prepared(owner.identity)
                    {
                        match &debit.kind {
                            AllocationKind::PreparedOwner { evidence, .. } => {
                                result.evidence_bytes = *evidence;
                                result.owner_bytes = debit
                                    .bytes
                                    .checked_sub(*evidence)
                                    .ok_or(LoanError::ByteOverflow)?;
                            }
                            AllocationKind::PreparedUse(_) => {
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
                            | AllocationKind::Filled => return Err(LoanError::WrongLoan),
                        }
                    }
                }
                Ok(result)
            })
    }
}
