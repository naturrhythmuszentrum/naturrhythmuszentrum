// SPDX-License-Identifier: AGPL-3.0-only
//! A requested omega declaration precedes storage; pending storage grants no computation proof.

pub(super) mod bridge;
mod pending;
use super::{LcInputInventory, LcQuantityDemand, LcShapeError};
use crate::{ExecutionAccount, LoanError, NumericalErrorScope};
pub use bridge::{LcOmegaPrepareFailure, LcOmegaPrepared};
use nrz_bindings::{BindingSymbols, Descriptor, LcQuantity, Registry, readings::EntryMeaning};
use nrz_scalar::Scalar;
pub use pending::{LcOutputCellState, LcOutputPending, LcOutputPublicationError};

/// Proposed immutable output inventory; statistical output needs a different admitted owner.
#[derive(Debug)]
pub struct LcOutputDeclaration<'a> {
    /// Original ordered inductance and capacitance constraint positions.
    pub positions: [usize; 2],
    /// Requested quantities are specialization data, never per-cell mask bits.
    pub quantities: LcQuantityDemand,
    /// Statistical companions are explicitly refused by this deterministic slice.
    pub statistical: bool,
    /// Exact source-free one-radian operand, retained by the checked interpretation.
    pub radian: EntryMeaning,
    /// Original registry must admit rate times angle as angular frequency.
    pub registry: &'a Registry,
}

