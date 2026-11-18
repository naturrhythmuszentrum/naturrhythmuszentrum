// SPDX-License-Identifier: AGPL-3.0-only
//! Owning original declarations; no mathematical or native authority.

use super::{AT, LcOmegaOriginals, LcOmegaOriginalsRequest, same_symbols};
use crate::readings::EntryMeaning;
use crate::{AdmissionError, BinaryOp, BindingSymbols, LcForwardCover, LcQuantity, Registry};
use nrz_cas::limits::{Budget, BudgetKind};
use std::{fmt, rc::Rc};

/// Indexed originals borrowed without a temporary array of cloned bundles.
/// The accessor is invoked once per required position during admission.
pub struct LcOmegaDeclarationRequest<'a, F> {
    /// Original checked question allocation.
    pub question: &'a LcForwardCover,
    /// Number of original positional pairs.
    pub input_len: usize,
    /// Lend one source/mapped pair, or refuse an absent position.
    pub input_at: F,
    /// Selected original L and C positions.
    pub positions: [usize; 2],
    /// Instance, sample and branch cardinalities.
    pub counts: [usize; 3],
    /// Already declared private requested result.
    pub output: &'a BindingSymbols,
    /// Exact original one-radian interpretation.
    pub radian: &'a EntryMeaning,
    /// Original registry admitting the ordered kind relation.
    pub registry: &'a Registry,
}

impl<F> fmt::Debug for LcOmegaDeclarationRequest<'_, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaDeclarationRequest")
            .field("input_len", &self.input_len)
            .field("positions", &self.positions)
            .field("counts", &self.counts)
            .finish_non_exhaustive()
    }
}

/// Sealed owning physical declaration; no root-law or publication authority.
///
/// Sharing prepays both acquisition and release; there is no free `Clone`.
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaDeclarationToken;
/// fn escape(token: LcOmegaDeclarationToken) { let _ = token.symbols(); }
/// ```
///
/// ```compile_fail,E0423
/// use nrz_bindings::LcOmegaDeclarationToken;
/// fn forge() { let _ = LcOmegaDeclarationToken(0); }
/// ```
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaDeclarationToken;
/// fn duplicate(token: LcOmegaDeclarationToken) { let _ = token.clone(); }
/// ```
pub struct LcOmegaDeclarationToken(Rc<Originals>);

/// One captured inventory; all existing physical and graph allocations stay shared.
struct Originals {
    /// Exact question keeps its relation, query and branch domain live.
    question: LcForwardCover,
    /// One retained pair buffer, never returned to callers.
    inputs: Vec<[BindingSymbols; 2]>,
    /// Ordered physical operands.
    positions: [usize; 2],
    /// Fixed tuple specialization.
    counts: [usize; 3],
    /// Private requested result and companions.
    output: BindingSymbols,
    /// Exact angle definition, retaining its original reference.
    radian: EntryMeaning,
    /// Compact exact coherent descriptors validated against the supplied registry.
    quantities: [LcQuantity; 3],
    /// Ordered operation validated at admission, not a registry proof token.
    operation: BinaryOp,
}

impl fmt::Debug for LcOmegaDeclarationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaDeclarationToken")
            .field("positions", &self.0.positions)
            .field("counts", &self.0.counts)
            .finish_non_exhaustive()
    }
}

impl LcOmegaDeclarationToken {
    /// Capture each indexed row once, validate originals, and publish one shared owner.
    /// Existing original residence must remain reserved by the caller on this ledger.
    /// # Errors
    /// Preserves physical, alias, companion and cumulative resource refusals.
    pub fn admit<'a, F>(
        request: &LcOmegaDeclarationRequest<'a, F>,
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError>
    where
        F: Fn(usize) -> Option<&'a [BindingSymbols; 2]>,
    {
        budget.with_work_phase("LC omega declaration", "admission", |budget| {
            budget.work(1, AT)?;
            if request.input_len != request.question.relation().inputs().len() {
                return Err(AdmissionError::ValueMismatch);
            }
            budget.check(BudgetKind::MatrixEntries, request.input_len as u64, AT)?;
            let bytes = retained_bytes(request.input_len)?;
            admission_capacity(bytes, budget)?;
            budget.work(2, AT)?; // Pair allocation and eventual cleanup.
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(request.input_len)
                .map_err(|_| AdmissionError::ResourceLimit)?;
            let bytes = retained_bytes(inputs.capacity())?;
            admission_capacity(bytes, budget)?;
            budget.with_retained_payload(bytes, AT, |budget| {
                budget.work(10, AT)?; // Question, radian and three output edges, including release.
                budget.work(4, AT)?; // Three fixed descriptor tags and their ordered operation.
                let mut original = Originals {
                    question: request.question.clone(),
                    inputs,
                    positions: request.positions,
                    counts: request.counts,
                    output: request.output.clone(),
                    radian: request.radian.clone(),
                    quantities: [
                        LcQuantity::Rate,
                        LcQuantity::InitialPhase,
                        LcQuantity::AngularFrequency,
                    ],
                    operation: BinaryOp::Multiply,
                };
                for index in 0..request.input_len {
                    budget.work(1, AT)?;
                    let pair = (request.input_at)(index).ok_or(AdmissionError::ValueMismatch)?;
                    budget.work(12, AT)?; // Six symbol edges and their eventual releases.
                    original.inputs.push(pair.clone());
                }
                distinct(&original, budget)?;
                LcOmegaOriginals::check(original.request(request.registry), budget)?;
                budget.work(2, AT)?; // Shared allocation and eventual cleanup.
                budget.work(2, AT)?; // First token edge and its eventual release.
                Ok(Self(Rc::new(original)))
            })
        })
    }

