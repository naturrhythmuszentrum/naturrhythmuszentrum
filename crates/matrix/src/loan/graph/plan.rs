// SPDX-License-Identifier: AGPL-3.0-only
//! Checked caller capacities are not native invocation limits or CAS vertex quotas.

use super::{
    Backing, LoanError,
    inventory::{add, bytes},
    records::{Edge, Frame, Node, Scope},
};

/// Borrowed proposal counts, with no graph publication or borrowed numerical authority.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Arguments<'a> {
    /// Allocated node slots, including vacancies.
    pub(crate) nodes: usize,
    /// Allocated edge slots, exactly equal to the proposed edge count.
    pub(crate) edges: usize,
    /// Complete proposed concrete edge count, even for pending nodes.
    pub(crate) proposed_edges: usize,
    /// Fixed number of future notification scope records.
    pub(crate) scopes: usize,
    /// Original nonreplenishing traversal count.
    pub(crate) drive: u64,
    /// One count for every proposed node; the rest of the pool remains vacant.
    pub(crate) attempts: &'a [u64],
}
/// Complete private storage acquisition inventory, before allocation starts.
#[derive(Clone, Copy, Debug)]
pub(super) struct Plan {
    /// Base and all summary words.
    pub(super) ready: usize,
    /// Packed scope membership across all reserved nodes.
    pub(super) membership: usize,
    /// Complete covered allocation bytes, including the shared header.
    pub(super) retained: usize,
    /// Cleanup entitlements, including the final shared allocation.
    pub(super) allocations: usize,
    /// Exact graph entry, record/word initialization and prepaid cleanup Work.
    pub(super) work: usize,
}
impl Plan {
    /// Only capacity and count validation occurs here; W still owes topology validation.
    pub(super) fn new(args: Arguments<'_>) -> Result<Self, LoanError> {
        let fits = args.attempts.len() <= args.nodes;
        let exact_edges = args.edges == args.proposed_edges;
        if !fits || !exact_edges {
            return Err(LoanError::UnsupportedContract);
        }
        let mut total_attempts = 0_u64;
        for attempt in args.attempts {
            total_attempts = total_attempts
                .checked_add(*attempt)
                .ok_or(LoanError::CountOverflow)?;
        }
        let ready = ready_words(args.nodes)?;
        let membership = args
            .nodes
            .div_ceil(64)
            .checked_mul(args.scopes)
            .and_then(|words| words.checked_mul(2))
            .ok_or(LoanError::ByteOverflow)?;
        let retained = add(
            super::super::rc_bytes::<Backing>(),
            add(
                add(bytes::<Node>(args.nodes)?, bytes::<Edge>(args.edges)?)?,
                add(
                    add(bytes::<u64>(ready)?, bytes::<Frame>(args.nodes)?)?,
                    add(
                        bytes::<u64>(args.nodes)?,
                        add(bytes::<Scope>(args.scopes)?, bytes::<u64>(membership)?)?,
                    )?,
                )?,
            )?,
        )?;
        let extents = [
            args.nodes,
            args.edges,
            ready,
            args.nodes,
            args.nodes,
            args.scopes,
            membership,
        ];
        let allocations = 1 + extents.into_iter().filter(|count| *count != 0).count();
        // H initial private control-cursor transition; C node/E edge visits;
        // Scratch and scope backing is initialized by native words, not by whole records.
        // Attempt-copy visits are N; cleanup pays each backing and one fixed owner record.
        let mut work = add(1, add(allocations, 1)?)?;
        for count in [
            args.nodes,
            args.edges,
            ready,
            args.nodes,
            membership,
            bytes::<Frame>(args.nodes)? / size_of::<usize>(),
            bytes::<Scope>(args.scopes)? / size_of::<usize>(),
        ] {
            work = add(work, count)?;
        }
        work = add(work, args.attempts.len())?;
        Ok(Self {
            ready,
            membership,
            retained,
            allocations,
            work,
        })
    }
}
/// One summary word per lower group through the final top word; zero slots has no words.
fn ready_words(nodes: usize) -> Result<usize, LoanError> {
    let mut words = nodes.div_ceil(64);
    let mut total = words;
    while words > 1 {
        words = words.div_ceil(64);
        total = add(total, words)?;
    }
    Ok(total)
}
