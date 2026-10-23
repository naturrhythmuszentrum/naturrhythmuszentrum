// SPDX-License-Identifier: AGPL-3.0-only
//! Every selected scalar copy and its exact block operand are fixed before floating execution.

use super::{AT, Heap, HeapError, PhasorError, input};
use crate::ReadingBinding;
use nrz_bindings::readings::algebra::limits::BudgetKind;
use nrz_bindings::{Budget, readings::PhasorBlockCorrespondence};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// One exact whole block and all four scalar views admitted in the same atomic family import.
#[derive(Clone, Debug)]
pub struct PhasorOperand<P: Scalar> {
    /// Complete Complex block interpretation with its reconstruction proof.
    whole: ReadingBinding<P>,
    /// Actual stored copies in logical column order, including negative and repeated entries.
    copies: [ReadingBinding<P>; 4],
}

impl<P: Scalar> PhasorOperand<P> {
    /// Check each complete scalar view against its actual stored copy and whole-family identity.
    /// # Errors
    /// Refuses another group, selected copy, map, companion transformation or incomplete coverage.
    pub fn new(
        whole: ReadingBinding<P>,
        copies: [ReadingBinding<P>; 4],
        budget: &mut Budget,
    ) -> Result<Self, PhasorError> {
        input::check(&whole, &copies, budget)?;
        Ok(Self { whole, copies })
    }
    /// Complete original Complex reading and all stored-copy discrepancy records.
    #[must_use]
    pub const fn whole(&self) -> &ReadingBinding<P> {
        &self.whole
    }
    /// Four original scalar views, preserving their own center, total and numerical error.
    #[must_use]
    pub const fn copies(&self) -> &[ReadingBinding<P>; 4] {
        &self.copies
    }
}

/// Exact correspondence and complete ordered source inventory prepared before execution.
#[derive(Clone, Debug)]
pub struct PhasorProduct<P: Scalar>(Rc<Plan<P>>);

/// Shared immutable metadata preserves original graph and operand order.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Bindings-owned complete block law on the actual original operands.
    correspondence: PhasorBlockCorrespondence,
    /// Two independently versioned whole families with every scalar copy and companion.
    operands: [PhasorOperand<P>; 2],
    /// Complete retained evidence allowance beside numerical scratch.
    bytes: u64,
}

impl<P: Scalar> PhasorProduct<P> {
    /// Match the exact ordered operands to the block law without reading numerical storage.
    /// # Errors
    /// Refuses reordered, foreign or substituted readings and exhausted cumulative resources.
    pub fn new(
        correspondence: PhasorBlockCorrespondence,
        operands: [PhasorOperand<P>; 2],
        budget: &mut Budget,
    ) -> Result<Self, PhasorError> {
        let bytes = storage(&correspondence, &operands, budget)?;
        budget.with_retained_payload(bytes, AT, |budget| {
            correspondence.check_operands(
                &[
                    operands[0].whole().adapter().representation().clone(),
                    operands[1].whole().adapter().representation().clone(),
                ],
                budget,
            )?;
            Ok::<_, PhasorError>(())
        })?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            correspondence,
            operands,
            bytes,
        })))
    }
    /// Exact original Complex multiplication and its checked ordered Real block law.
    #[must_use]
    pub fn correspondence(&self) -> &PhasorBlockCorrespondence {
        &self.0.correspondence
    }
    /// Both complete original source inventories, in multiplication order.
    #[must_use]
    pub fn operands(&self) -> &[PhasorOperand<P>; 2] {
        &self.0.operands
    }
    /// Retained exact metadata charged beside each numerical evaluation.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

impl<P: Scalar + 'static> PhasorProduct<P> {
    /// Every scalar companion is revalidated even when current center bytes agree.
    pub(super) fn validate(&self, heap: &Heap) -> Result<(), PhasorError> {
        for operand in self.operands() {
            heap.validate_reading(operand.whole())?;
            for copy in operand.copies() {
                heap.validate_reading(copy)?;
            }
        }
        Ok(())
    }
}

/// Bound original physical metadata comparisons and retained representation certificates.
fn storage<P: Scalar>(
    law: &PhasorBlockCorrespondence,
    operands: &[PhasorOperand<P>; 2],
    budget: &mut Budget,
) -> Result<u64, PhasorError> {
    let mut bytes = law
        .retained_payload_bytes()
        .checked_add(2048)
        .ok_or(HeapError::ExtentOverflow)?;
    for operand in operands {
        for binding in std::iter::once(operand.whole()).chain(operand.copies()) {
            crate::binding::adapters::reading_work(binding.reading(), budget)
                .map_err(super::LiveBindingError::from)?;
            let size = u64::try_from(
                binding
                    .adapter()
                    .representation()
                    .witness()
                    .exported_byte_len(),
            )
            .map_err(|_| HeapError::ExtentOverflow)?;
            bytes = bytes
                .checked_add(size.checked_mul(8).ok_or(HeapError::ExtentOverflow)?)
                .ok_or(HeapError::ExtentOverflow)?;
        }
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}
