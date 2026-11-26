// SPDX-License-Identifier: AGPL-3.0-only
//! One bounded owner retains originals, native continuations and its independent evidence sink.

use super::super::{Borrowed as CoreBorrowed, sink::Sink, walk::Core};
use super::{node::Node, scope::Boundary};
use crate::LcInputInventory;
use nrz_bindings::Registry;
use nrz_scalar::Scalar;

/// Caller-selected extents are separate from native invocation limits and original Work.
#[derive(Clone, Copy, Debug)]
pub struct Capacities {
    /// Reserved native and walk node slots, including vacancies.
    pub nodes: usize,
    /// Exactly the fixed proposed dependency count.
    pub edges: usize,
    /// Maximum retained notification scopes.
    pub scopes: usize,
    /// Original nonreplenishing graph traversal count.
    pub drive: u64,
    /// Complete fixed evidence record extent.
    pub records: usize,
    /// Complete fixed evidence-word extent.
    pub words: usize,
    /// Maximum simultaneously escaped scope/outcome witnesses.
    pub witnesses: usize,
}

/// No clone, serialized constructor or mutable record access can reconstruct execution authority.
#[derive(Debug)]
pub struct Owner<'i, P: Scalar, const N: usize> {
    /// The fixed adapter header is an explicitly admitted allocation.
    pub(super) backing: Box<Backing<'i, P, N>>,
}

/// Field order destroys native continuations before graph reservation and sink status.
#[derive(Debug)]
pub(super) struct Backing<'i, P: Scalar, const N: usize> {
    /// Bounded native phase records, including their reserved vacant capacity.
    pub(super) nodes: Vec<Node<'i, P, N>>,
    /// Fixed boundary stamps for the serialized scope closure operation.
    pub(super) boundaries: Vec<Boundary>,
    /// Original inventory remains borrowed through every surviving native continuation.
    pub(super) inventory: &'i LcInputInventory<P, N>,
    /// Original semantic registry is never copied or reconstructed.
    pub(super) registry: &'i Registry,
    /// Original execution account is authenticated before all graph access.
    pub(super) account: u64,
    /// Original heap identity prevents cross-heap graph operations.
    pub(super) heap: u64,
    /// Exact adapter allocation inventory, separate from the walk and sink allocations.
    pub(super) bytes: usize,
    /// The existing bounded incidence/readiness/notification mechanism.
    pub(super) core: Core,
    /// Fixed native preflight observer has no backedge to this owner.
    pub(super) gate: std::rc::Rc<super::gate::Gate>,
    /// Bounded history survives independently through its admitted witnesses.
    pub(super) sink: Sink,
    /// Acquired adapter storage retains its own finite prepaid cleanup entitlement.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the owning receipt retains the original prepaid destruction entitlement"
        )
    )]
    pub(super) cleanup: crate::loan::registry::CleanupReceipt,
}

/// Borrowed overlap evidence does not grant mutation, numerical access or ownership reconstruction.
#[derive(Clone, Copy, Debug)]
pub struct Borrowed<'a> {
    /// Original walk allocation identity and actual capacity projection.
    pub(super) core: CoreBorrowed<'a>,
    /// Exact independently owned adapter allocation bytes.
    pub(super) bytes: usize,
    /// Original account identity.
    pub(super) account: u64,
    /// Borrowed original phase observations; no owner is cloned or erased into numerical authority.
    pub(super) exact: &'a dyn super::exact::Source,
    /// Borrowed observer latches an actual original Work stop during a scoped report.
    pub(super) gate: &'a super::gate::Gate,
}
impl<P: Scalar, const N: usize> Owner<'_, P, N> {
    /// An admitted owner lends immutable identity and actual allocation facts for overlap.
    #[must_use]
    pub fn borrowed(&self) -> Borrowed<'_> {
        Borrowed {
            core: self.backing.core.reservation.borrowed(),
            bytes: self.backing.bytes,
            account: self.backing.account,
            exact: self,
            gate: &self.backing.gate,
        }
    }
}

impl<P: Scalar, const N: usize> Owner<'_, P, N> {
    /// Authenticate the original account, heap and graph occupation before native access.
    pub(super) fn authenticate(
        &self,
        handle: super::super::walk::Handle,
        heap: &crate::Heap,
        account: &mut crate::ExecutionAccount,
    ) -> Result<(), super::super::walk::Error> {
        let result = (|| {
            account.authenticate(self.backing.account)?;
            if self.backing.gate.stopped() {
                return Err(super::super::walk::Error::WorkStopped);
            }
            crate::loan::charge(&mut account.ledger, 2)?; // H operation entry, A graph and heap scope.
            if heap.scope != self.backing.heap
                || handle.owner != self.backing.core.reservation.backing.identity
            {
                return Err(crate::LoanError::ForeignOwner.into());
            }
            let original = self.backing.core.handle(handle.slot, account)?;
            if original != handle {
                return Err(crate::LoanError::StaleOccupation.into());
            }
            Ok(())
        })();
        self.backing.gate.observe(&result);
        result
    }
}

impl<P: Scalar, const N: usize> Owner<'_, P, N> {
    /// The named simultaneous graph scope is separate from the native heap inventory.
    pub(super) fn graph_bytes(
        &self,
        others: &[Borrowed<'_>],
        account: &mut crate::ExecutionAccount,
    ) -> Result<usize, super::super::walk::Error> {
        account.authenticate(self.backing.account)?;
        let prior = super::admission::plan::prior_bytes_excluding(
            others,
            Some(self.backing.core.reservation.borrowed()),
            account,
        )?;
        let core =
            super::super::Inventory::inspect(&[self.backing.core.reservation.borrowed()], account)?;
        let add = super::super::inventory::add;
        Ok(add(
            add(prior, core.covered_bytes)?,
            add(self.backing.bytes, super::super::sink::live_bytes(account)?)?,
        )?)
    }
}

impl<P: Scalar, const N: usize> Owner<'_, P, N> {
    /// Compose justified exact partitions with graph storage; native heap bytes remain separate.
    pub(super) fn resident_bytes(
        &self,
        others: &[Borrowed<'_>],
        heap: &crate::Heap,
        account: &mut crate::ExecutionAccount,
    ) -> Result<(usize, u64), super::super::walk::Error> {
        let graph = self.graph_bytes(others, account)?;
        let (exact, overlap) = super::exact::Scope {
            current: Some(self),
            prior: others,
            heap,
        }
        .reservation(account)?;
        Ok((
            super::super::inventory::add(graph, exact.covered()?)?,
            overlap,
        ))
    }
}
