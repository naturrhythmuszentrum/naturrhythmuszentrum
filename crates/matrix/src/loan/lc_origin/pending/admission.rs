// SPDX-License-Identifier: AGPL-3.0-only
//! All fallible preparation precedes moving the original prepared correspondence and closure.
use super::super::{LcChannel, LcDebits, LcLiveness, LcOutcome};
use super::{Bundle, LcApplicationPending, Native as PendingNative, Storage, Use, plan::Plan};
use crate::heap::{
    descriptors::DescriptorRecord,
    slots::{PreparedSlots, SlotPlan},
};
use crate::loan::{
    lifecycle::admission::grow_tracked,
    payload::Numerical,
    preparation::Preparation,
    rc_bytes,
    registry::{
        AllocationKind, CleanupReceipt, Debit, DebitInventory, DebitOwner, Group, Identity,
        ManagedDescriptor, ProducerOutcome, State, Token,
    },
    vector,
};
use crate::mask::storage::{LayoutRecord, MaskRecord, Native};
use crate::{CellLayoutVersion, Heap, HostChannel, LcOmegaPrepared, LoanError, MaskVersion};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::{mem::MaybeUninit, rc::Rc};

/// Constructed payloads and index records; the original prepared owner has not moved yet.
#[repr(C)]
pub(super) struct Transaction {
    /// Exact occupation of this new pending producer.
    identity: Identity,
    /// Prepaid capacities and identities, moved into installation.
    plan: Plan,
    /// Prepared native index changes and debit backing.
    records: Records,
    /// Three private initialized numerical channels.
    channels: [LcChannel; 3],
    /// Private demand partition, output layout and completed mask.
    geometry: Geometry,
    /// Allocated destination for the final storage move.
    box_shell: Box<MaybeUninit<Storage>>,
    /// Unique empty complete-bundle destination.
    bundle: Rc<Bundle>,
    /// Unique empty initial-use destination.
    initial_use: Rc<Use>,

    /// Existing immutable demand edge.
    demand_mask: Rc<MaskRecord>,
    /// Existing immutable demand-layout edge.
    demand_layout: Rc<LayoutRecord>,
    /// Remaining prepaid owner destruction entitlement.
    cleanup: CleanupReceipt,
    /// Final edge, after every protected payload field.
    liveness: Rc<LcLiveness>,
}

/// Prepared index rows and their exact-capacity temporary backing owners.
#[repr(C)]
struct Records {
    /// Three unfilled descriptor occupations.
    descriptors: PreparedSlots<DescriptorRecord>,
    /// One reserved output layout occupation.
    layouts: PreparedSlots<Native<LayoutRecord>>,
    /// One reserved completed-mask occupation.
    masks: PreparedSlots<Native<MaskRecord>>,
    /// Seventeen new weak allocation records.
    debits: Vec<Debit>,
}

/// Demand partition and empty completion metadata; all fields move into storage.
#[repr(C)]
struct Geometry {
    /// Demand-only outcome records, with no filled evidence.
    outcomes: Vec<LcOutcome>,
    /// The private contiguous output layout.
    layout: Rc<LayoutRecord>,
    /// The private, initially empty completed mask.
    completed: Rc<MaskRecord>,
}

