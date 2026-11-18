// SPDX-License-Identifier: AGPL-3.0-only
//! Fund exact positional ownership before a no-fail installation of both native debits.
use super::{
    LcOmegaCandidate, LcShapeError, borrowed::LcBorrowedCapturePlan, prepared::LcOmegaJoinReceipt,
};
#[cfg(test)]
use crate::ExecutionAccount;
use crate::heap::resources::{ResourcePermit, ResourcePlan, ResourcePolicy, add};
use crate::loan::{
    charge,
    lc_origin::{LcClosure, LcJoinIdentity, LcSourceRow},
    preparation::Preparation,
    rc_bytes,
    registry::{AllocationKind, CleanupReceipt, Debit, DebitOwner, Token},
};
use crate::{Heap, LoanError};
use nrz_bindings::{Budget, LcOmegaCorrespondence, readings::algebra::limits::BudgetKind};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Receipt plus facade edge, outer control block, minus the released initial bindings token.
const PREPARED_INCREMENT: usize = size_of::<LcOmegaJoinReceipt>()
    + size_of::<Rc<LcOmegaCorrespondence>>()
    + 2 * size_of::<usize>()
    - size_of::<nrz_bindings::LcOmegaDeclarationToken>();

/// Complete two-slot transaction; table replacement precedes every retaining-edge acquisition.
#[repr(C)]
struct Plan {
    /// Exact vacant native debit slots.
    slots: [usize; 2],
    /// Complete role keys, distinct from the shared cleanup identity.
    keys: [u64; 2],
    /// Original scope/account and fresh join identity.
    identity: LcJoinIdentity,
    /// Admitted debit table capacity and revision successor.
    capacity: usize,
    /// Prechecked successor, installed only with the completed result.
    revision: u64,
    /// Complete native allocation authorization for the original heap/account.
    permit: ResourcePermit,
    /// Allocation, record and owning-edge cleanup is prepaid before acquisition.
    cleanup: CleanupReceipt,
}

impl Plan {
    /// Derive planning Work and actual backing growth from the existing tables.
    fn new(
        heap: &Heap,
        account: u64,
        rows: usize,
        live: u64,
        capture_bytes: usize,
        budget: &mut Budget,
    ) -> Result<Self, LoanError> {
        charge(budget, 3)?;
        let identity = LcJoinIdentity {
            scope: heap.scope,
            account,
            join: crate::heap::identity::identity()?,
        };
        let keys = [
            crate::heap::identity::identity()?,
            crate::heap::identity::identity()?,
        ];
        charge(budget, heap.loans.debits.len())?;
        let slots = heap.loans.vacant_inventory::<2>(2)?;
        let capacity = add(slots[1], 1)?.max(heap.loans.debits.capacity());
        let (growth, old) =
            crate::loan::lifecycle::admission::growth(&heap.loans.debits, capacity)?;
        let row_bytes = rows
            .checked_mul(size_of::<LcSourceRow>())
            .ok_or(LoanError::ByteOverflow)?;
        let retained = add(add(rc_bytes::<LcClosure>(), row_bytes)?, growth)?;
        let output = live
            .checked_add(PREPARED_INCREMENT as u64)
            .and_then(|bytes| bytes.checked_add(retained as u64))
            .ok_or(LoanError::ByteOverflow)?;
        budget.check(BudgetKind::OutputBytes, output, crate::loan::AT)?;
        let peak = staging(row_bytes, growth, old, capture_bytes)?;
        let scratch = peak.checked_sub(retained).ok_or(LoanError::ByteOverflow)?;
        let revision = heap.loans.next_revision()?;
        charge(budget, heap.resource_work()?)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            add(retained, scratch)? as u64,
            crate::loan::AT,
        )?;
        let mut permit = heap.preflight_resources(
            ResourcePlan {
                retained,
                scratch,
                ..ResourcePlan::default()
            },
            ResourcePolicy::from_budget(budget),
        )?;
        permit.account = Some(account);
        let edges = rows
            .checked_mul(7)
            .and_then(|n| n.checked_add(4))
            .ok_or(LoanError::CountOverflow)?;
        let allocations = 3 + usize::from(growth != 0);
        let records = 2;
        charge(budget, 2 * (allocations + records + edges))?;
        Ok(Self {
            slots,
            keys,
            identity,
            capacity,
            revision,
            permit,
            cleanup: CleanupReceipt {
                identity: identity.join,
                allocations,
                records,
                edges,
            },
        })
    }
}

