// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen native scalar inputs retain their original decision association and all radii.

use crate::{Dependency, Heap, HeapError, InputSnapshot, ValueBinding};
use nrz_bindings::readings::algebra::{
    Fault, Location,
    limits::{BudgetKind, BudgetUsage},
};
use nrz_bindings::{Budget, DecisionInputs};
use nrz_scalar::Scalar;

/// Accounting location shared by capture and current-version authentication.
const AT: Location = Location::new("matrix decision input snapshot");

/// A refused snapshot has no numerical publication or retained heap state.
#[derive(Debug, thiserror::Error)]
pub enum DecisionSnapshotError {
    /// Current numerical occupation or evidence does not match.
    #[error("decision snapshot: {0}")]
    Heap(#[from] HeapError),
    /// Complete existing native input capture refused.
    #[error("decision input capture: {0}")]
    Capture(Box<crate::OperationError>),
    /// The original physical input association does not match.
    #[error("decision input association: {0}")]
    Association(Box<nrz_bindings::DecisionError>),
    /// Caller Work or payload allowance was exhausted.
    #[error("decision snapshot resources: {0}")]
    Resources(#[from] Fault),
    /// This receiving subset requires scalar native inputs and both radius channels.
    #[error("decision snapshot requires native scalar inputs with total and numerical radii")]
    Unsupported,
}

/// Archived scalar coordinates and their complete exact dependency key.
#[derive(Debug)]
pub struct DecisionInputSnapshot<P: Scalar> {
    /// Original occupation, schema, selection and complete companion inventory.
    input: InputSnapshot<P>,
    /// Current-version validation uses the same complete record relation as publication.
    dependency: Dependency,
    /// Center, total radius and included numerical-only radius, in that order.
    coordinates: [P; 3],
}

impl<P: Scalar> DecisionInputSnapshot<P> {
    /// Historical source identity; this getter grants no current heap authority.
    #[must_use]
    pub const fn input(&self) -> &InputSnapshot<P> {
        &self.input
    }
    /// Frozen center, total and included numerical error; no count is stored here.
    #[must_use]
    pub const fn coordinates(&self) -> [P; 3] {
        self.coordinates
    }
}

/// Complete original input association and the heap versions read together under one borrow.
///
/// Inspection is historical. Reuse requires `Heap::check_decision_inputs`; this primitive
/// establishes no numerical enclosure theorem, selected Integer or published attachment.
#[derive(Debug)]
pub struct FrozenDecisionInputs<P: Scalar> {
    /// Allocating numerical authority, including for an empty original input inventory.
    scope: u64,
    /// Complete immutable source, exact maps, case and parameter association.
    association: DecisionInputs,
    /// Canonically ordered originals with their actual numerical versions and channels.
    operands: Vec<DecisionInputSnapshot<P>>,
    /// Newly owned native metadata and conservative map payload allowance.
    bytes: u64,
    /// Cumulative caller usage before and after capture.
    usage: [BudgetUsage; 2],
}

impl<P: Scalar> FrozenDecisionInputs<P> {
    /// Exact physical source and designated value/companion declarations.
    #[must_use]
    pub const fn association(&self) -> &DecisionInputs {
        &self.association
    }
    /// Complete ordered historical input set, including both radius channels.
    #[must_use]
    pub fn operands(&self) -> &[DecisionInputSnapshot<P>] {
        &self.operands
    }
    /// Snapshot payload allowance, excluding already retained exact declaration graphs.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }
    /// Caller ledger snapshots; every failed attempt also spends on that same ledger.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 2] {
        self.usage
    }
}

impl Heap {
    /// Freeze every designated native scalar and both companions without reserving outputs.
    ///
    /// The immutable heap borrow covers all association, version and numerical reads.
    /// Exact maps remain in the original association; input coordinates are coherent.
    /// # Errors
    /// Refuses missing, reordered, foreign or stale bindings, changed original symbols,
    /// unsupported selections, incomplete companions and caller limits atomically.
    pub fn freeze_decision_inputs<P: Scalar + 'static>(
        &self,
        association: &DecisionInputs,
        inputs: &[ValueBinding<P>],
        budget: &mut Budget,
    ) -> Result<FrozenDecisionInputs<P>, DecisionSnapshotError> {
        let before = budget.usage();
        let bytes = snapshot_bytes::<P, FrozenDecisionInputs<P>>(inputs.len())?;
        let operands = capture(self, inputs, bytes, budget, |symbols, budget| {
            association.authenticate(association.case(), symbols, budget)
        })?;
        Ok(FrozenDecisionInputs {
            scope: self.scope,
            association: association.clone(),
            operands,
            bytes,
            usage: [before, budget.usage()],
        })
    }

