// SPDX-License-Identifier: AGPL-3.0-only
//! Certified LC current blocks reuse the Real solve theorem and retain the original source closure.

mod constants;
mod execution;
mod inventory;
pub use inventory::{
    PhasorAllocation, PhasorAllocationChannel, PhasorAllocationStage, PhasorInventory,
};
mod numeric;
mod residual;
pub use residual::PhasorTransferredResidual;
mod plan;
mod publication;
mod transfer;
mod verification;
pub use numeric::PhasorSolveCertificate;
pub use plan::PhasorSolve;
pub use publication::{PhasorSolvePublication, PublishedPhasorSolve};
pub use transfer::PhasorTransferBounds;

use super::{AT, PhasorError};
use crate::{Coverage, Heap};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetUsage};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Complete certified current snapshot with coherent physical centers, totals and numerical errors.
#[derive(Clone, Debug)]
pub struct PhasorSolveEvaluation<P: Scalar> {
    /// Original checked operation and every live source version.
    plan: PhasorSolve<P>,
    /// Independently evaluated current copies and both overlapping radius channels.
    contents: Rc<Contents<P>>,
    /// Complete immutable native allocation receipt, sharing original meanings.
    inventory: PhasorInventory<P>,
    /// Original impedance and voltage assurance scopes.
    coverage: [Coverage; 2],
    /// Arithmetic theorem revision.
    revision: u32,
    /// Before lowering, before numerical access and after complete solve, on one caller ledger.
    usage: [BudgetUsage; 3],
}

/// Final numerical owners are shared together; cloning an evaluation creates no floating copies.
#[derive(Debug)]
struct Contents<P: Scalar> {
    /// Four current centers and both overlapping radius channels.
    payload: inventory::Owner<P, super::numeric::Values<P>>,
    /// Real inverse, residual and perturbation proof fields.
    certificate: inventory::Owner<P, PhasorSolveCertificate<P>>,
    /// Matching exact metric transport enclosures.
    transfer: inventory::Owner<P, PhasorTransferBounds<P>>,
    /// Reconstructed residual and current bounds.
    residual: inventory::Owner<P, PhasorTransferredResidual<P>>,
}

impl<P: Scalar + 'static> PhasorSolveEvaluation<P> {
    /// Four independently restored coherent current centers in logical column order.
    /// # Errors
    /// Refuses stale sources, companions or arithmetic revisions.
    pub fn centers<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.contents.payload.centers)
    }
    /// Complete deterministic radii including numerical errors once.
    /// # Errors
    /// Refuses stale sources, companions or arithmetic revisions.
    pub fn totals<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.contents.payload.totals)
    }
    /// Numerical-only radii, including every input copy discrepancy and the solve error.
    /// # Errors
    /// Refuses stale sources, companions or arithmetic revisions.
    pub fn errors<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P; 4], PhasorError> {
        self.validate(heap, context)?;
        Ok(&self.contents.payload.errors)
    }
    /// Historical Real solve evidence; further execution still requires live dependencies.
    #[must_use]
    pub fn certificate(&self) -> &PhasorSolveCertificate<P> {
        &self.contents.certificate
    }
    /// Enclosed norm factors with the exact convention retained by `Self::plan`.
    #[must_use]
    pub fn transfer(&self) -> &PhasorTransferBounds<P> {
        &self.contents.transfer
    }
    /// Reconstructed physical residual and its current component bound in the named metrics.
    #[must_use]
    pub fn transferred_residual(&self) -> &PhasorTransferredResidual<P> {
        &self.contents.residual
    }
    /// Completed native array, source-copy and verifier inventory with exact observed bytes.
    #[must_use]
    pub const fn inventory(&self) -> &PhasorInventory<P> {
        &self.inventory
    }
    /// Original prescribed equation, metrics and all source copies.
    #[must_use]
    pub const fn plan(&self) -> &PhasorSolve<P> {
        &self.plan
    }
    /// Original assurance scopes; generated totals cannot strengthen them.
    #[must_use]
    pub const fn coverage(&self) -> [Coverage; 2] {
        self.coverage
    }
    /// Cumulative lowering and solve usage, without resetting the caller's budget.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 3] {
        self.usage
    }
    /// Revalidate the complete selected source closure.
    fn validate(&self, heap: &Heap, context: &ArithmeticContext) -> Result<(), PhasorError> {
        if self.revision != context.revision() {
            return Err(PhasorError::Identity);
        }
        self.plan.validate(heap)
    }
}

impl Heap {
    /// Solve `R(Z)*R(I)=R(V)` after checked LC lowering and full-family sign separation.
    ///
    /// Coherent physical coordinates restore identically. Every original stored copy enters
    /// the Real verifier independently, and both overlapping radius channels are preserved.
    /// # Errors
    /// Refuses zero/straddling reactance, stale inputs, failed Real proof margins and limits.
    pub fn evaluate_phasor_solve<P: Scalar + 'static>(
        &self,
        plan: &PhasorSolve<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PhasorSolveEvaluation<P>, PhasorError> {
        plan.validate(self)?;
        let run = inventory::Run::new(plan, false, budget)?;
        execution::evaluate(self, plan, &run, context, budget)
    }
}

#[cfg(test)]
mod tests;
