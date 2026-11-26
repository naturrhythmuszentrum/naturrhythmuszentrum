// SPDX-License-Identifier: AGPL-3.0-only
//! Complete account-scope storage is admitted before the caller's native records move.

pub(super) mod plan;
use super::super::{
    Arguments, Reservation, sink,
    walk::{Admission, Core, Error, Kind},
};
use super::{
    node::Node,
    owner::{Backing, Borrowed, Capacities, Owner},
    scope::Boundary,
};
use crate::loan::{preparation::Preparation, registry::CleanupReceipt};
use crate::{ExecutionAccount, Heap, LcInputInventory, LoanError};
use nrz_bindings::Registry;
use nrz_scalar::Scalar;
use std::{mem::MaybeUninit, rc::Rc};

/// A refusal returns the identical native capabilities, with completed Work left spent.
#[derive(Debug)]
pub struct Refusal<'i, P: Scalar, const N: usize> {
    /// Original declarations, checked request parts and selected replay borrows.
    pub nodes: Vec<Node<'i, P, N>>,
    /// Typed storage, ownership or fixed-topology refusal.
    pub error: Error,
}

impl<'i, P: Scalar + 'static, const N: usize> Owner<'i, P, N> {
    /// Publish the complete private owner only after all storage, topology and sink gates pass.
    #[expect(
        clippy::too_many_arguments,
        reason = "all original borrowed authority and caller-selected capacities are explicit"
    )]
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn admit(
        mut nodes: Vec<Node<'i, P, N>>,
        attempts: &[u64],
        capacity: Capacities,
        inventory: &'i LcInputInventory<P, N>,
        registry: &'i Registry,
        prior: &[Borrowed<'_>],
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, Refusal<'i, P, N>> {
        let prepared = prepare(
            &nodes,
            nodes.capacity(),
            attempts,
            capacity,
            inventory,
            prior,
            heap,
            account,
        );
        let Prepared {
            mut records,
            boundaries,
            core,
            sink,
            bytes,
            gate,
            header,
            cleanup,
        } = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Err(Refusal { nodes, error }),
        };
        let positions = positions(&mut nodes, attempts);
        if let Err(error) = positions {
            return Err(Refusal {
                nodes,
                error: error.into(),
            });
        }
        records.extend(nodes);
        Ok(Self {
            backing: Box::write(
                header,
                Backing {
                    nodes: records,
                    boundaries,
                    inventory,
                    registry,
                    account: account.identity,
                    heap: heap.scope,
                    bytes,
                    core,
                    sink,
                    gate,
                    cleanup,
                },
            ),
        })
    }
}

/// Every returned allocation was counted in the simultaneous plan before acquisition.
struct Prepared<'i, P: Scalar, const N: usize> {
    /// Empty fixed native slots acquire originals only after every fallible operation.
    records: Vec<Node<'i, P, N>>,
    /// Fixed scope-boundary stamps.
    boundaries: Vec<Boundary>,
    /// Independently guarded graph core.
    core: Core,
    /// Independently guarded bounded evidence storage.
    sink: sink::Sink,
    /// Disjoint adapter allocation subtotal.
    bytes: usize,
    /// Fixed observer with no graph backedge.
    gate: Rc<super::gate::Gate>,
    /// Allocated header remains uninitialized until originals move without further failure.
    header: Box<MaybeUninit<Backing<'i, P, N>>>,
    /// Only successfully acquired surviving storage receives this entitlement.
    cleanup: CleanupReceipt,
}

/// Private temporary topology and codec records are destroyed before the final publication.
#[expect(
    clippy::too_many_arguments,
    reason = "preflight keeps the same explicit original account and borrowed inventory"
)]
fn prepare<'i, P: Scalar + 'static, const N: usize>(
    nodes: &[Node<'i, P, N>],
    original_capacity: usize,
    attempts: &[u64],
    capacity: Capacities,
    inventory: &'i LcInputInventory<P, N>,
    prior: &[Borrowed<'_>],
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result<Prepared<'i, P, N>, Error> {
    let plan = plan::Plan::new(
        nodes,
        original_capacity,
        attempts,
        capacity,
        inventory,
        prior,
        heap,
        account,
    )?;
    let identity = crate::heap::identity::identity().map_err(LoanError::from)?;
    let mut preparation = Preparation::new(CleanupReceipt {
        identity,
        allocations: plan.allocations,
        records: 1,
        edges: nodes.len(),
    });
    let records = buffer(capacity.nodes, false, &mut preparation)?;
    let mut boundaries = buffer(capacity.nodes, false, &mut preparation)?;
    boundaries.resize(capacity.nodes, Boundary::default());
    let mut kinds = buffer(nodes.len(), true, &mut preparation)?;
    kinds.resize(nodes.len(), Kind::Native);
    let edges = plan::connections(nodes, capacity.edges)?;
    if capacity.edges != 0 {
        preparation.acquired(true)?;
    }
    let reservation = Reservation::reserve::<P>(
        Arguments {
            nodes: capacity.nodes,
            edges: capacity.edges,
            proposed_edges: edges.len(),
            scopes: capacity.scopes,
            drive: capacity.drive,
            attempts,
        },
        &[],
        account,
    )?;
    let mut core = match Core::admit(reservation, &kinds, &edges, account) {
        Admission::Admitted(core) => core,
        Admission::OwnershipWait(_) => {
            return Err(LoanError::Heap(crate::HeapError::OwnershipPending).into());
        }
        Admission::Refused(error, _) => return Err(error),
    };
    for (slot, node) in nodes.iter().enumerate() {
        if matches!(node.contract, super::node::Contract::Input(_)) {
            let handle = core.handle(slot, account)?;
            core.park(handle, account)?;
        }
    }
    let destinations = plan::destinations(core.reservation.backing.identity, attempts)?;
    if !nodes.is_empty() {
        preparation.acquired(true)?;
    }
    let sink = sink::Sink::admit(
        &destinations,
        plan.sink,
        &[core.reservation.borrowed()],
        account,
    )
    .map_err(Error::Delivery)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let gate = Rc::new(super::gate::Gate::new(account));
    preparation.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let header = Box::new_uninit();
    preparation.acquired(false)?;
    let cleanup = preparation.finish()?;
    Ok(Prepared {
        records,
        boundaries,
        core,
        sink,
        bytes: plan.bytes,
        gate,
        header,
        cleanup,
    })
}

/// Position arithmetic uses the original checked attempt counts without a truncating fallback.
fn positions<P: Scalar, const N: usize>(
    nodes: &mut [Node<'_, P, N>],
    attempts: &[u64],
) -> Result<(), LoanError> {
    let mut base = nodes.len();
    for (node, attempts) in nodes.iter_mut().zip(attempts) {
        let count = usize::try_from(*attempts).map_err(|_| LoanError::CountOverflow)?;
        let next = base
            .checked_add(count)
            .and_then(|n| n.checked_add(1))
            .ok_or(LoanError::CountOverflow)?;
        node.probe_base = base;
        base = next;
    }
    Ok(())
}

/// Acquired-prefix accounting follows each exact-capacity allocation, including temporary owners.
fn buffer<T>(
    capacity: usize,
    temporary: bool,
    preparation: &mut Preparation,
) -> Result<Vec<T>, LoanError> {
    let values = crate::loan::vector(capacity)?;
    if capacity != 0 {
        preparation.acquired(temporary)?;
    }
    Ok(values)
}