/// Prepare every fallible owner while the supplied facade remains borrowed.
pub(super) fn prepare<P: Scalar + 'static, const N: usize>(
    prepared: &LcOmegaPrepared<'_, P, N>,
    heap: &mut Heap,
    demand: MaskVersion,
    account: u64,
    budget: &mut Budget,
) -> Result<Transaction, LoanError> {
    let mut plan = Plan::new(prepared, heap, account, demand, budget)?;
    super::check::all(prepared, heap, account)?;
    plan.permit.storage(heap.storage.epoch())?;
    let cleanup_identity = plan.cleanup.identity;
    let mut guard = Preparation::new(std::mem::replace(
        &mut plan.cleanup,
        CleanupReceipt::empty(cleanup_identity),
    ));
    grow_tracked(&mut heap.loans.groups, plan.groups, &mut guard)?;
    grow_tracked(&mut heap.loans.debits, plan.debits, &mut guard)?;
    let descriptors = heap.descriptors.slots.prepare_fixed(
        &plan.descriptor_keys.take().ok_or(LoanError::WrongLoan)?,
        &mut guard,
    )?;
    let layouts = heap.masks.layouts.prepare_fixed(
        &plan.layout_keys.take().ok_or(LoanError::WrongLoan)?,
        &mut guard,
    )?;
    let masks = heap.masks.masks.prepare_fixed(
        &plan.mask_keys.take().ok_or(LoanError::WrongLoan)?,
        &mut guard,
    )?;
    let slots = [descriptors, layouts, masks];
    let identity = identity(prepared, heap, &plan, account, &slots)?;
    let layout_version = identity.layout;
    let channels = channels::<P>(&plan, &mut guard)?;
    let demand_mask = Rc::clone(heap.masks.masks.get(demand.key)?.owner()?);
    let demand_layout = Rc::clone(heap.masks.layouts.get(demand.layout.key)?.owner()?);
    let geometry = geometry(
        &plan,
        layout_version,
        &demand_mask,
        &demand_layout,
        &mut guard,
    )?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let box_shell = Box::<Storage>::new_uninit();
    guard.acquired(false)?;
    let bundle = shared(
        Bundle {
            storage: None,
            seal: None,
        },
        &mut guard,
    )?;
    let initial_use = shared(Use { payload: None }, &mut guard)?;
    let liveness = shared(
        LcLiveness {
            identity,
            receipt: cleanup_identity,
        },
        &mut guard,
    )?;
    let records = records(&plan, identity, &liveness, slots, &mut guard)?;
    let cleanup = guard.finish()?;
    Ok(Transaction {
        identity,
        plan,
        records,
        channels,
        geometry,
        box_shell,
        bundle,
        initial_use,
        demand_mask,
        demand_layout,
        cleanup,
        liveness,
    })
}

/// Initialize immutable demand outcomes and allocate empty output completion metadata.
fn geometry(
    plan: &Plan,
    layout_version: CellLayoutVersion,
    demand_mask: &MaskRecord,
    demand_layout: &LayoutRecord,
    guard: &mut Preparation,
) -> Result<Geometry, LoanError> {
    let mut outcomes = vector(plan.cells)?;
    if plan.cells != 0 {
        guard.acquired(false)?;
    }
    outcomes.extend((0..plan.cells).map(|cell| LcOutcome {
        status: u8::from(crate::loan::payload::bit(&demand_mask.words, cell)),
        target: 0,
        reason: 0,
    }));
    let mut completed_words = vector(plan.words)?;
    if plan.words != 0 {
        guard.acquired(false)?;
    }
    completed_words.resize(plan.words, 0);
    let layout = shared(
        LayoutRecord {
            axis: demand_layout.axis,
            components: crate::Layout::contiguous(crate::Extent::new(plan.cells, 1)?)?,
            gather: None,
        },
        guard,
    )?;
    let completed = shared(
        MaskRecord {
            layout: layout_version,
            words: completed_words,
        },
        guard,
    )?;
    Ok(Geometry {
        outcomes,
        layout,
        completed,
    })
}

