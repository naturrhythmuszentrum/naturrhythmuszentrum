// SPDX-License-Identifier: AGPL-3.0-only
//! Disjoint native record, fixed graph, temporary and sink capacities share one preflight.

use super::super::{
    native::Native,
    node::{Contract, Node},
    owner::{Backing, Borrowed, Capacities},
    scope::Boundary,
};
use crate::loan::{
    charge,
    graph::{
        Arguments, Inventory,
        inventory::{add, bytes},
        plan::Plan as CorePlan,
        sink,
        walk::{Connection, Error, Kind},
    },
};
use crate::{ExecutionAccount, Heap, LcInputInventory, LoanError};
use nrz_bindings::readings::algebra::limits::BudgetKind;
use nrz_scalar::Scalar;

/// Only adapter allocations enter this retained subtotal; other owners remain disjoint.
pub(super) struct Plan {
    /// Boxed header, native record capacity and scope-boundary capacity.
    pub(super) bytes: usize,
    /// Adapter-owned allocation boundaries, excluding independently guarded core and sink storage.
    pub(super) allocations: usize,
    /// Original caller-selected bounded evidence extents.
    pub(super) sink: sink::admission::Capacities,
}
impl Plan {
    /// Checked counts and complete overlap precede every newly acquired allocation.
    #[expect(
        clippy::too_many_arguments,
        reason = "the complete simultaneous scope keeps original authorities and capacities explicit"
    )]
    pub(super) fn new<P: Scalar + 'static, const N: usize>(
        nodes: &[Node<'_, P, N>],
        original_capacity: usize,
        attempts: &[u64],
        capacity: Capacities,
        inventory: &LcInputInventory<P, N>,
        prior: &[Borrowed<'_>],
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, Error> {
        if nodes.len() != attempts.len() || nodes.len() > capacity.nodes {
            return Err(LoanError::UnsupportedContract.into());
        }
        validate(nodes, inventory, heap, account)?;
        let sink = sink::admission::Capacities {
            records: capacity.records,
            words: capacity.words,
            witnesses: capacity.witnesses,
        };
        sink_extents(nodes.len(), attempts, capacity, account)?;
        let core = CorePlan::new(Arguments {
            nodes: capacity.nodes,
            edges: capacity.edges,
            proposed_edges: edge_count(nodes)?,
            scopes: capacity.scopes,
            drive: capacity.drive,
            attempts,
        })?;
        let adapter_bytes = add(
            add(
                size_of::<Backing<'_, P, N>>(),
                crate::loan::rc_bytes::<super::super::gate::Gate>(),
            )?,
            add(
                bytes::<Node<'_, P, N>>(capacity.nodes)?,
                bytes::<Boundary>(capacity.nodes)?,
            )?,
        )?;
        let previous = prior_bytes(prior, account)?;
        // Existing complete heap inventory keeps its native tariff and conservative debits.
        charge(
            &mut account.ledger,
            heap.resource_work().map_err(LoanError::from)?,
        )?;
        let native = heap.resource_bytes().map_err(LoanError::from)?;
        let proposed = super::super::exact::Nodes(nodes);
        let (exact, overlap) = super::super::exact::Scope {
            current: Some(&proposed),
            prior,
            heap,
        }
        .reservation(account)?;
        let native = add(native, exact.covered()?)?;
        let sink_bytes = sink::admission::planned_bytes(sink, account).map_err(Error::Delivery)?;
        let current_delivery = sink::live_bytes(account)?;
        let retained = add(
            add(add(previous, native)?, current_delivery)?,
            add(add(core.retained, adapter_bytes)?, sink_bytes)?,
        )?;
        let temporary = add(
            bytes::<Node<'_, P, N>>(original_capacity)?,
            add(
                bytes::<Kind>(nodes.len())?,
                add(
                    bytes::<Connection>(capacity.edges)?,
                    bytes::<sink::admission::Node>(nodes.len())?,
                )?,
            )?,
        )?;
        let staged = add(retained, temporary)?;
        account
            .ledger
            .check(
                BudgetKind::PayloadBytes,
                u64::try_from(retained).map_err(|_| LoanError::ByteOverflow)?,
                crate::loan::AT,
            )
            .map_err(LoanError::from)?;
        account
            .ledger
            .check(
                BudgetKind::StagedPayloadBytes,
                u64::try_from(staged)
                    .map_err(|_| LoanError::ByteOverflow)?
                    .checked_sub(overlap)
                    .ok_or(LoanError::ByteOverflow)?,
                crate::loan::AT,
            )
            .map_err(LoanError::from)?;
        // H owner publication; N moved native records and proposed kinds; E temporary edges;
        // I boundary words; M four fixed sink-position words, count and route;
        // P adapter/temporary allocations, old proposal buffer, each native slot and fixed owner cleanup.
        let records = nodes.len().checked_mul(9).ok_or(LoanError::CountOverflow)?;
        let allocations = 2
            + 2 * usize::from(capacity.nodes != 0)
            + 2 * usize::from(!nodes.is_empty())
            + usize::from(capacity.edges != 0)
            + usize::from(original_capacity != 0);
        let work = add(
            4 + allocations,
            add(records, add(capacity.edges, capacity.nodes)?)?,
        )?;
        charge(&mut account.ledger, work)?;
        Ok(Self {
            bytes: adapter_bytes,
            allocations: allocations - usize::from(original_capacity != 0),
            sink,
        })
    }
}

/// Borrowed originals and endpoint roles are checked without acquiring native retention edges.
fn validate<P: Scalar, const N: usize>(
    nodes: &[Node<'_, P, N>],
    inventory: &LcInputInventory<P, N>,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result<(), Error> {
    for node in nodes {
        charge(&mut account.ledger, 1)?; // N fixed declared contract.
        match &node.contract {
            Contract::Input(Some(declaration)) => {
                account.authenticate(declaration.account)?;
                if declaration.axis.scope != heap.scope {
                    return Err(LoanError::ForeignOwner.into());
                }
            }
            Contract::Mapped {
                source,
                schema,
                parts: Some(parts),
                ..
            } => {
                account.authenticate(parts.destination.account)?;
                let crate::binding::NativeRecipe::Kernel(law) = &parts.operation.recipe else {
                    return Err(LoanError::UnsupportedContract.into());
                };
                if !matches!(law.recipe, crate::kernels::contract::Recipe::Transform(_)) {
                    return Err(LoanError::UnsupportedContract.into());
                }
                if !matches!(
                    nodes.get(*source).map(|n| &n.contract),
                    Some(Contract::Input(_))
                ) {
                    return Err(LoanError::UnsupportedContract.into());
                }
                let mut original = false;
                for position in 0..N {
                    charge(&mut account.ledger, 1)?; // A original schema identity.
                    original |= inventory
                        .input(position)
                        .is_some_and(|input| std::ptr::eq(input, *schema));
                }
                if !original {
                    return Err(LoanError::TargetMismatch.into());
                }
            }
            Contract::Omega { sources, .. } => {
                let Some(Native::OmegaDeclaration(declaration)) = &node.native else {
                    return Err(LoanError::UnsupportedContract.into());
                };
                charge(&mut account.ledger, 1)?; // A complete original inventory.
                if !std::ptr::eq(declaration.inventory(), inventory) {
                    return Err(LoanError::TargetMismatch.into());
                }
                for (position, source) in sources.iter().enumerate() {
                    charge(&mut account.ledger, 2)?; // N producer contract and A positional original.
                    let Some(Node {
                        contract: Contract::Mapped { schema, .. },
                        ..
                    }) = nodes.get(*source)
                    else {
                        return Err(LoanError::UnsupportedContract.into());
                    };
                    if !inventory
                        .input(position)
                        .is_some_and(|input| std::ptr::eq(input, *schema))
                    {
                        return Err(LoanError::TargetMismatch.into());
                    }
                }
            }
            Contract::Delivery { source } => {
                charge(&mut account.ledger, 1)?; // N original checked producer.
                if !matches!(
                    nodes.get(*source).map(|n| &n.contract),
                    Some(Contract::Omega { .. })
                ) {
                    return Err(LoanError::UnsupportedContract.into());
                }
            }
            Contract::Input(None) | Contract::Mapped { parts: None, .. } => {
                return Err(LoanError::UnsupportedContract.into());
            }
        }
    }
    Ok(())
}

/// Count every original positional dependency; equal sources do not merge positions.
fn edge_count<P: Scalar, const N: usize>(nodes: &[Node<'_, P, N>]) -> Result<usize, LoanError> {
    nodes.iter().try_fold(0, |total, node| {
        add(
            total,
            match node.contract {
                Contract::Input(_) => 0,
                Contract::Mapped { .. } | Contract::Delivery { .. } => 1,
                Contract::Omega { .. } => N,
            },
        )
    })
}

/// Compare allocation identities, never equal byte totals, when counting simultaneous owners.
pub(in crate::loan::graph::adapter) fn prior_bytes(
    prior: &[Borrowed<'_>],
    account: &mut ExecutionAccount,
) -> Result<usize, Error> {
    prior_bytes_excluding(prior, None, account)
}

/// An already counted current owner is excluded by allocation identity, never by size.
pub(in crate::loan::graph::adapter) fn prior_bytes_excluding(
    prior: &[Borrowed<'_>],
    excluded: Option<crate::loan::graph::Borrowed<'_>>,
    account: &mut ExecutionAccount,
) -> Result<usize, Error> {
    let mut total = 0;
    for (index, owner) in prior.iter().enumerate() {
        account.authenticate(owner.account)?;
        if let Some(excluded) = excluded {
            charge(&mut account.ledger, 1)?; // A current owner already counted separately.
            if std::ptr::eq(excluded.0, owner.core.0) {
                continue;
            }
        }
        let mut repeated = false;
        for previous in &prior[..index] {
            charge(&mut account.ledger, 1)?; // A original graph allocation.
            if std::ptr::eq(previous.core.0, owner.core.0) {
                repeated = true;
                break;
            }
        }
        if !repeated {
            let core = Inventory::inspect(std::slice::from_ref(&owner.core), account)?;
            total = add(total, add(core.covered_bytes, owner.bytes)?)?;
        }
    }
    Ok(total)
}

/// Temporary incidence records preserve the producer order of every original consumer position.
pub(super) fn connections<P: Scalar, const N: usize>(
    nodes: &[Node<'_, P, N>],
    capacity: usize,
) -> Result<Vec<Connection>, LoanError> {
    let mut edges = crate::loan::vector(capacity)?;
    for (consumer, node) in nodes.iter().enumerate() {
        let mut append = |producer| {
            edges.push(Connection {
                producer,
                consumer,
                demand: producer,
                semantic: false,
            });
        };
        match &node.contract {
            Contract::Input(_) => {}
            Contract::Mapped { source, .. } | Contract::Delivery { source } => append(*source),
            Contract::Omega { sources, .. } => {
                for source in sources {
                    append(*source);
                }
            }
        }
    }
    Ok(edges)
}

/// Closed positional sink contracts borrow no native error payload or capability.
pub(super) fn destinations(
    owner: u64,
    attempts: &[u64],
) -> Result<Vec<sink::admission::Node>, LoanError> {
    let mut nodes = crate::loan::vector(attempts.len())?;
    for (node, attempts) in attempts.iter().enumerate() {
        nodes.push(sink::admission::Node {
            position: sink::codec::Position {
                owner,
                node: u64::try_from(node).map_err(|_| LoanError::CountOverflow)?,
                generation: 1,
                phase: 0,
            },
            attempts: *attempts,
            route: sink::admission::Route::ConsumingReturn,
        });
    }
    Ok(nodes)
}

/// Original attempt envelopes determine the exact fixed codec capacity before any allocation.
fn sink_extents(
    nodes: usize,
    attempts: &[u64],
    capacity: Capacities,
    account: &mut ExecutionAccount,
) -> Result<(), Error> {
    charge(&mut account.ledger, attempts.len())?; // N original attempt records.
    let mut probe_words = 0_usize;
    let mut probe_records = 0_usize;
    for attempt in attempts {
        charge(&mut account.ledger, 1)?; // M sink attempt extent.
        let count = usize::try_from(*attempt).map_err(|_| LoanError::CountOverflow)?;
        probe_records = add(probe_records, count)?;
        probe_words = add(
            probe_words,
            count.checked_mul(8).ok_or(LoanError::ByteOverflow)?,
        )?;
    }
    let records = add(
        nodes.checked_mul(2).ok_or(LoanError::CountOverflow)?,
        probe_records,
    )?;
    let words = add(
        nodes.checked_mul(11).ok_or(LoanError::ByteOverflow)?,
        probe_words,
    )?;
    if capacity.records < records || capacity.words < words {
        return Err(Error::Delivery(sink::Error::Full));
    }
    Ok(())
}
