// SPDX-License-Identifier: AGPL-3.0-only
//! Separated serialized residence is a scoped inventory, never a numerical completion certificate.
use super::{exact, gate::Gate, owner::Borrowed};
use crate::loan::{
    charge,
    graph::{
        Inventory,
        inventory::{Row, add},
        sink,
        walk::Error,
    },
};
use crate::{ExecutionAccount, Heap, LoanError};

/// Original native debit meanings remain separate from unknown exact/provenance capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Native {
    /// Addressable native scalar elements by precision, including detached host occupations.
    pub elements: [usize; 2],
    /// Observable numerical slab capacity, including padding and conservative unreconciled debits.
    pub numerical_bytes: usize,
    /// Native table/header/tracking and execution-history inventory under its existing debit policy.
    pub other_owner_and_table_bytes: usize,
    /// Source-free constant debits combine owner and numerical bytes; no finer partition is claimed.
    pub unpartitioned_constant_bytes: usize,
}

/// No aggregate total combines logical allowances, overlapping inline records and unknown backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// Native heap partition uses one serialized read of the original heap and ledger.
    pub native: Native,
    /// Deduplicated named graph storage excluding its observer and separate evidence storage.
    pub graph_bytes: usize,
    /// Fixed active observer allocations, including their disjoint Rc control words.
    pub observer_bytes: usize,
    /// Account-resident in-process delivery/history, including witnesses whose graph was abandoned.
    pub delivery_history_bytes: usize,
    /// Existing logical allowances and actual certificate/control-block observations stay distinct.
    pub exact_children: exact::Report,
    /// Preserve O's original coverage classifications, including its unchanged deferred rows.
    pub walk_rows: [Row; 7],
    /// Distinct admitted graph allocations in the explicitly supplied set.
    pub graph_owners: usize,
    /// Original symbolic/provenance backing is unavailable; use each owner's borrowed R report.
    pub original_exact_capacity_bytes: Option<usize>,
    /// Original provenance backing is not upgraded by a native or graph byte inventory.
    pub provenance_capacity_bytes: Option<usize>,
}
impl Snapshot {
    /// Include exactly the named borrowed graph owners and the supplied heap snapshot.
    /// Required external exporters are outside this in-process report. Originals retain
    /// the separate `Owner::originals` observations and exclusions for each supplied owner.
    /// # Errors
    /// Preserves original account, occupation, phase and cumulative resource refusals.
    pub fn inspect(
        owners: &[Borrowed<'_>],
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, Error> {
        for owner in owners {
            account.authenticate(owner.account)?;
        }
        let result = Self::inspect_inner(owners, heap, account);
        for owner in owners {
            owner.gate.observe(&result);
        }
        result
    }

    /// Every scoped report shares the same original Work ledger and carries no completion authority.
    fn inspect_inner(
        owners: &[Borrowed<'_>],
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, Error> {
        let _span = tracing::trace_span!("nrz.graph.lifecycle_inventory").entered();
        let native = native(heap, account)?;
        let (graph_bytes, observer_bytes, walk_rows, graph_owners) = graphs(owners, account)?;
        charge(&mut account.ledger, 1)?; // H account-resident delivery registration.
        let delivery_history_bytes = sink::live_bytes(account)?;
        let exact_children = exact::Scope {
            current: None,
            prior: owners,
            heap,
        }
        .inspect(account)?;
        Ok(Self {
            native,
            graph_bytes,
            observer_bytes,
            delivery_history_bytes,
            exact_children,
            walk_rows,
            graph_owners,
            original_exact_capacity_bytes: None,
            provenance_capacity_bytes: None,
        })
    }
}

/// The original whole-heap tariff is unchanged; the new disjoint partition funds its extra visits.
fn native(heap: &Heap, account: &mut ExecutionAccount) -> Result<Native, LoanError> {
    account
        .ledger
        .with_work_phase("matrix.graph", "native residence partition", |budget| {
            charge(budget, heap.resource_work()?)?;
            let total = heap.resource_bytes()?;
            charge(
                budget,
                add(1, add(heap.storage.scan_work(), heap.loans.debits.len())?)?,
            )?;
            let (managed, detached, constants) = heap.loans.graph_partition()?;
            let legacy = heap.storage.elements();
            let numerical_bytes = add(detached, heap.storage.numerical_bytes()?)?;
            let other_owner_and_table_bytes = total
                .checked_sub(numerical_bytes)
                .and_then(|bytes| bytes.checked_sub(constants))
                .ok_or(LoanError::ByteOverflow)?;
            Ok(Native {
                elements: [add(legacy[0], managed[0])?, add(legacy[1], managed[1])?],
                numerical_bytes,
                other_owner_and_table_bytes,
                unpartitioned_constant_bytes: constants,
            })
        })
}

/// Stable first occurrence counts each covered graph allocation once, without retaining an owner.
fn graphs(
    owners: &[Borrowed<'_>],
    account: &mut ExecutionAccount,
) -> Result<(usize, usize, [Row; 7], usize), Error> {
    let mut graph = 0;
    let mut observers = 0;
    let mut count = 0;
    let mut rows = [0_usize; 3];
    for (index, owner) in owners.iter().enumerate() {
        let mut repeated = false;
        for previous in &owners[..index] {
            charge(&mut account.ledger, 1)?; // A original graph allocation.
            if std::ptr::eq(previous.core.0, owner.core.0) {
                repeated = true;
                break;
            }
        }
        if repeated {
            continue;
        }
        charge(&mut account.ledger, 1)?; // H fixed adapter byte/observer partition.
        let core = Inventory::inspect(std::slice::from_ref(&owner.core), account)?;
        for (sum, row) in rows.iter_mut().zip(core.rows) {
            let Row::Covered(bytes) = row else {
                return Err(LoanError::UnsupportedContract.into());
            };
            *sum = add(*sum, bytes)?;
        }
        let observer = crate::loan::rc_bytes::<Gate>();
        let adapter = owner
            .bytes
            .checked_sub(observer)
            .ok_or(LoanError::ByteOverflow)?;
        graph = add(graph, add(core.covered_bytes, adapter)?)?;
        observers = add(observers, observer)?;
        count = add(count, 1)?;
    }
    Ok((graph, observers, Inventory::rows(rows), count))
}