/// Bind every future native occupation before allocating its protected storage.
fn identity<P: Scalar + 'static, const N: usize>(
    prepared: &LcOmegaPrepared<'_, P, N>,
    heap: &Heap,
    plan: &Plan,
    account: u64,
    slots: &[SlotPlan; 3],
) -> Result<Identity, LoanError> {
    let [descriptors, layouts, masks] = slots;
    let root = *descriptors.keys().first().ok_or(LoanError::WrongLoan)?;
    let layout_version = CellLayoutVersion {
        scope: heap.scope,
        key: *layouts.keys().first().ok_or(LoanError::WrongLoan)?,
    };
    let mask_version = MaskVersion {
        scope: heap.scope,
        key: *masks.keys().first().ok_or(LoanError::WrongLoan)?,
        layout: layout_version,
    };
    Ok(Identity {
        scope: heap.scope,
        account,
        root,
        channels: [
            Some(root),
            descriptors.keys().get(1).copied(),
            descriptors.keys().get(2).copied(),
        ],
        layout: layout_version,
        mask: mask_version,
        request: plan.request,
        nonce: plan.nonce,
        declaration: prepared.declaration.key,
        receipt: plan.cleanup.identity,
        format: crate::storage::Format::of::<P>(),
    })
}

/// Build each native occupation once after all protected payloads exist.
fn records(
    plan: &Plan,
    identity: Identity,
    liveness: &Rc<LcLiveness>,
    slots: [SlotPlan; 3],
    guard: &mut Preparation,
) -> Result<Records, LoanError> {
    let [descriptors, layouts, masks] = slots;
    let mut records = vector(17)?;
    guard.acquired(true)?;
    for (index, bytes) in allocation_bytes(plan).into_iter().enumerate() {
        records.push(Debit {
            kind: AllocationKind::LcPending {
                witness: Rc::downgrade(liveness),
                header: index == 10,
            },
            owner: DebitOwner::LcApplication(crate::loan::registry::LcDebitIdentity::of(identity)),
            key: plan.keys[index],
            elements: if index < 3 { plan.cells } else { 0 },
            bytes,
        });
    }
    let mut descriptor_records = vector(3)?;
    guard.acquired(true)?;
    descriptor_records.extend(HostChannel::ALL.into_iter().map(|channel| {
        DescriptorRecord::Managed(ManagedDescriptor {
            identity,
            channel,
            filled: false,
        })
    }));
    let mut layout_records = vector(1)?;
    guard.acquired(true)?;
    layout_records.push(Native::Reserved(identity.root));
    let mut mask_records = vector(1)?;
    guard.acquired(true)?;
    mask_records.push(Native::Reserved(identity.root));
    let descriptors = descriptors.bind(descriptor_records)?;
    let layouts = layouts.bind(layout_records)?;
    let masks = masks.bind(mask_records)?;
    Ok(Records {
        descriptors,
        layouts,
        masks,
        debits: records,
    })
}

/// Acquire one fixed shared shell and immediately record its prepaid destruction.
fn shared<T>(value: T, guard: &mut Preparation) -> Result<Rc<T>, LoanError> {
    #[cfg(test)]
    crate::loan::fault::before()?;
    let owner = Rc::new(value);
    guard.acquired(false)?;
    Ok(owner)
}

/// Allocate the three numerical and six packed initialization backings.
fn channels<P: Scalar + 'static>(
    plan: &Plan,
    guard: &mut Preparation,
) -> Result<[LcChannel; 3], LoanError> {
    let mut channels = [None, None, None];
    for channel in &mut channels {
        let numerical = Numerical::zeroed::<P>(plan.cells)?;
        if plan.cells != 0 {
            guard.acquired(false)?;
        }
        let mut components = vector(plan.words)?;
        if plan.words != 0 {
            guard.acquired(false)?;
        }
        components.resize(plan.words, 0);
        let mut complete = vector(plan.words)?;
        if plan.words != 0 {
            guard.acquired(false)?;
        }
        complete.resize(plan.words, 0);
        *channel = Some(LcChannel {
            numerical,
            components,
            complete,
        });
    }
    let [Some(first), Some(second), Some(third)] = channels else {
        return Err(LoanError::WrongLoan);
    };
    Ok([first, second, third])
}

