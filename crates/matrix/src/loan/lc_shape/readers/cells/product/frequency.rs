// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original boxes authorize only demanded strictly positive local continuations.

mod observation;
mod prelude;
mod requested;
mod traversal;
use super::{LcInputCell, LcInputProduct, LcProductCell, LcProductError, work};
use crate::{Elementwise, KernelError};
use nrz_scalar::Scalar;
pub use prelude::LcFrequencyPrelude;
pub use requested::{LcQuantityDemand, LcRequestedCell, LcRequestedState, LcRequestedValues};
pub(in crate::loan) use traversal::LcKernelStage;

/// Original target whose sign obstructed the local frequency continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcFrequencyTarget {
    /// Explicit original inductance role.
    Inductance,
    /// Explicit original capacitance role.
    Capacitance,
    /// Product of those original roles.
    Product,
    /// Enclosed root whose strict positive separation was lost numerically.
    Root,
}
/// Original observation before arithmetic or together with its actual product.
#[derive(Debug)]
enum Observation<'a, P: Scalar, const N: usize> {
    /// An original-element sign outcome requires no product.
    Inputs(LcInputCell<'a, P, N>),
    /// Positively separated original elements authorized this product.
    Product(LcProductCell<'a, P, N>),
}
impl<'a, P: Scalar + 'static, const N: usize> LcInputProduct<'a, P, N> {
    /// Share complete observation and L/C/P classification without calculating unused successors.
    fn observe_product(
        &self,
        cell: usize,
    ) -> Result<(Observation<'a, P, N>, Option<LcRequestedState<P>>), LcProductError> {
        let inputs = self.cells.observe(cell)?;
        let values = self
            .positions
            .map(|position| inputs.readings()[position].values());
        if let Some(state) = classify(
            [
                LcFrequencyTarget::Inductance,
                LcFrequencyTarget::Capacitance,
            ],
            values,
        ) {
            return Ok((Observation::Inputs(inputs), Some(state)));
        }
        let product = self.evaluate(inputs)?;
        let state = classify([LcFrequencyTarget::Product], [product.values()]);
        Ok((Observation::Product(product), state))
    }
    /// One ordinary root kernel serves every requested quantity subset.
    fn root(&self, product: &LcProductCell<'_, P, N>) -> Result<[P; 3], LcProductError> {
        let values = product.values();
        let tau = crate::kernels::nonlinear::evaluate_selected(
            values[0],
            nrz_accuracy::Nonnegative::new(values[1]).map_err(KernelError::from)?,
            Elementwise::SquareRoot,
            product.inputs().cell(),
            &self.context,
        )?;
        Ok([tau.center, tau.total, tau.error])
    }
    /// Reciprocal is entered only when omega or cyclic frequency needs it.
    fn reciprocal(&self, tau: [P; 3], cell: usize) -> Result<[P; 3], LcProductError> {
        let value = crate::kernels::nonlinear::evaluate_selected(
            tau[0],
            nrz_accuracy::Nonnegative::new(tau[1]).map_err(KernelError::from)?,
            Elementwise::Reciprocal,
            cell,
            &self.context,
        )?;
        Ok([value.center, value.total, value.error])
    }
    /// Ordinary Product propagates actual input and constant totals with local rounding error.
    fn scale(&self, input: [P; 3], factor: [P; 3], cell: usize) -> Result<[P; 3], LcProductError> {
        let value = crate::kernels::binary::evaluate_selected(
            [input[0], factor[0]],
            [input[1], factor[1]],
            crate::BinaryElementwise::Product,
            cell,
            &self.context,
        )?;
        Ok([value.center, value.total, value.error])
    }
}
/// Strict dyadic comparisons prove sign without rounded endpoint construction or nominal assumptions.
fn classify<P: Scalar, const N: usize>(
    targets: [LcFrequencyTarget; N],
    values: [[P; 3]; N],
) -> Option<LcRequestedState<P>> {
    let mut undecided = None;
    for (target, [center, total, _]) in targets.into_iter().zip(values) {
        if center < -total {
            return Some(LcRequestedState::Invalid(target));
        }
        let unresolved = center <= total;
        let first = undecided.is_none();
        if unresolved && first {
            undecided = Some(LcRequestedState::Undecided(target));
        }
    }
    undecided
}
