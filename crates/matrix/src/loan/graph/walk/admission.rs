// SPDX-License-Identifier: AGPL-3.0-only
//! Construct private incidence, then execute the specified iterative DFS before publication.

use super::{Admission, BLACK, Connection, Core, Error, GRAY, Kind, LIVE, NONE, READY, add};
use crate::loan::charge;
use crate::loan::graph::{
    Backing, ExecutionAccount, LoanError, Reservation,
    records::{Endpoint, Frame},
};
use nrz_bindings::Budget;
use std::rc::Rc;

impl Core {
    /// Readers prevent consumption; all other failures retain a non-runnable private owner.
    pub(crate) fn admit(
        reservation: Reservation,
        kinds: &[Kind],
        edges: &[Connection],
        account: &mut ExecutionAccount,
    ) -> Admission {
        Self::admit_from(
            reservation,
            kinds,
            edges,
            account,
            crate::heap::identity::identity,
        )
    }
    /// Isolated counters exercise the same owner-admission path without seeding `NEXT_ID`.
    #[cfg(test)]
    pub(super) fn admit_with(
        reservation: Reservation,
        kinds: &[Kind],
        edges: &[Connection],
        account: &mut ExecutionAccount,
        source: &std::sync::atomic::AtomicU64,
    ) -> Admission {
        Self::admit_from(reservation, kinds, edges, account, || {
            crate::heap::identity::allocate_identity(source)
        })
    }
    /// Reader refusal precedes allocation of the new admitted-owner identity.
    fn admit_from(
        mut reservation: Reservation,
        kinds: &[Kind],
        edges: &[Connection],
        account: &mut ExecutionAccount,
        identity: impl FnOnce() -> Result<u64, crate::HeapError>,
    ) -> Admission {
        if let Err(error) = account.authenticate(reservation.backing.account) {
            return Admission::Refused(error.into(), reservation);
        }
        let entry = account
            .ledger
            .with_work_phase("matrix.graph", "graph admission", |budget| {
                charge(budget, 1)
            });
        if let Err(error) = entry {
            return Admission::Refused(error.into(), reservation);
        }
        if Rc::strong_count(&reservation.backing) != 1 {
            return Admission::OwnershipWait(reservation);
        }
        let Some(backing) = Rc::get_mut(&mut reservation.backing) else {
            return Admission::OwnershipWait(reservation);
        };
        let result = account
            .ledger
            .with_work_phase("matrix.graph", "graph admission", |budget| {
                validate(backing, kinds, edges, budget)?;
                construct(backing, edges, budget)?;
                dfs(backing, budget)?;
                let owner = identity().map_err(LoanError::from)?;
                publish(backing, budget)?;
                backing.identity = owner;
                Ok(())
            });
        match result {
            Ok(()) => Admission::Admitted(Self { reservation }),
            Err(error) => Admission::Refused(error, reservation),
        }
    }
}
/// Contract and endpoint inspections precede every private topology mutation.
fn validate(
    backing: &Backing,
    kinds: &[Kind],
    edges: &[Connection],
    budget: &mut Budget,
) -> Result<(), Error> {
    let nodes_fit = kinds.len() == backing.proposed;
    let edges_fit = edges.len() == backing.edges.len();
    if !nodes_fit || !edges_fit {
        return Err(LoanError::UnsupportedContract.into());
    }
    for kind in kinds {
        charge(budget, 1)?; // N proposed node contract.
        if !matches!(kind, Kind::Fixture) {
            return Err(LoanError::UnsupportedContract.into());
        }
    }
    for edge in edges {
        charge(budget, 1)?; // E proposed endpoint/association record.
        let producer_fits = edge.producer < kinds.len();
        let consumer_fits = edge.consumer < kinds.len();
        if !producer_fits || !consumer_fits {
            return Err(LoanError::UnsupportedContract.into());
        }
    }
    Ok(())
}
/// Append in proposal order using private node tails, then convert tails to transition cursors.
fn construct(
    backing: &mut Backing,
    edges: &[Connection],
    budget: &mut Budget,
) -> Result<(), Error> {
    for slot in 0..backing.nodes.len() {
        charge(budget, 1)?; // N fixed slot state and links; colour remains inline.
        let node = &mut backing.nodes[slot];
        node.links = [NONE; 8];
        node.counts = [0; 2];
        node.flags = if slot < backing.proposed { LIVE } else { 0 };
    }
    for (index, proposed) in edges.iter().enumerate() {
        let ends = [proposed.producer, proposed.consumer];
        let mut prior = [NONE; 2];
        // E initialization + two N incidence/count visits; each existing tail is another E.
        charge(budget, 3)?;
        let mut work = 0;
        for (direction, slot) in ends.into_iter().enumerate() {
            prior[direction] = backing.nodes[slot].links[5 + direction];
            work += usize::from(prior[direction] != NONE);
        }
        charge(budget, work)?;
        let edge = &mut backing.edges[index];
        edge.endpoints = ends.map(|slot| Endpoint {
            slot,
            generation: backing.nodes[slot].generation,
        });
        edge.links = [prior[0], NONE, prior[1], NONE];
        edge.demand = proposed.demand;
        edge.flags = u64::from(proposed.semantic);
        for (direction, slot) in ends.into_iter().enumerate() {
            if prior[direction] == NONE {
                backing.nodes[slot].links[direction] = index;
            } else {
                backing.edges[prior[direction]].links[direction * 2 + 1] = index;
            }
            backing.nodes[slot].links[5 + direction] = index;
        }
        backing.nodes[proposed.producer].counts[1] += 1;
        backing.nodes[proposed.consumer].counts[0] += 1;
    }
    Ok(())
}
/// Root order, edge order and frame movement are the design's literal algorithm.
fn dfs(backing: &mut Backing, budget: &mut Budget) -> Result<(), Error> {
    for root in 0..backing.nodes.len() {
        charge(budget, 1)?; // N root inspection includes vacancies and inline colour.
        if backing.nodes[root].flags != LIVE {
            continue;
        }
        charge(budget, 1)?; // X root push; gray update belongs to the root N visit.
        backing.nodes[root].flags |= GRAY;
        backing.stack[0] = Frame {
            node: root,
            edge: backing.nodes[root].links[0],
        };
        let mut depth = 1;
        while depth != 0 {
            let frame = backing.stack[depth - 1];
            if frame.edge == NONE {
                charge(budget, 2)?; // N exhausted frame, X pop.
                backing.nodes[frame.node].flags = LIVE | BLACK;
                depth -= 1;
                continue;
            }
            charge(budget, 3)?; // E edge, N endpoint colour/generation, N source cursor.
            let edge = backing.edges[frame.edge];
            let target = edge.endpoints[1];
            if backing.nodes[target.slot].generation != target.generation {
                return Err(LoanError::StaleOccupation.into());
            }
            backing.stack[depth - 1].edge = edge.links[1];
            let colour = backing.nodes[target.slot].flags;
            if colour & GRAY != 0 {
                return Err(Error::Cycle);
            }
            if colour & BLACK != 0 {
                continue;
            }
            charge(budget, 1)?; // X child push, no edge frame.
            backing.nodes[target.slot].flags |= GRAY;
            backing.stack[depth] = Frame {
                node: target.slot,
                edge: backing.nodes[target.slot].links[0],
            };
            depth += 1;
        }
    }
    Ok(())
}
/// Prepay complete publication and cleanup before any ready bit becomes visible.
fn publish(backing: &mut Backing, budget: &mut Budget) -> Result<(), Error> {
    charge(budget, backing.proposed)?; // N readiness preflight inspections.
    let ready = backing.nodes[..backing.proposed]
        .iter()
        .filter(|node| node.counts[0] == 0)
        .count();
    let mut work = add(1, backing.proposed)?; // H header publication, N occupied states/cursors.
    work = add(work, ready)?; // M each initial base-bit update.
    // P cleanup: each occupied node has a fixed retirement N+H and full index clear.
    // Each concrete edge has E+2N+H, its present incidence neighbours, and two full paths.
    // Explicit owner abandonment additionally inspects every reserved slot/edge once;
    // its index clear and fixed node/header work are separate, finite entitlements.
    let height = super::bitmap::height(backing.nodes.len());
    let node_cleanup = add(3, height.checked_mul(2).ok_or(LoanError::CountOverflow)?)?;
    let mut cleanup = backing
        .proposed
        .checked_mul(node_cleanup)
        .ok_or(LoanError::CountOverflow)?;
    cleanup = add(cleanup, backing.nodes.len())?;
    let records = add(backing.cleanup.records, cleanup)?;
    let mut edge_cleanup = backing.edges.len();
    for edge in &backing.edges {
        charge(budget, 1)?; // E inventory inspection, before any retaining use is acquired.
        let neighbours = edge.links.iter().filter(|link| **link != NONE).count();
        edge_cleanup = add(
            edge_cleanup,
            add(
                4 + neighbours,
                height.checked_mul(2).ok_or(LoanError::CountOverflow)?,
            )?,
        )?;
    }
    let edges = add(backing.cleanup.edges, edge_cleanup)?;
    cleanup = add(cleanup, edge_cleanup)?;
    work = add(work, cleanup)?;
    let mut lower = backing.nodes.len().div_ceil(64);
    while lower > 1 {
        let upper = lower.div_ceil(64);
        work = add(work, add(lower, upper)?)?; // M lower reads + initialized summary writes.
        lower = upper;
    }
    charge(budget, work)?;
    for slot in 0..backing.proposed {
        let node = &mut backing.nodes[slot];
        node.flags = LIVE;
        node.links[5] = NONE;
        node.links[6] = NONE;
        node.links[7] = NONE;
        if node.counts[0] == 0 {
            node.flags |= READY;
            backing.ready[slot / 64] |= 1 << (slot % 64);
        }
    }
    let mut offset = 0;
    let mut lower = backing.nodes.len().div_ceil(64);
    while lower > 1 {
        let upper = lower.div_ceil(64);
        for word in 0..upper {
            let mut summary = 0;
            for child in word * 64..((word + 1) * 64).min(lower) {
                if backing.ready[offset + child] != 0 {
                    summary |= 1 << (child % 64);
                }
            }
            backing.ready[offset + lower + word] = summary;
        }
        offset += lower;
        lower = upper;
    }
    backing.control.waiting = [NONE; 3];
    backing.control.pending = [NONE; 2];
    backing.control.free = [NONE; 2];
    backing.cleanup.edges = edges;
    backing.cleanup.records = records;
    Ok(())
}
