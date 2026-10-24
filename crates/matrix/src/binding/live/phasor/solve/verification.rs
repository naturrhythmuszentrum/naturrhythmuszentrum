// SPDX-License-Identifier: AGPL-3.0-only
//! Shared theorem entry points fill only native buffers whose permits precede allocation.

use super::{
    PhasorAllocationStage as Stage, PhasorError,
    inventory::{Owner, Run},
};
use crate::solve_verification::{self as verify, SolveMatrix, SolveWork};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Borrowed original inventory and arithmetic environment, with no numerical copies.
pub(super) struct Verifier<'a, P: Scalar> {
    /// Exact allocation meanings for this native solve.
    pub(super) run: &'a Run<P>,
    /// Admitted outward arithmetic environment.
    pub(super) context: &'a ArithmeticContext,
}
impl<P: Scalar + 'static> Verifier<'_, P> {
    /// One inverse-defect vector and the independent inverse proof owner.
    pub(super) fn inverse(
        &self,
        operator: &impl SolveMatrix<P>,
        candidate: &[P],
        work: &mut SolveWork,
    ) -> Result<Owner<P, verify::InverseWitness<P>>, PhasorError> {
        self.run.inline(Stage::InverseProof, || {
            let mut defects = self.run.zeros(Stage::InverseDefects)?;
            Ok(verify::inverse_with_defects(
                operator,
                candidate,
                &mut defects,
                work,
                self.context,
            )?)
        })
    }
    /// One represented-residual vector and the independent center proof owner.
    pub(super) fn center(
        &self,
        operator: &impl SolveMatrix<P>,
        rhs: &impl SolveMatrix<P>,
        candidate: &[P],
        witness: &verify::InverseWitness<P>,
        work: &mut SolveWork,
    ) -> Result<Owner<P, verify::CenterCertificate<P>>, PhasorError> {
        self.run.inline(Stage::CenterProof, || {
            let mut residuals = self.run.zeros(Stage::CenterResiduals)?;
            Ok(verify::center_with_residuals(
                operator,
                rhs,
                candidate,
                witness,
                &mut residuals,
                work,
                self.context,
            )?)
        })
    }
    /// Each selected overlapping radius channel has two vectors and its own proof fields.
    pub(super) fn family(
        &self,
        operator: &impl SolveMatrix<P>,
        rhs: &impl SolveMatrix<P>,
        witness: &verify::InverseWitness<P>,
        center: &verify::CenterCertificate<P>,
        numerical: bool,
    ) -> Result<Owner<P, verify::FamilyCertificate<P>>, PhasorError> {
        let (proof, first, second) = if numerical {
            (
                Stage::NumericalProof,
                Stage::OperatorErrors,
                Stage::VoltageErrors,
            )
        } else {
            (
                Stage::TotalProof,
                Stage::OperatorTotals,
                Stage::VoltageTotals,
            )
        };
        self.run.inline(proof, || {
            let mut operator_bounds = self.run.zeros(first)?;
            let mut rhs_bounds = self.run.zeros(second)?;
            Ok(verify::family_with_bounds(
                operator,
                rhs,
                witness,
                center,
                [&mut operator_bounds, &mut rhs_bounds],
                self.context,
            )?)
        })
    }
}
