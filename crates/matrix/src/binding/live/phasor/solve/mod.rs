// SPDX-License-Identifier: AGPL-3.0-only
//! Certified LC current blocks reuse the Real solve theorem and retain the original source closure.

mod constants;
mod numeric;
mod residual;
pub use residual::PhasorTransferredResidual;
mod plan;
mod publication;
mod transfer;
pub use numeric::PhasorSolveCertificate;
pub use plan::PhasorSolve;
pub use publication::{PhasorSolvePublication, PublishedPhasorSolve};
pub use transfer::PhasorTransferBounds;

use super::{AT, PhasorError, input};
use crate::{Coverage, Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::algebra::limits::{BudgetKind, BudgetUsage},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Complete certified current snapshot with coherent physical centers, totals and numerical errors.
#[derive(Clone, Debug)]
pub struct PhasorSolveEvaluation<P: Scalar> {
    /// Original checked operation and every live source version.
    plan: PhasorSolve<P>,
    /// Independently evaluated current copies and both overlapping radius channels.
    payload: Rc<super::numeric::Values<P>>,
    /// Existing Real inverse, residual and common-operator perturbation proof.
    certificate: PhasorSolveCertificate<P>,
    /// Matching exact norm/reference transport, evaluated outward on the full reactance box.
    transfer: PhasorTransferBounds<P>,
    /// Reconstructed physical residual and matching current metric bounds.
    residual: PhasorTransferredResidual<P>,
    /// Original impedance and voltage assurance scopes.
    coverage: [Coverage; 2],
    /// Arithmetic theorem revision.
    revision: u32,
    /// Before lowering, before numerical access and after complete solve, on one caller ledger.
    usage: [BudgetUsage; 3],
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
        Ok(&self.payload.centers)
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
        Ok(&self.payload.totals)
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
        Ok(&self.payload.errors)
    }
    /// Historical Real solve evidence; further execution still requires live dependencies.
    #[must_use]
    pub const fn certificate(&self) -> &PhasorSolveCertificate<P> {
        &self.certificate
    }
    /// Enclosed norm factors with the exact convention retained by `Self::plan`.
    #[must_use]
    pub const fn transfer(&self) -> &PhasorTransferBounds<P> {
        &self.transfer
    }
    /// Reconstructed physical residual and its current component bound in the named metrics.
    #[must_use]
    pub const fn transferred_residual(&self) -> &PhasorTransferredResidual<P> {
        &self.residual
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
        let before = budget.usage();
        let bytes = plan
            .retained_payload_bytes()
            .checked_add(4096)
            .ok_or(HeapError::ExtentOverflow)?;
        let (payload, certificate, transport, residual, coverage, lowered) = budget
            .with_retained_payload(bytes, AT, |budget| {
                let references = transfer::lower(plan, context, budget)?;
                let constants = constants::lower(plan, context, budget)?;
                let lowered = budget.usage();
                let (impedance, operator_scope) =
                    input::capture(self, &plan.operands()[0], budget)?;
                let (voltage, voltage_scope) = input::capture(self, &plan.operands()[1], budget)?;
                constants::check(&[&impedance, &voltage], &constants, context, budget)?;
                let reactance = numeric::domain(&impedance, context, budget)?;
                let transport =
                    transfer::evaluate(plan, references[0], reactance, context, budget)?;
                let (payload, certificate) = numeric::solve(&impedance, &voltage, context, budget)?;
                let residual = residual::evaluate(
                    plan,
                    &[impedance, voltage],
                    &payload.centers,
                    &references,
                    context,
                    budget,
                )?;
                budget.check(BudgetKind::OutputBytes, bytes, AT)?;
                Ok::<_, PhasorError>((
                    payload,
                    certificate,
                    transport,
                    residual,
                    [operator_scope, voltage_scope],
                    lowered,
                ))
            })?;
        Ok(PhasorSolveEvaluation {
            plan: plan.clone(),
            payload: Rc::new(payload),
            certificate,
            transfer: transport,
            residual,
            coverage,
            revision: context.revision(),
            usage: [before, lowered, budget.usage()],
        })
    }
}

#[cfg(test)]
mod tests;
