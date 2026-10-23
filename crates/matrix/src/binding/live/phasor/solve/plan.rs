// SPDX-License-Identifier: AGPL-3.0-only
//! Exact equation, metrics and all stored source copies precede numerical access.

use super::super::{AT, PhasorError, PhasorOperand};
use crate::{Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::{PhasorResidualTransfer, PhasorSolveDefinition, algebra::limits::BudgetKind},
};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Complete prescribed LC solve, including both physical metrics and all source versions.
#[derive(Clone, Debug)]
pub struct PhasorSolve<P: Scalar>(Rc<Plan<P>>);

/// Shared immutable execution inventory.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Original equation and exact residual transport.
    transfer: PhasorResidualTransfer,
    /// Impedance then declared voltage, each with all four stored copies.
    operands: [PhasorOperand<P>; 2],
    /// Bounded exact reference import policy.
    literals: LiteralLimits,
    /// Retained evidence allowance beside numerical scratch.
    bytes: u64,
}

impl<P: Scalar> PhasorSolve<P> {
    /// Match both actual source representations against the prescribed original equation.
    /// # Errors
    /// Refuses substituted selections, contexts, operands and exhausted cumulative resources.
    pub fn new(
        transfer: PhasorResidualTransfer,
        operands: [PhasorOperand<P>; 2],
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, PhasorError> {
        let definition = transfer.definition();
        for (operand, expected) in operands
            .iter()
            .zip([definition.impedance(), definition.voltage()])
        {
            let actual = operand.whole().adapter().representation();
            crate::binding::adapters::reading_work(actual.reading(), budget)
                .map_err(crate::LiveBindingError::from)?;
            let reading = actual.reading() == expected.reading();
            let reconstruction = actual.reconstruction() == expected.reconstruction();
            if !reading || !reconstruction {
                return Err(PhasorError::Identity);
            }
        }
        let mut bytes = transfer
            .retained_payload_bytes()
            .checked_add(4096)
            .ok_or(HeapError::ExtentOverflow)?;
        for operand in &operands {
            for copy in operand.copies() {
                budget.work(1, AT)?;
                let proof = u64::try_from(
                    copy.adapter()
                        .representation()
                        .witness()
                        .exported_byte_len(),
                )
                .map_err(|_| HeapError::ExtentOverflow)?;
                bytes = bytes
                    .checked_add(proof.checked_mul(8).ok_or(HeapError::ExtentOverflow)?)
                    .ok_or(HeapError::ExtentOverflow)?;
            }
        }
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            transfer,
            operands,
            literals,
            bytes,
        })))
    }
    /// Original exact equation and complete current result slot.
    #[must_use]
    pub fn definition(&self) -> &PhasorSolveDefinition {
        self.0.transfer.definition()
    }
    /// The named residual metric and both immutable reference definitions.
    #[must_use]
    pub fn transfer(&self) -> &PhasorResidualTransfer {
        &self.0.transfer
    }
    /// All original impedance and voltage copies, with their independent numerical errors.
    #[must_use]
    pub fn operands(&self) -> &[PhasorOperand<P>; 2] {
        &self.0.operands
    }
    /// Complete retained metadata allowance charged beside every execution.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Bounded scalar import policy.
    pub(super) fn literals(&self) -> LiteralLimits {
        self.0.literals
    }
}

impl<P: Scalar + 'static> PhasorSolve<P> {
    /// Revalidate every whole family, copy and companion, including equal-value mutations.
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
