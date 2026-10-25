// SPDX-License-Identifier: AGPL-3.0-only
//! Signed root covariance is a view of one first-order variance, never a support bound.

use super::{Heap, LcSpectralEvaluation, SpectralError as Error};
use crate::MomentAssurance;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

impl<P: Scalar + 'static> LcSpectralEvaluation<P> {
    /// Statistical scope, absent unless an original derivative and complete joint model were supplied.
    #[must_use]
    pub fn moment_assurance(&self) -> Option<MomentAssurance> {
        if self
            .plan
            .extension()
            .is_some_and(|extension| extension.moments.is_some())
        {
            Some(MomentAssurance::LinearizedMoments)
        } else {
            None
        }
    }
    /// Both signed physical root partials, ordered `(Re+, Im+, Re-, Im-)` by original L/C.
    /// # Errors
    /// Refuses stale original values, companions, model targets or arithmetic revisions.
    pub fn root_partials(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<Option<[[Enclosure<P>; 2]; 4]>, Error> {
        self.validate(heap, context)?;
        Ok(self
            .plan
            .extension()
            .and_then(|extension| extension.moments)
            .map(|moments| {
                let zero = [Enclosure::singleton(Finite::ZERO); 2];
                [
                    zero,
                    moments.partials,
                    zero,
                    moments.partials.map(Enclosure::negated),
                ]
            }))
    }
    /// Complete covariance in physical rate coordinates, ordered `(Re+, Im+, Re-, Im-)`.
    ///
    /// Each enclosure bounds a covariance parameter of the local linear model. It supplies
    /// neither nonlinear means nor deterministic support for the physical root excursions.
    /// Both diagonal root blocks and both signed cross blocks share one variance owner.
    /// # Errors
    /// Refuses stale original values, companions, model targets or arithmetic revisions.
    pub fn root_covariance(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<Option<[[Enclosure<P>; 4]; 4]>, Error> {
        self.validate(heap, context)?;
        Ok(self
            .plan
            .extension()
            .and_then(|extension| extension.moments)
            .map(|moments| {
                let mut result = [[Enclosure::singleton(Finite::ZERO); 4]; 4];
                result[1][1] = moments.variance;
                result[3][3] = moments.variance;
                result[1][3] = moments.variance.negated();
                result[3][1] = moments.variance.negated();
                result
            }))
    }
}