/// Allocation capacities follow the closed seventeen-role inventory.
const fn allocation_bytes(plan: &Plan) -> [usize; 17] {
    let word_bytes = plan.words * 8;
    [
        plan.slab,
        plan.slab,
        plan.slab,
        word_bytes,
        word_bytes,
        word_bytes,
        word_bytes,
        word_bytes,
        word_bytes,
        size_of::<Storage>(),
        rc_bytes::<LcLiveness>(),
        rc_bytes::<LayoutRecord>(),
        plan.cells * size_of::<LcOutcome>(),
        word_bytes,
        rc_bytes::<MaskRecord>(),
        rc_bytes::<Bundle>(),
        rc_bytes::<Use>(),
    ]
}

/// Move the original facade only after preparation has become infallible.
pub(super) fn install<'i, P: Scalar, const N: usize>(
    prepared: LcOmegaPrepared<'i, P, N>,
    heap: &mut Heap,
    demand: MaskVersion,
    transaction: Transaction,
) -> LcApplicationPending<'i, P, N> {
    let Transaction {
        identity,
        plan,
        records,
        channels,
        geometry,
        box_shell,
        bundle,
        initial_use,
        demand_mask,
        demand_layout,
        cleanup,
        liveness,
    } = transaction;
    let Geometry {
        outcomes,
        layout,
        completed,
    } = geometry;
    let Records {
        descriptors,
        layouts,
        masks,
        debits: records,
    } = records;
    let mut inventory = [None; 19];
    for (index, record) in records.iter().enumerate() {
        inventory[index] = Some((plan.slots[index], record.key));
    }
    inventory[17] = Some((prepared.receipt.header.slot, prepared.receipt.header.key));
    inventory[18] = Some((prepared.receipt.rows.slot, prepared.receipt.rows.key));
    let debits = debit_tokens(
        &plan,
        identity,
        prepared.receipt.header,
        prepared.receipt.rows,
    );
    let original_inventory = prepared.declaration.inventory;
    let storage = Box::write(
        box_shell,
        Storage {
            identity,
            schedule: prepared.receipt.schedule,
            symbols: prepared.declaration.symbols,
            demand,
            demand_mask,
            demand_layout,
            layout,
            completed,
            closure: prepared.receipt.closure,
            correspondence: prepared.correspondence,
            channels,
            outcomes,
            debits,
            cleanup,
            liveness,
        },
    );
    heap.descriptors.slots.publish(descriptors);
    heap.masks.layouts.publish(layouts);
    heap.masks.masks.publish(masks);
    while heap.loans.groups.len() < heap.descriptors.slots.scan_work() {
        heap.loans.groups.push(None);
    }
    heap.loans.groups[identity.root.slot] = Some(Group {
        identity,
        state: State::LcOut(Rc::downgrade(&storage.liveness)),
        outcome: ProducerOutcome::Pending,
        delivered: false,
        pin: 0,
        debits: DebitInventory::LcApplication(inventory),
    });
    for (index, record) in records.into_iter().enumerate() {
        heap.loans.install(plan.slots[index], record);
    }
    heap.loans.pending += 1;
    heap.loans.revision = plan.revision;
    LcApplicationPending {
        inventory: original_inventory,
        identity,
        demand,
        native: Some(PendingNative {
            bundle,
            initial_use,
            storage,
        }),
        cancelled: None,
    }
}

/// Move adopted tokens and create the seventeen fixed new token locators.
fn debit_tokens(plan: &Plan, identity: Identity, header: Token, rows: Token) -> LcDebits {
    let token = |index: usize| {
        Some(Token {
            slot: plan.slots[index],
            key: plan.keys[index],
            receipt: identity.receipt,
        })
    };
    LcDebits {
        numerical: [token(0), token(1), token(2)],
        tracking: [token(3), token(4), token(5), token(6), token(7), token(8)],
        storage: token(9),
        liveness: token(10),
        layout: token(11),
        outcomes: token(12),
        completed_words: token(13),
        completed_header: token(14),
        closure: Some(header),
        rows: Some(rows),
        bundle: token(15),
        initial_use: token(16),
    }
}