/// Maximum of the simultaneous phases, relative to the live by-value candidate.
/// Each moved cleanup receipt, row vector and token array belongs to exactly one record.
fn staging(
    row_bytes: usize,
    growth: usize,
    old: usize,
    capture: usize,
) -> Result<usize, LoanError> {
    use crate::loan::application::lc_source::LcMappedSource;
    let moved_plan = size_of::<Plan>() - size_of::<CleanupReceipt>();
    let constructing = moved_plan + size_of::<Preparation>() + capture;
    // Replacement retains both buffers under the native conservative replacement policy.
    let replacement = add(
        add(growth, old.checked_mul(2).ok_or(LoanError::ByteOverflow)?)?,
        constructing + size_of::<Vec<Option<Debit>>>(),
    )?;
    let planning =
        size_of::<Plan>() + size_of::<ResourcePlan>() + size_of::<ResourcePolicy>() + capture;
    // The local row moves into its backing; the borrowed source view is still live.
    let rows = add(
        add(growth, row_bytes)?,
        constructing
            + size_of::<Vec<LcSourceRow>>()
            + size_of::<LcSourceRow>()
            + size_of::<LcMappedSource<'_>>(),
    )?;
    let closure = add(
        add(growth, row_bytes)?,
        constructing + size_of::<LcClosure>() + rc_bytes::<LcClosure>(),
    )?;
    // Both the old 48-byte carrier (inside external live) and new 64-byte Rc are present.
    let outer = add(
        add(growth, row_bytes)?,
        constructing + rc_bytes::<LcClosure>() + rc_bytes::<LcOmegaCorrespondence>(),
    )?;
    // The moved carrier no longer occupies external live's 48-byte slot. Initial token
    // remains until allocate returns; its later drop reduces final residence by eight.
    let installation = add(
        add(growth, row_bytes)?,
        rc_bytes::<LcClosure>()
            + 2 * size_of::<usize>()
            + moved_plan
            + size_of::<CleanupReceipt>()
            + capture
            + size_of::<[Token; 2]>()
            + 2 * size_of::<Rc<()>>()
            + size_of::<[Debit; 2]>(),
    )?;
    let final_owner = add(
        add(growth, row_bytes)?,
        rc_bytes::<LcClosure>() + PREPARED_INCREMENT,
    )?;
    Ok([
        planning,
        replacement,
        rows,
        closure,
        outer,
        installation,
        final_owner,
    ]
    .into_iter()
    .max()
    .unwrap_or_default())
}

/// Every fallible step finishes before installing either debit or exposing the facade.
pub(in crate::loan::lc_shape) fn join_on<P: Scalar + 'static>(
    heap: &mut Heap,
    candidate: LcOmegaCandidate,
    captured: LcBorrowedCapturePlan<'_, '_, P>,
    external: u64,
    identity: u64,
    budget: &mut Budget,
) -> Result<(Rc<LcOmegaCorrespondence>, LcOmegaJoinReceipt), LcShapeError> {
    let live = candidate
        .retained_bytes()?
        .checked_add(external)
        .ok_or(LoanError::ByteOverflow)?;
    budget
        .with_retained_payload(live, crate::loan::AT, |budget| {
            allocate(heap, identity, candidate, captured, budget)
        })
        .map_err(LcShapeError::Loan)
}

/// Unit controls borrow the same live ledger used by the public consuming facade.
#[cfg(test)]
pub(in crate::loan::lc_shape) fn join<P: Scalar + 'static>(
    heap: &mut Heap,
    candidate: LcOmegaCandidate,
    captured: LcBorrowedCapturePlan<'_, '_, P>,
    external: u64,
    account: &mut ExecutionAccount,
) -> Result<(Rc<LcOmegaCorrespondence>, LcOmegaJoinReceipt), LcShapeError> {
    join_on(
        heap,
        candidate,
        captured,
        external,
        account.identity,
        &mut account.ledger,
    )
}

