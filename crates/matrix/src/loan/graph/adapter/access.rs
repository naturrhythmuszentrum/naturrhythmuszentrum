// SPDX-License-Identifier: AGPL-3.0-only
//! Original graph occupations authenticate delivery, repair and sealed result access.

use super::{
    native::Native,
    node::{Contract, State},
    owner::Owner,
};
use crate::loan::graph::{
    sink::{self, delivery::Pull, witness::Witness},
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap, LcOmegaOutput, LcPublishedResult, LoanError};
use nrz_scalar::Scalar;

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Borrow the original exact-owner scopes without claiming their unavailable physical capacities.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn originals(
        &self,
        account: &mut ExecutionAccount,
    ) -> Result<crate::loan::lc_shape::exact_scope::ExactScope<'i, P, N>, Error> {
        let result = (|| {
            account.authenticate(self.backing.account)?;
            crate::loan::charge(&mut account.ledger, 1)?; // H original inventory and Registry borrows.
            Ok(crate::loan::lc_shape::exact_scope::ExactScope::new(
                self.backing.inventory,
                self.backing.registry,
            ))
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// Borrow a handle only from the live original occupation.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn handle(&self, slot: usize, account: &mut ExecutionAccount) -> Result<Handle, Error> {
        let result = self.backing.core.handle(slot, account);
        self.backing.gate.observe(&result);
        result
    }

    /// Inspect the admitted lifecycle disposition without gaining numerical or repair authority.
    /// # Errors
    /// Refuses foreign or stale occupations and preserves original Work exhaustion.
    pub fn state(
        &self,
        handle: Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<State, Error> {
        let result = (|| {
            self.authenticate(handle, heap, account)?;
            crate::loan::charge(&mut account.ledger, 1)?; // N original phase/outcome record.
            Ok(self.backing.nodes[handle.slot].state)
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// Receipt words remain protected by the original bounded sink borrow.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn pull(
        &self,
        receipt: sink::Handle,
        account: &mut ExecutionAccount,
    ) -> Result<Pull<'_>, Error> {
        let result = self
            .backing
            .sink
            .pull(receipt, account)
            .map_err(Error::Delivery);
        self.backing.gate.observe(&result);
        result
    }

    /// An escaped evidence edge preserves abandonment status but grants no numerical access.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn witness(
        &self,
        receipt: sink::Handle,
        account: &mut ExecutionAccount,
    ) -> Result<Witness, Error> {
        let result = self
            .backing
            .sink
            .witness(receipt, account)
            .map_err(Error::Delivery);
        self.backing.gate.observe(&result);
        result
    }

    /// Probe acknowledgements release only the named original sink obligation.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn acknowledge_probe(
        &self,
        receipt: sink::Handle,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        if receipt.slot < self.backing.nodes.len() {
            return Err(Error::InvalidPhase);
        }
        let result = self
            .backing
            .sink
            .acknowledge(receipt, account)
            .map_err(Error::Delivery);
        self.backing.gate.observe(&result);
        result
    }

    /// A saved sink acknowledgement survives interruption before the walk's separate release gate.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn acknowledge_outcome(
        &mut self,
        handle: Handle,
        receipt: sink::Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let result = (|| {
            self.authenticate(handle, heap, account)?;
            if self.backing.nodes[handle.slot].evidence != Some(receipt) {
                return Err(LoanError::WrongLoan.into());
            }
            crate::loan::charge(&mut account.ledger, 1)?; // N saved acknowledgement state.
            if !self.backing.nodes[handle.slot].evidence_acknowledged {
                let result = self
                    .backing
                    .sink
                    .acknowledge(receipt, account)
                    .map_err(Error::Delivery);
                self.backing.gate.observe(&result);
                result?;
                self.backing.nodes[handle.slot].evidence_acknowledged = true;
            }
            self.backing.core.acknowledge(handle, account)
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// A consuming-return record may be acknowledged before or after restoring its exact declaration.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn acknowledge_return(
        &mut self,
        handle: Handle,
        receipt: sink::Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), Error> {
        let result = (|| {
            self.authenticate(handle, heap, account)?;
            crate::loan::charge(&mut account.ledger, 1)?; // N exact returned-record association.
            if self.backing.nodes[handle.slot].returned_evidence != Some(receipt) {
                return Err(LoanError::WrongLoan.into());
            }
            let result = self
                .backing
                .sink
                .acknowledge(receipt, account)
                .map_err(Error::Delivery);
            self.backing.gate.observe(&result);
            result?;
            self.backing.nodes[handle.slot].returned_evidence = None;
            Ok(())
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// Only the existing sealed delivery result supplies numerical reader and retained-use APIs.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn result(
        &self,
        handle: Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<&LcPublishedResult<P>, Error> {
        let result = (|| {
            self.authenticate(handle, heap, account)?;
            crate::loan::charge(&mut account.ledger, 1)?; // N admitted delivery and actual sealed phase.
            let node = &self.backing.nodes[handle.slot];
            if !matches!(node.contract, Contract::Delivery { .. }) || node.state != State::Published
            {
                return Err(Error::InvalidPhase);
            }
            let Some(Native::OmegaPublished(result)) = &node.native else {
                return Err(Error::InvalidPhase);
            };
            Ok(result)
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// A consuming retry must return the identical specialization and original inventory.
    #[expect(
        clippy::result_large_err,
        reason = "the consuming return preserves the identical declaration without a new allocation"
    )]
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn restore_preparation(
        &mut self,
        handle: Handle,
        declaration: LcOmegaOutput<'i, P, N>,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<(), (LcOmegaOutput<'i, P, N>, Error)> {
        let checked = self.authenticate(handle, heap, account).and_then(|()| {
            crate::loan::charge(&mut account.ledger, 3)?; // N continuation, A inventory and specialization.
            let node = &self.backing.nodes[handle.slot];
            let Contract::Omega { specialization, .. } = node.contract else {
                return Err(Error::InvalidPhase);
            };
            if node.state != State::Returned || node.native.is_some() {
                return Err(Error::InvalidPhase);
            }
            if specialization != declaration.specialization_key()
                || !std::ptr::eq(declaration.inventory(), self.backing.inventory)
            {
                return Err(LoanError::TargetMismatch.into());
            }
            self.backing.core.resume(handle, account)
        });
        self.backing.gate.observe(&checked);
        if let Err(error) = checked {
            return Err((declaration, error));
        }
        self.backing.nodes[handle.slot].native = Some(Native::OmegaDeclaration(declaration));
        self.backing.nodes[handle.slot].state = State::Pending;
        Ok(())
    }
}