/// Immutable omega projection beside the original complete forward-question inventory.
///
/// The declared equation is omega = a * (1 rad), with a = 1/sqrt(L*C).
/// This is a pending interpretation obligation, not a certified projected answer.
/// Represented-input outer cover; `LocalOperation` numerical error;
/// feasibility and determinacy not asserted. Numerical publication is unavailable.
///
/// Output symbols are private so ordinary host import cannot acquire their identities.
/// Both scalar formats reject the host-import route at the public API boundary:
///
/// ```compile_fail,E0624
/// use nrz_matrix::{LcOmegaOutput, HostTileDeclaration, CellAxis, Coverage};
/// use nrz_matrix::ExecutionAccount;
/// fn import(schema: &LcOmegaOutput<'_, f32, 5>, axis: CellAxis, account: &mut ExecutionAccount) {
///     let _ = HostTileDeclaration::<f32>::check(
///         schema.symbols().clone(), axis, Coverage::RepresentedInputs, account);
/// }
/// ```
///
/// ```compile_fail,E0624
/// use nrz_matrix::{LcOmegaOutput, HostTileDeclaration, CellAxis, Coverage};
/// use nrz_matrix::ExecutionAccount;
/// fn import(schema: &LcOmegaOutput<'_, f64, 5>, axis: CellAxis, account: &mut ExecutionAccount) {
///     let _ = HostTileDeclaration::<f64>::check(
///         schema.symbols().clone(), axis, Coverage::RepresentedInputs, account);
/// }
/// ```
#[derive(Debug)]
pub struct LcOmegaOutput<'a, P: Scalar, const N: usize> {
    /// Retain every original constraint and fixed tuple specialization by borrowing its owner.
    pub(in crate::loan) inventory: &'a LcInputInventory<P, N>,
    /// Ordered original operands of the declared positive-root rate expression.
    pub(in crate::loan) positions: [usize; 2],
    /// Explicit exact angle operand distinguishes omega from its equal-coordinate rate.
    radian: EntryMeaning,
    /// Coherent inverse-time operand of the retained ordered multiplication relation.
    rate: Descriptor,
    /// Fresh angular-frequency value and own-target deterministic companions.
    pub(in crate::loan) symbols: BindingSymbols,
    /// Nonwrapping identity fixes this requested-quantity specialization.
    pub(in crate::loan) key: u64,
}
impl<'a, P: Scalar, const N: usize> LcOmegaOutput<'a, P, N> {
    /// Declare the omega-only deterministic inventory before any numerical reservation.
    /// # Errors
    /// Refuses other demands, statistics, original roles, angle/rate meanings and resources.
    pub fn declare(
        inventory: &'a LcInputInventory<P, N>,
        request: LcOutputDeclaration<'_>,
        account: &mut ExecutionAccount,
    ) -> Result<Self, LcShapeError> {
        let original = inventory.input(0).ok_or(LoanError::UnsupportedContract)?;
        account.authenticate(original.account)?;
        let (rate, symbols) = account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "LC omega declaration",
            |budget| {
                crate::loan::charge(budget, 7)?;
                if request.quantities != LcQuantityDemand::Omega || request.statistical {
                    return Err(LoanError::UnsupportedContract.into());
                }
                for (position, quantity) in request
                    .positions
                    .into_iter()
                    .zip([LcQuantity::Inductance, LcQuantity::Capacitance])
                {
                    let member = inventory.input(position).ok_or(LoanError::TargetMismatch)?;
                    if member.question().relation().inputs()[position].quantity() != quantity {
                        return Err(LoanError::TargetMismatch.into());
                    }
                }
                let units =
                    |error| LcShapeError::Declaration(nrz_bindings::AdmissionError::Units(error));
                let angle = LcQuantity::InitialPhase.descriptor().map_err(units)?;
                let rate = LcQuantity::Rate.descriptor().map_err(units)?;
                if request.radian.descriptor() != &angle
                    || request.radian.magnitude().as_rational()
                        != Some(&nrz_bindings::readings::algebra::Rational::one())
                {
                    return Err(LoanError::TargetMismatch.into());
                }
                let omega = request
                    .registry
                    .binary(
                        nrz_bindings::BinaryOp::Multiply,
                        &rate,
                        request.radian.descriptor(),
                    )
                    .map_err(units)?;
                if omega != LcQuantity::AngularFrequency.descriptor().map_err(units)? {
                    return Err(LoanError::TargetMismatch.into());
                }
                let tuples = original
                    .counts
                    .into_iter()
                    .try_fold(1_usize, |count, next| {
                        count.checked_mul(next).ok_or(LoanError::CountOverflow)
                    })?;
                let tuples = u32::try_from(tuples).map_err(|_| LoanError::CountOverflow)?;
                let symbols = BindingSymbols::declare(&omega, [tuples, 1], [true; 2], budget)?;
                Ok::<_, LcShapeError>((rate, symbols))
            },
        )?;
        Ok(Self {
            inventory,
            positions: request.positions,
            radian: request.radian,
            rate,
            symbols,
            key: crate::heap::identity::identity().map_err(LoanError::from)?,
        })
    }
    /// Complete original question and positional inventory, unchanged by output declaration.
    #[must_use]
    pub const fn inventory(&self) -> &'a LcInputInventory<P, N> {
        self.inventory
    }
    /// Original ordered L/C selections defining the pending rate expression.
    #[must_use]
    pub const fn positions(&self) -> [usize; 2] {
        self.positions
    }
    /// Requested omega is fixed for this specialization; only demand membership can vary.
    #[must_use]
    pub const fn quantities(&self) -> LcQuantityDemand {
        LcQuantityDemand::Omega
    }
    /// Exact source-free angle in omega = a * (1 rad).
    #[must_use]
    pub const fn radian(&self) -> &EntryMeaning {
        &self.radian
    }
    /// Inverse-time operand is distinct from the output's angular-frequency kind.
    #[must_use]
    pub const fn rate_descriptor(&self) -> &Descriptor {
        &self.rate
    }
    /// Internal reservation symbols; never expose this set or its reconstructible symbols.
    #[must_use]
    pub(crate) const fn symbols(&self) -> &BindingSymbols {
        &self.symbols
    }
    /// Requested quantity, original roles and input specialization have one immutable identity.
    #[must_use]
    pub const fn specialization_key(&self) -> u64 {
        self.key
    }
    /// Represented-input outer cover; feasibility and determinacy not asserted.
    #[must_use]
    pub const fn numerical_error_scope(&self) -> NumericalErrorScope {
        NumericalErrorScope::LocalOperation
    }
}
