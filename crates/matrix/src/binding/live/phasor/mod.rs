// SPDX-License-Identifier: AGPL-3.0-only
//! Checked block products preserve every stored copy and its separate deterministic coverage.

mod input;
mod numeric;
mod plan;
mod publication;
mod solve;
pub use plan::{PhasorOperand, PhasorProduct};
pub use publication::{PhasorPublication, PublishedPhasor};
pub use solve::{
    PhasorAllocation, PhasorAllocationChannel, PhasorAllocationStage, PhasorInventory, PhasorSolve,
    PhasorSolveCertificate, PhasorSolveEvaluation, PhasorSolvePublication, PhasorTransferBounds,
    PhasorTransferredResidual, PublishedPhasorSolve,
};

use super::LiveBindingError;
use crate::{Coverage, Heap, HeapError};
use nrz_bindings::readings::algebra::{
    Fault, Location,
    limits::{BudgetKind, BudgetUsage},
};
use nrz_bindings::{
    Budget,
    readings::{PhasorBlockCorrespondence, ReadingError},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Shared cumulative accounting location for checked block execution.
const AT: Location = Location::new("phasor block execution");

/// Exact, numerical or live-source refusal before a complete product is returned.
#[derive(Clone, Debug, thiserror::Error)]
pub enum PhasorError {
    /// Original ordered correspondence or selected copy differs.
    #[error("phasor exact correspondence: {0}")]
    Reading(#[from] ReadingError),
    /// A source value, component or companion is stale or foreign.
    #[error("phasor live source: {0}")]
    Live(#[from] LiveBindingError),
    /// Finite storage or shape admission refused.
    #[error("phasor storage: {0}")]
    Heap(#[from] HeapError),
    /// Caller cumulative resources were exhausted.
    #[error("phasor resources: {0}")]
    Exact(#[from] Fault),
    /// Outward arithmetic could not produce finite complete coverage.
    #[error("phasor arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// The existing Real inverse, residual or perturbation theorem refused.
    #[error("phasor Real solve: {0}")]
    Solve(#[from] crate::SolveError),
    /// A private numerical owner lacks its original complete allocation authority.
    #[error("phasor private allocation: {0}")]
    Inventory(#[from] crate::PrivateAllocationError),
    /// Exact zero impedance is observable, but current and admittance are undefined.
    #[error("zero impedance: current and admittance are unavailable")]
    ZeroImpedance,
    /// The full declared reactance box is not separated from zero.
    #[error("reactance enclosure contains zero")]
    ReactanceNotSeparated,
    /// Component order, group, source identity or arithmetic revision differs.
    #[error("phasor execution identity mismatch")]
    Identity,
}

/// Complete immutable result of the actual ordered native Real block multiplication.
///
/// The four entries keep independent rounding errors. This snapshot retains the exact
/// original Complex product and every source version; it is not a new heap reading family,
/// a physical solve certificate or a statistical companion.
#[derive(Clone, Debug)]
pub struct PhasorEvaluation<P: Scalar> {
    /// Complete predeclared exact and numerical source inventory.
    product: PhasorProduct<P>,
    /// Complete logical-column center, total and numerical-error arrays.
    payload: Rc<numeric::Values<P>>,
    /// Original assurance scopes for the two operands.
    coverage: [Coverage; 2],
    /// Arithmetic theorem revision consumed by the outward operations.
    revision: u32,
    /// Caller ledger before any numerical source is read.
    before: BudgetUsage,
    /// Caller ledger after the complete numerical product.
    after: BudgetUsage,
}

impl<P: Scalar + 'static> PhasorEvaluation<P> {
    /// Four native matrix-product centers in logical column order.
    /// # Errors
    /// Refuses any stale source or a changed arithmetic revision.
    pub fn centers<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.payload.centers)
    }
    /// Complete deterministic radii, including the numerical errors without adding them twice.
    /// # Errors
    /// Refuses any stale source or a changed arithmetic revision.
    pub fn totals<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.payload.totals)
    }
    /// Per-entry numerical radii cover input copy discrepancies and product rounding.
    /// # Errors
    /// Refuses any stale source or a changed arithmetic revision.
    pub fn errors<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.payload.errors)
    }
    /// Actual original operation, whole-family proofs and ordered block correspondence.
    #[must_use]
    pub fn correspondence(&self) -> &PhasorBlockCorrespondence {
        self.product.correspondence()
    }
    /// Complete original source and selected-copy inventories.
    #[must_use]
    pub fn operands(&self) -> &[PhasorOperand<P>; 2] {
        self.product.operands()
    }
    /// Original deterministic coverage scopes; no statistical independence is inferred.
    #[must_use]
    pub const fn coverage(&self) -> [Coverage; 2] {
        self.coverage
    }
    /// Cumulative caller usage before and after numerical execution; coefficients are exact integers.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 2] {
        [self.before, self.after]
    }
    /// Revalidate all scalar companions and each original whole-family reconstruction.
    fn validate(&self, heap: &Heap, context: &ArithmeticContext) -> Result<(), PhasorError> {
        if self.revision != context.revision() {
            return Err(PhasorError::Identity);
        }
        self.product.validate(heap)
    }
}

impl Heap {
    /// Execute the actual ordered Real block product after exact correspondence and source admission.
    ///
    /// All centers, deterministic totals and numerical errors are private until complete.
    /// Repeated coordinates keep their own stored values and radii throughout contraction.
    /// # Errors
    /// Refuses stale sources, incomplete selected-copy companions, nonfinite arithmetic and limits.
    pub fn evaluate_phasor_product<P: Scalar + 'static>(
        &self,
        product: &PhasorProduct<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PhasorEvaluation<P>, PhasorError> {
        product.validate(self)?;
        let before = budget.usage();
        let retained = product.retained_payload_bytes();
        let (payload, coverage) = budget.with_retained_payload(retained, AT, |budget| {
            let bytes = u64::try_from(std::mem::size_of::<numeric::Values<P>>())
                .map_err(|_| HeapError::ExtentOverflow)?;
            budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(16), AT)?;
            let (left, first) = input::capture(self, &product.operands()[0], budget)?;
            let (right, second) = input::capture(self, &product.operands()[1], budget)?;
            let payload = numeric::multiply(&left, &right, context, budget)?;
            budget.check(
                BudgetKind::OutputBytes,
                u64::try_from(
                    std::mem::size_of::<PhasorEvaluation<P>>() + 2 * std::mem::size_of::<usize>(),
                )
                .map_err(|_| HeapError::ExtentOverflow)?
                .checked_add(bytes)
                .ok_or(HeapError::ExtentOverflow)?,
                AT,
            )?;
            Ok::<_, PhasorError>((payload, [first, second]))
        })?;
        Ok(PhasorEvaluation {
            product: product.clone(),
            payload: Rc::new(payload),
            coverage,
            revision: context.revision(),
            before,
            after: budget.usage(),
        })
    }
}

#[cfg(test)]
mod tests;