    /// Authenticate the complete frozen dependency set against its current numerical owner.
    /// # Errors
    /// Refuses foreign owners, stale values, alias writes, companion-only changes and limits.
    /// Archived coordinates remain inspectable and cannot be promoted to current evidence.
    pub fn check_decision_inputs<P: Scalar + 'static>(
        &self,
        frozen: &FrozenDecisionInputs<P>,
        budget: &mut Budget,
    ) -> Result<(), DecisionSnapshotError> {
        check(self, frozen.scope, &frozen.operands, budget)
    }
}

/// Charge the fixed native capture schedule plus conservative complete provenance scans.
fn charge_scan(heap: &Heap, budget: &mut Budget) -> Result<(), DecisionSnapshotError> {
    let usage = heap.provenance_usage();
    let traversal = count(usage.work())?
        .checked_add(count(usage.records())?)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_mul(32))
        .ok_or(HeapError::Capacity)?;
    budget.work(traversal, AT)?;
    Ok(())
}

/// Checked counts never truncate into a smaller allowance.
fn count(value: usize) -> Result<u64, HeapError> {
    u64::try_from(value).map_err(|_| HeapError::Capacity)
}

/// Native fixed-size metadata is accounted without scanning shared exact graphs.
fn count_size<T>() -> Result<u64, HeapError> {
    count(size_of::<T>())
}

/// Account the same per-source native metadata for either immutable association wrapper.
pub(super) fn snapshot_bytes<P: Scalar, T>(length: usize) -> Result<u64, HeapError> {
    count(length)?
        .checked_mul(count_size::<DecisionInputSnapshot<P>>()?.saturating_add(16))
        .and_then(|value| value.checked_add(count_size::<T>().ok()?))
        .ok_or(HeapError::Capacity)
}

/// Authenticate all symbols first, then capture the complete ordered native inventory.
pub(super) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    inputs: &[ValueBinding<P>],
    bytes: u64,
    budget: &mut Budget,
    authenticate: impl FnOnce(
        &[&nrz_bindings::BindingSymbols],
        &mut Budget,
    ) -> Result<(), nrz_bindings::DecisionError>,
) -> Result<Vec<DecisionInputSnapshot<P>>, DecisionSnapshotError> {
    let count = count(inputs.len())?;
    let staged = bytes
        .checked_add(count.saturating_mul(8))
        .ok_or(HeapError::Capacity)?;
    budget.check(BudgetKind::MatrixEntries, count, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, staged, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    let symbols = inputs.iter().map(ValueBinding::symbols).collect::<Vec<_>>();
    authenticate(&symbols, budget)
        .map_err(|cause| DecisionSnapshotError::Association(Box::new(cause)))?;
    let mut operands = Vec::new();
    operands
        .try_reserve_exact(inputs.len())
        .map_err(|_| HeapError::Capacity)?;
    for binding in inputs {
        operands.push(capture_operand(heap, binding, budget)?);
    }
    Ok(operands)
}

/// Preserve every native value, total-radius and numerical-companion dependency.
fn capture_operand<P: Scalar + 'static>(
    heap: &Heap,
    binding: &ValueBinding<P>,
    budget: &mut Budget,
) -> Result<DecisionInputSnapshot<P>, DecisionSnapshotError> {
    charge_scan(heap, budget)?;
    heap.refuse_joint_operation(binding.handle())
        .map_err(|cause| DecisionSnapshotError::Capture(Box::new(cause)))?;
    let value = heap.value_by_symbol(binding.symbol(), binding)?;
    let native_scalar = value.region().extent().entries() == 1
        && heap.symbols.selection(binding.handle().key).is_none();
    if !native_scalar {
        return Err(DecisionSnapshotError::Unsupported);
    }
    let center = value.region().get(0, 0)?;
    let total = heap
        .selected_bound(binding.handle())?
        .ok_or(DecisionSnapshotError::Unsupported)?
        .region()
        .get(0, 0)?;
    let error = heap
        .companion(
            value
                .evaluation()
                .ok_or(DecisionSnapshotError::Unsupported)?,
        )?
        .region()
        .get(0, 0)?;
    let input = InputSnapshot::capture(heap, binding.handle())
        .map_err(|cause| DecisionSnapshotError::Capture(Box::new(cause)))?;
    let dependency = Dependency::from_snapshot(&input);
    Ok(DecisionInputSnapshot {
        input,
        dependency,
        coordinates: [center, total, error],
    })
}

/// Recheck the identical complete lease relation for scalar and lattice inventories.
pub(super) fn check<P: Scalar + 'static>(
    heap: &Heap,
    scope: u64,
    operands: &[DecisionInputSnapshot<P>],
    budget: &mut Budget,
) -> Result<(), DecisionSnapshotError> {
    budget.work(1, AT)?;
    if scope != heap.scope {
        return Err(HeapError::ForeignHandle.into());
    }
    for operand in operands {
        charge_scan(heap, budget)?;
        heap.refuse_joint_operation(operand.input.handle())
            .map_err(|cause| DecisionSnapshotError::Capture(Box::new(cause)))?;
        operand.dependency.validate(heap)?;
        heap.value(operand.input.handle())?;
    }
    Ok(())
}
