// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit original L and C positions feed one local enclosed multiplication.

mod frequency;
pub(in crate::loan) use frequency::LcKernelStage;
pub use frequency::{
    LcFrequencyPrelude, LcFrequencyTarget, LcQuantityDemand, LcRequestedCell, LcRequestedState,
    LcRequestedValues,
};

use super::{LcInputCell, LcInputCells, work};
use crate::{ExecutionAccount, KernelError, LoanError, NumericalErrorScope};
use nrz_bindings::LcQuantity;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Product binding, complete coverage or finite-image refusal.
#[derive(Debug, thiserror::Error)]
pub enum LcProductError {
    /// Original authority, representation or Work refused.
    #[error("LC product: {0}")]
    Loan(#[from] LoanError),
    /// Native arithmetic context or finite product image refused.
    #[error("LC product arithmetic: {0}")]
    Kernel(#[from] KernelError),
}

/// One tile's complete input authority and explicit original L/C positions.
#[derive(Debug)]
pub struct LcInputProduct<'a, P: Scalar, const N: usize> {
    /// Complete native input planes and immutable inventory authority.
    cells: LcInputCells<'a, P, N>,
    /// Original inductance then capacitance positions; never descriptor-based substitution.
    positions: [usize; 2],
    /// Native arithmetic assumptions checked once at tile binding.
    context: ArithmeticContext,
}

/// Local L×C result together with its complete original input observation.
#[derive(Debug)]
pub struct LcProductCell<'a, P: Scalar, const N: usize> {
    /// Every original role and its unchanged companions, axes and tuple identity.
    inputs: LcInputCell<'a, P, N>,
    /// Explicit original inductance then capacitance positions.
    positions: [usize; 2],
    /// Product center, complete total and local nominal numerical error.
    values: [P; 3],
}
impl<P: Scalar, const N: usize> LcProductCell<'_, P, N> {
    /// Complete original observation, including incoming numerical-only companions.
    #[must_use]
    pub const fn inputs(&self) -> &LcInputCell<'_, P, N> {
        &self.inputs
    }
    /// Original inductance then capacitance positions selected at tile binding.
    #[must_use]
    pub const fn positions(&self) -> [usize; 2] {
        self.positions
    }
    /// Nearest product center, Cartesian total radius and local rounding radius.
    #[must_use]
    pub const fn values(&self) -> [P; 3] {
        self.values
    }
    /// Only this multiplication's nominal rounding is bounded by the numerical channel.
    #[must_use]
    pub const fn numerical_error_scope(&self) -> NumericalErrorScope {
        NumericalErrorScope::LocalOperation
    }
}
impl<'a, P: Scalar + 'static, const N: usize> LcInputCells<'a, P, N> {
    /// Role and environment admission are funded by readiness before this binding.
    pub(in crate::loan) fn into_product_prepaid(
        self,
        positions: [usize; 2],
    ) -> Result<LcInputProduct<'a, P, N>, LcProductError> {
        for (position, quantity) in positions
            .into_iter()
            .zip([LcQuantity::Inductance, LcQuantity::Capacitance])
        {
            let member = self
                .readers
                .inventory
                .input(position)
                .ok_or(LoanError::UnsupportedContract)?;
            let input = member
                .question()
                .relation()
                .inputs()
                .get(position)
                .ok_or(LoanError::UnsupportedContract)?;
            if input.quantity() != quantity {
                return Err(LoanError::TargetMismatch.into());
            }
        }
        let context = ArithmeticContext::new().map_err(KernelError::from)?;
        Ok(LcInputProduct {
            cells: self,
            positions,
            context,
        })
    }

    /// Bind explicit original inductance and capacitance roles once for this tile.
    /// # Errors
    /// Refuses foreign accounts, missing or wrong roles, Work and arithmetic context.
    /// Refusal consumes the view; the original tiles remain with their caller.
    pub fn into_product(
        self,
        positions: [usize; 2],
        account: &mut ExecutionAccount,
    ) -> Result<LcInputProduct<'a, P, N>, LcProductError> {
        let original = self
            .readers
            .inventory
            .input(0)
            .ok_or(LoanError::UnsupportedContract)?;
        account.authenticate(original.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC input product binding", |budget| {
                crate::loan::charge(budget, 4)?;
                self.into_product_prepaid(positions)
            })
    }
}
impl<'a, P: Scalar + 'static, const N: usize> LcInputProduct<'a, P, N> {
    /// Observe every original role, then multiply the selected L/C total boxes.
    /// # Errors
    /// Authority, Work, holes and addresses refuse before loads; finite-image failure
    /// follows the complete observation and leaves all native sources unchanged.
    pub fn read_cell(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<LcProductCell<'a, P, N>, LcProductError> {
        let original = self
            .cells
            .readers
            .inventory
            .input(0)
            .ok_or(LoanError::UnsupportedContract)?;
        account.authenticate(original.account)?;
        account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "LC input product evaluation",
            |budget| {
                let visits = work::<N>(7)?
                    .checked_add(1)
                    .ok_or(LoanError::CountOverflow)?;
                crate::loan::charge(budget, visits)?;
                self.evaluate(self.cells.observe(cell)?)
            },
        )
    }
    /// Evaluate the original product after a caller prepays complete observation and invocation.
    fn evaluate(
        &self,
        inputs: LcInputCell<'a, P, N>,
    ) -> Result<LcProductCell<'a, P, N>, LcProductError> {
        let cell = inputs.cell();
        let [left, right] = self
            .positions
            .map(|position| inputs.readings()[position].values());
        let value = crate::kernels::binary::evaluate_selected(
            [left[0], right[0]],
            [left[1], right[1]],
            crate::BinaryElementwise::Product,
            cell,
            &self.context,
        )?;
        Ok(LcProductCell {
            inputs,
            positions: self.positions,
            values: [value.center, value.total, value.error],
        })
    }
}
