// SPDX-License-Identifier: AGPL-3.0-only
//! The origin traversal reuses the observed kernel bodies after whole-traversal prepayment.
use super::observation::Attempt;
use super::{LcFrequencyTarget, LcInputProduct, LcProductError, LcRequestedState, Observation};
use nrz_scalar::Scalar;

/// The consuming helper supplies attribution even when its input radius is malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan) enum LcKernelStage {
    /// Original L/C multiplication.
    Product,
    /// Positive square-root helper.
    Root,
    /// Final angular-frequency reciprocal helper.
    Reciprocal,
}
impl<P: Scalar + 'static, const N: usize> LcInputProduct<'_, P, N> {
    /// Whole-demand readiness checks the same qualified masks and addresses as each load.
    pub(in crate::loan) fn preflight(&self, cell: usize) -> Result<(), crate::LoanError> {
        self.cells.preflight(cell)
    }
    /// The private permit admits all selected cells and their cleanup before this invocation.
    pub(in crate::loan) fn omega_prepaid(
        &self,
        cell: usize,
        account: u64,
        work: u64,
        _permit: &crate::loan::lc_origin::pending::execution::Traversal,
    ) -> Result<LcRequestedState<P>, (LcKernelStage, LcProductError)> {
        let mut stage = LcKernelStage::Product;
        let attempt = Attempt {
            cell,
            demand: "omega",
            consumer: "origin-traversal",
            account,
            work,
        };
        self.observe_prepaid(
            &attempt,
            || {
                let result = self.omega_body(cell, &mut stage);
                (result, work)
            },
            |state| *state,
        )
        .map_err(|error| (stage, error))
    }
    /// Each helper retains its original complete total and operation-local numerical error.
    fn omega_body(
        &self,
        cell: usize,
        stage: &mut LcKernelStage,
    ) -> Result<LcRequestedState<P>, LcProductError> {
        let (observation, blocked) = self.observe_product(cell)?;
        if let Some(blocked) = blocked {
            return Ok(blocked);
        }
        let Observation::Product(product) = observation else {
            return Err(crate::LoanError::UnsupportedContract.into());
        };
        *stage = LcKernelStage::Root;
        let tau = self.root(&product)?;
        if tau[0] <= tau[1] {
            return Ok(LcRequestedState::Undecided(LcFrequencyTarget::Root));
        }
        *stage = LcKernelStage::Reciprocal;
        let rate = self.reciprocal(tau, cell)?;
        Ok(LcRequestedState::Ready {
            tau,
            rate: Some(rate),
            values: super::LcRequestedValues {
                omega: Some(rate),
                frequency: None,
                period: None,
            },
        })
    }
}
