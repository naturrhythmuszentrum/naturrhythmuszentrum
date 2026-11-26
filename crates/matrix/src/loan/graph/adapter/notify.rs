// SPDX-License-Identifier: AGPL-3.0-only
//! Notification closes only no-loan expectations inside the retained ownership closure.

use super::{
    node::{Contract, State},
    owner::Owner,
    scope::{self, Progress, Scope},
};
use crate::loan::graph::{
    sink::codec::{Encoding, Evidence, Outcome},
    walk::{Error, Handle},
};
use crate::{ExecutionAccount, Heap};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static, const N: usize> Owner<'_, P, N> {
    /// The original fixed topology supplies the owned upstream closure of these roots.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn begin_scope(
        &mut self,
        roots: &[Handle],
        owned: &[Handle],
        account: &mut ExecutionAccount,
    ) -> Result<Scope, Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.begin_scope_inner(roots, owned, account);
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn begin_scope_inner(
        &mut self,
        roots: &[Handle],
        owned: &[Handle],
        account: &mut ExecutionAccount,
    ) -> Result<Scope, Error> {
        scope::begin(
            &mut self.backing.core,
            &mut self.backing.boundaries,
            roots,
            owned,
            account,
        )
    }

    /// Each step preserves the admitted generation and the shared stack's saved position.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn close_scope(
        &mut self,
        scope: Scope,
        account: &mut ExecutionAccount,
    ) -> Result<Progress, Error> {
        let result = (|| {
            account.authenticate(self.backing.account)?;
            if self.backing.gate.stopped() {
                return Err(Error::WorkStopped);
            }

            scope::step(
                &mut self.backing.core,
                &self.backing.boundaries,
                scope,
                account,
            )
        })();
        self.backing.gate.observe(&result);
        result
    }

    /// One bounded position closes no external producer and never cancels an issued host loan.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn notify_one(
        &mut self,
        scope: Scope,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Progress, Error> {
        account.authenticate(self.backing.account)?;
        if self.backing.gate.stopped() {
            return Err(Error::WorkStopped);
        }
        let result = self.notify_one_inner(scope, heap, account);
        self.backing.gate.observe(&result);
        self.backing.core.record_external(&result);
        result
    }

    /// Preserve native ownership across this operation's independently fallible phases.
    fn notify_one_inner(
        &mut self,
        scope: Scope,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Progress, Error> {
        if heap.scope != self.backing.heap {
            return Err(crate::LoanError::ForeignOwner.into());
        }
        let Some((slot, owned)) = scope::next(&self.backing.core, scope, account)? else {
            return Ok(Progress::Published);
        };
        crate::loan::charge(&mut account.ledger, 1)?; // N exact admitted expectation and current state.
        let node = &self.backing.nodes[slot];
        let expectation = matches!(node.contract, Contract::Input(_));
        let unissued = matches!(
            node.state,
            State::Pending | State::ResourceWait | State::OwnershipWait | State::Repair
        );
        if owned && expectation && unissued {
            let prepared = self
                .backing
                .sink
                .prepare(slot, Encoding::Outcome, account)
                .map_err(Error::Delivery)?;
            scope::prepay_next(&self.backing.core, account)?;
            crate::loan::charge(&mut account.ledger, 2)?; // N closed expectation and saved evidence.
            let evidence = prepared
                .commit(Evidence::Outcome(Outcome::Absent))
                .map_err(|(_, error)| Error::Delivery(error))?;
            let node = &mut self.backing.nodes[slot];
            node.contract = Contract::Input(None);
            node.state = State::Absent;
            node.evidence = Some(evidence);
        } else {
            scope::prepay_next(&self.backing.core, account)?;
        }
        let node = &self.backing.nodes[slot];
        let unfinished = self.backing.core.reservation.backing.nodes[slot].flags
            & crate::loan::graph::walk::control::COMPLETE
            == 0;
        if owned && expectation && node.state == State::Absent && unfinished {
            let handle = self.backing.core.handle(slot, account)?;
            self.backing.core.resume(handle, account)?;
        }
        scope::commit_next(&mut self.backing.core, scope)?;
        Ok(Progress::Pending)
    }
}
