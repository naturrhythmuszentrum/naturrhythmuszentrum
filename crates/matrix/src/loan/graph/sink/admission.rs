// SPDX-License-Identifier: AGPL-3.0-only
//! Complete storage and original-account reservation precede every escaping sink edge.

use super::{Backing, Debit, Encoding, Error, Position, Sink, Slot, Status};
use crate::loan::{
    ExecutionAccount, LoanError, charge,
    graph::inventory::{add, bytes},
    rc_bytes,
};
use nrz_bindings::readings::algebra::limits::BudgetKind;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Admission disposition applies to every reachable field, not only a late error instance.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Route {
    /// Native variable payloads are returned intact with their consuming owner.
    ConsumingReturn,
    /// A node would retain the unsupported-scale String.
    RetainUnsupportedScale,
    /// A node would retain the conflicting-kind String.
    RetainConflictingKind,
    /// A node would retain the series-evidence Rational.
    RetainSeriesConstant,
    /// No owning-layer bound or intact consuming-return route is established.
    Unproved,
}
/// Fixed original node contract, with no admission authority manufactured from an error tag.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    /// Original phase and occupation retained in each destination.
    pub(crate) position: Position,
    /// Finite original attempt envelope, not a Work limit.
    pub(crate) attempts: u64,
    /// Reachable variable-field disposition.
    pub(crate) route: Route,
}
/// Caller-selected storage extents; no defaults or resource dimensions are introduced.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Capacities {
    /// Full allocated record extent.
    pub(crate) records: usize,
    /// Full allocated evidence-word extent.
    pub(crate) words: usize,
    /// Maximum simultaneously escaping outside witnesses.
    pub(crate) witnesses: usize,
}
/// Checked storage plan excludes no allocation or account-registration control block.
struct Plan {
    /// Complete new shared owner and three backing allocations.
    bytes: usize,
    /// First account registration needs its own shared allocation.
    registration: bool,
    /// Complete allocation, edge and final-status cleanup entitlement.
    cleanup: usize,
    /// Actual word initialization across record, evidence and witness capacity.
    initialize: usize,
    /// Repeated source-position reads and the final registration update.
    source: usize,
}
impl Plan {
    /// Every required terminal, probe and exhaustion destination is reserved before activation.
    fn new(nodes: &[Node], capacity: Capacities, registration: bool) -> Result<Self, Error> {
        let mut records = nodes.len();
        let mut words = nodes.len().checked_mul(6).ok_or(LoanError::ByteOverflow)?;
        for node in nodes {
            if node.position.owner == 0 || node.position.generation == 0 {
                return Err(LoanError::UnsupportedContract.into());
            }
            if !matches!(node.route, Route::ConsumingReturn) {
                return Err(Error::Unbounded);
            }
            let attempts = usize::try_from(node.attempts).map_err(|_| LoanError::CountOverflow)?;
            records = add(records, add(attempts, 1)?)?;
            words = add(
                words,
                add(attempts.checked_mul(8).ok_or(LoanError::ByteOverflow)?, 5)?,
            )?;
        }
        if capacity.records < records || capacity.words < words {
            return Err(Error::Full);
        }
        let retained = add(
            rc_bytes::<Backing>(),
            add(
                bytes::<Slot>(capacity.records)?,
                add(
                    bytes::<u64>(capacity.words)?,
                    bytes::<Cell<u64>>(capacity.witnesses)?,
                )?,
            )?,
        )?;
        let allocations = 1
            + usize::from(registration)
            + [capacity.records, capacity.words, capacity.witnesses]
                .into_iter()
                .filter(|n| *n != 0)
                .count();
        // P: allocations, registration edge, debit update, abandonment status and each record.
        let cleanup = add(allocations + 3, capacity.records)?;
        let initialize = add(
            bytes::<Slot>(capacity.records)?.div_ceil(8),
            add(
                capacity.words,
                add(capacity.witnesses, usize::from(registration))?,
            )?,
        )?;
        let source = add(
            records.checked_mul(4).ok_or(LoanError::CountOverflow)?,
            add(nodes.len(), 1)?,
        )?;
        Ok(Self {
            bytes: retained,
            registration,
            cleanup,
            initialize,
            source,
        })
    }
}
impl Sink {
    /// Prior graph rows are borrowed and authenticated; all live sink debits are account-resident.
    pub(crate) fn admit(
        nodes: &[Node],
        capacity: Capacities,
        prior: &[crate::loan::graph::Borrowed<'_>],
        account: &mut ExecutionAccount,
    ) -> Result<Self, Error> {
        let prior = crate::loan::graph::Inventory::inspect(prior, account)?.covered_bytes;
        charge(
            &mut account.ledger,
            add(
                1,
                nodes.len().checked_mul(4).ok_or(LoanError::CountOverflow)?,
            )?,
        )?;
        // H entry + M owner/generation, route and attempt count of each node contract.
        if account.delivery.is_some() {
            charge(&mut account.ledger, 1)?;
        }
        let plan = Plan::new(nodes, capacity, account.delivery.is_none())?;
        let registration = usize::from(plan.registration) * rc_bytes::<Cell<usize>>();
        let combined = add(
            add(prior, super::live_bytes(account)?)?,
            add(plan.bytes, registration)?,
        )?;
        let combined = u64::try_from(combined).map_err(|_| LoanError::ByteOverflow)?;
        account
            .ledger
            .check(BudgetKind::PayloadBytes, combined, crate::loan::AT)
            .map_err(LoanError::from)?;
        account
            .ledger
            .check(BudgetKind::StagedPayloadBytes, combined, crate::loan::AT)
            .map_err(LoanError::from)?;
        // H saved owner state; I all initialized words; P all acquired prefixes and final release.
        charge(
            &mut account.ledger,
            add(add(1 + plan.cleanup, plan.initialize)?, plan.source)?,
        )?;
        let identity = crate::heap::identity::identity().map_err(LoanError::from)?;
        let slots = slots(nodes, capacity.records)?;
        let mut words = crate::loan::vector(capacity.words)?;
        words.resize(capacity.words, 0);
        let mut witnesses = crate::loan::vector(capacity.witnesses)?;
        witnesses.resize_with(capacity.witnesses, || Cell::new(0));
        #[cfg(test)]
        if account.delivery.is_none() {
            crate::loan::fault::before()?;
        }
        let registration = registration_owner(account);
        let next = add(registration.get(), plan.bytes)?;
        #[cfg(test)]
        crate::loan::fault::before()?;
        registration.set(next);
        let backing = Rc::new(Backing {
            account: account.identity,
            identity,
            status: Cell::new(if nodes.is_empty() {
                Status::Complete
            } else {
                Status::Incomplete
            }),
            outstanding: Cell::new(nodes.len()),
            slots,
            words: RefCell::new(words),
            witnesses,
            debit: Debit {
                registration,
                bytes: plan.bytes,
            },
        });
        Ok(Self { backing })
    }
}
/// A lazy registration stays charged to this account even when every evidence owner disappears.
fn registration_owner(account: &mut ExecutionAccount) -> Rc<Cell<usize>> {
    if let Some(registration) = &account.delivery {
        return Rc::clone(registration);
    }
    let registration = Rc::new(Cell::new(0));
    account.delivery = Some(Rc::clone(&registration));
    registration
}
/// Stable admission order: terminal node records, then each node's probes and exhaustion.
fn slots(nodes: &[Node], capacity: usize) -> Result<Vec<Slot>, Error> {
    let mut slots = crate::loan::vector(capacity)?;
    let mut offset = 0;
    for node in nodes {
        push(&mut slots, &mut offset, Encoding::Outcome, node.position, 1);
    }
    for node in nodes {
        for _ in 0..node.attempts {
            push(&mut slots, &mut offset, Encoding::Probe, node.position, 0);
        }
        push(
            &mut slots,
            &mut offset,
            Encoding::Exhaustion,
            node.position,
            0,
        );
    }
    while slots.len() < capacity {
        push(
            &mut slots,
            &mut offset,
            Encoding::Vacant,
            Position::default(),
            0,
        );
    }
    Ok(slots)
}
/// Initialization was charged by actual destination words, including inline metadata.
fn push(
    slots: &mut Vec<Slot>,
    offset: &mut usize,
    encoding: Encoding,
    position: Position,
    state: u64,
) {
    slots.push(Slot {
        offset: *offset,
        bound: encoding.bytes(),
        encoding,
        position,
        generation: Cell::new(1),
        state: Cell::new(state),
        length: Cell::new(0),
        witnesses: Cell::new(0),
    });
    *offset += encoding.bytes().div_ceil(8);
}