/// Private prefixes unwind through prepaid Rust owners; installation itself only moves values.
fn allocate<P: Scalar + 'static>(
    heap: &mut Heap,
    account: u64,
    candidate: LcOmegaCandidate,
    captured: LcBorrowedCapturePlan<'_, '_, P>,
    budget: &mut Budget,
) -> Result<(Rc<LcOmegaCorrespondence>, LcOmegaJoinReceipt), LoanError> {
    let live = candidate
        .retained_bytes()
        .map_err(|_| LoanError::ByteOverflow)?;
    let plan = Plan::new(
        heap,
        account,
        captured.tiles.len(),
        live,
        size_of::<LcBorrowedCapturePlan<'_, '_, P>>(),
        budget,
    )?;
    plan.permit.storage(heap.storage.epoch())?;
    if plan.permit.scope != heap.scope || plan.permit.account != Some(account) {
        return Err(LoanError::ForeignOwner);
    }
    let mut guard = Preparation::new(plan.cleanup);
    crate::loan::lifecycle::admission::grow_tracked(
        &mut heap.loans.debits,
        plan.capacity,
        &mut guard,
    )?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(captured.tiles.len())
        .map_err(|_| LoanError::AllocationRefused)?;
    guard.acquired(false)?;
    if rows.capacity() != captured.tiles.len() {
        return Err(LoanError::AllocationRefused);
    }
    for tile in captured.tiles {
        rows.push(row(tile)?);
    }
    #[cfg(test)]
    crate::loan::fault::before()?;
    let closure = Rc::new(LcClosure {
        identity: plan.identity.join,
        rows,
    });
    guard.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let correspondence = Rc::new(candidate.correspondence);
    guard.acquired(false)?;
    let _cleanup = guard.finish()?;
    let [header, rows] = [0, 1].map(|index| Token {
        slot: plan.slots[index],
        key: plan.keys[index],
        receipt: plan.identity.join,
    });
    let records = [
        Debit {
            kind: AllocationKind::LcClosure(Rc::downgrade(&closure)),
            owner: DebitOwner::LcJoin(plan.identity),
            key: header.key,
            elements: 0,
            bytes: rc_bytes::<LcClosure>(),
        },
        Debit {
            kind: AllocationKind::LcRows(Rc::downgrade(&closure)),
            owner: DebitOwner::LcJoin(plan.identity),
            key: rows.key,
            elements: 0,
            bytes: closure.rows.capacity() * size_of::<LcSourceRow>(),
        },
    ];
    for (token, debit) in [&header, &rows].into_iter().zip(records) {
        heap.loans.install(token.slot, debit);
    }
    heap.loans.revision = plan.revision;
    Ok((
        correspondence,
        LcOmegaJoinReceipt {
            closure,
            join: plan.identity.join,
            schedule: captured.schedule,
            header,
            rows,
        },
    ))
}

/// Seven prepaid edges retain the exact authenticated pin and every original symbol channel.
fn row<P: Scalar + 'static>(tile: &crate::LcShapeTile<'_, P>) -> Result<LcSourceRow, LoanError> {
    let pin = tile
        .pin()
        .record
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    let mapped = pin.bundle.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    let source = mapped.origin.lc_source()?;
    let companions = [
        mapped.qualified[0].ok_or(LoanError::TargetMismatch)?,
        mapped.qualified[1].ok_or(LoanError::TargetMismatch)?,
        mapped.qualified[2].ok_or(LoanError::TargetMismatch)?,
    ];
    Ok(LcSourceRow {
        symbols: tile.schema().symbols.clone(),
        pin: Rc::clone(&tile.pin().record),
        position: tile.schema().input_index(),
        schema: tile.schema().specialization_key(),
        source: source.identity,
        mapped: mapped.identity,
        axes: tile.axes(),
        projection: source.projection,
        layout: source.identity.layout,
        companions,
    })
}

/// Existing guard fields and the fixed transaction have separate measured storage.
#[cfg(test)]
pub(super) fn layout<P: Scalar>() {
    use crate::loan::application::lc_source::LcMappedSource;
    use std::mem::{align_of, offset_of};
    assert_eq!((size_of::<Plan>(), align_of::<Plan>()), (152, 8));
    assert_eq!(
        [
            offset_of!(Plan, slots),
            offset_of!(Plan, keys),
            offset_of!(Plan, identity),
            offset_of!(Plan, capacity),
            offset_of!(Plan, revision),
            offset_of!(Plan, permit),
            offset_of!(Plan, cleanup)
        ],
        [0, 16, 32, 56, 64, 72, 120]
    );
    assert_eq!(align_of::<LcBorrowedCapturePlan<'_, '_, P>>(), 8);
    assert_eq!(
        [
            offset_of!(LcBorrowedCapturePlan<'_, '_, P>, tiles),
            offset_of!(LcBorrowedCapturePlan<'_, '_, P>, schedule),
        ],
        [0, 16]
    );
    assert_eq!(align_of::<LcMappedSource<'_>>(), 8);
    assert_eq!(
        [
            offset_of!(LcMappedSource<'_>, declaration),
            offset_of!(LcMappedSource<'_>, identity),
            offset_of!(LcMappedSource<'_>, attachments),
            offset_of!(LcMappedSource<'_>, projection),
        ],
        [0, 8, 224, 232]
    );
    assert_eq!(size_of::<ResourcePermit>(), 48);
    assert_eq!(size_of::<Preparation>(), 72);
    assert_eq!(size_of::<CleanupReceipt>(), 32);
    assert_eq!(size_of::<LcBorrowedCapturePlan<'_, '_, P>>(), 64);
    assert_eq!(size_of::<[Debit; 2]>(), 528);
    assert_eq!(size_of::<[Token; 2]>(), 48);
    assert_eq!(
        size_of::<crate::loan::application::lc_source::LcMappedSource<'_>>(),
        304
    );
    assert_eq!(size_of::<ResourcePlan>(), 48);
    assert_eq!(size_of::<ResourcePolicy>(), 16);
}