    /// Acquire a shared token with its release prepaid on the original ledger.
    /// # Errors
    /// Refuses exhausted Work or incremental token capacity before cloning its edge.
    pub fn share(&self, budget: &mut Budget) -> Result<Self, AdmissionError> {
        budget.with_work_phase("LC omega declaration", "sharing", |budget| {
            budget.with_retained_payload(retained_bytes(self.0.inputs.capacity())?, AT, |budget| {
                budget.check(BudgetKind::OutputBytes, size_of::<Self>() as u64, AT)?;
                budget.check(BudgetKind::StagedPayloadBytes, size_of::<Self>() as u64, AT)?;
                budget.work(2, AT)?;
                Ok(Self(Rc::clone(&self.0)))
            })
        })
    }

    /// Authenticate the exact declaration allocation, not structural equality.
    /// # Errors
    /// Refuses a separate declaration or an exhausted cumulative ledger.
    pub fn check_same(&self, expected: &Self, budget: &mut Budget) -> Result<(), AdmissionError> {
        budget.with_work_phase("LC omega declaration", "identity", |budget| {
            budget.work(1, AT)?;
            if Rc::ptr_eq(&self.0, &expected.0) {
                Ok(())
            } else {
                Err(AdmissionError::ValueMismatch)
            }
        })
    }

    /// Compare independently held indexed originals without exporting retained symbols.
    /// The caller retains its existing baseline; this method reserves the owner increment.
    /// # Errors
    /// Preserves exact question, value, companion, registry and resource refusal causes.
    pub fn check_originals<'a, F>(
        &self,
        expected: &LcOmegaDeclarationRequest<'a, F>,
        budget: &mut Budget,
    ) -> Result<(), AdmissionError>
    where
        F: Fn(usize) -> Option<&'a [BindingSymbols; 2]>,
    {
        budget.with_work_phase("LC omega declaration", "comparison", |budget| {
            budget.with_retained_payload(retained_bytes(self.0.inputs.capacity())?, AT, |budget| {
                compare(&self.0, expected, budget)
            })
        })
    }
}

impl Originals {
    /// Reuse unchanged physical checks against the captured immutable inventory.
    fn request<'a>(&'a self, registry: &'a Registry) -> LcOmegaOriginalsRequest<'a> {
        LcOmegaOriginalsRequest {
            question: &self.question,
            inputs: &self.inputs,
            positions: self.positions,
            counts: self.counts,
            output: &self.output,
            radian: &self.radian,
            registry,
        }
    }
}

/// New inline owner, shared control block, first token and captured pair capacity.
fn retained_bytes(capacity: usize) -> Result<u64, AdmissionError> {
    let fixed =
        size_of::<Originals>() + 2 * size_of::<usize>() + size_of::<LcOmegaDeclarationToken>();
    capacity
        .checked_mul(size_of::<[BindingSymbols; 2]>())
        .and_then(|bytes| bytes.checked_add(fixed))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(AdmissionError::ResourceLimit)
}

/// Conservative simultaneous sealing envelope plus the unchanged checked-view scratch.
fn admission_capacity(bytes: u64, budget: &Budget) -> Result<(), AdmissionError> {
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    let staged = bytes
        .checked_add(size_of::<LcOmegaOriginals<'_>>() as u64)
        .ok_or(AdmissionError::ResourceLimit)?;
    budget.check(BudgetKind::StagedPayloadBytes, staged, AT)?;
    Ok(())
}

/// Repeated physical coordinates may not merge original declaration handles.
fn distinct(original: &Originals, budget: &mut Budget) -> Result<(), AdmissionError> {
    for (index, value) in original.inputs.iter().flatten().enumerate() {
        for previous in original.inputs.iter().flatten().take(index) {
            budget.work(1, AT)?;
            if value.value() == previous.value() {
                return Err(AdmissionError::ValueMismatch);
            }
        }
    }
    for value in original.inputs.iter().flatten() {
        budget.work(1, AT)?;
        if original.output.value() == value.value() {
            return Err(AdmissionError::ResultMismatch);
        }
    }
    Ok(())
}

/// One fixed visit, one accessor per row, every symbol edge and the checked registry relation.
fn compare<'a, F>(
    original: &Originals,
    expected: &LcOmegaDeclarationRequest<'a, F>,
    budget: &mut Budget,
) -> Result<(), AdmissionError>
where
    F: Fn(usize) -> Option<&'a [BindingSymbols; 2]>,
{
    budget.work(1, AT)?;
    if !original.question.shares_allocation(expected.question)
        || original.positions != expected.positions
        || original.counts != expected.counts
        || !original.radian.shares_definition(expected.radian)
        || original.inputs.len() != expected.input_len
    {
        return Err(AdmissionError::ValueMismatch);
    }
    for (index, pair) in original.inputs.iter().enumerate() {
        budget.work(1, AT)?;
        let other = (expected.input_at)(index).ok_or(AdmissionError::ValueMismatch)?;
        for (value, expected) in pair.iter().zip(other) {
            same_symbols(value, expected, false, budget)?;
        }
    }
    same_symbols(&original.output, expected.output, true, budget)?;
    budget.work(1, AT)?;
    let [rate, angle, omega] = original.quantities;
    if expected.registry.binary(
        original.operation,
        &rate.descriptor()?,
        &angle.descriptor()?,
    )? != omega.descriptor()?
    {
        return Err(AdmissionError::ValueMismatch);
    }
    Ok(())
}
