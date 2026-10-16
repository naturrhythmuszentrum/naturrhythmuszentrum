// SPDX-License-Identifier: AGPL-3.0-only
//! Independent increasing-index interval contractions; no backend reduction order is assumed.

use crate::{KernelError, NormalizedView, OperationError};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Exact-center enclosure and three outward deterministic product contributions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductBound<P: Scalar> {
    /// Encloses the exact represented-center dot product.
    pub(crate) nominal: Enclosure<P>,
    /// `abs(A) R_B`, `R_A abs(B)` and `R_A R_B`, in that order.
    pub(crate) contributions: [Nonnegative<P>; 3],
}

/// Count actual contraction terms independently of dimensions used during preflight.
#[derive(Debug)]
pub(crate) struct ContractionWork {
    /// Unspent ordered contraction terms.
    remaining: usize,
}

impl ContractionWork {
    /// Begin one checked finite contraction allowance.
    pub(crate) const fn new(terms: usize) -> Self {
        Self { remaining: terms }
    }
    /// Refuse before the next term when the allowance is exhausted.
    fn charge(&mut self) -> Result<(), OperationError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(OperationError::ResourceLimit)?;
        Ok(())
    }
}

/// Enclose one exact real dot and all perturbation terms in increasing contraction order.
pub(crate) fn product<P: Scalar + 'static>(
    left: &NormalizedView<'_, P>,
    right: &NormalizedView<'_, P>,
    row: usize,
    column: usize,
    work: &mut ContractionWork,
    context: &ArithmeticContext,
) -> Result<ProductBound<P>, OperationError> {
    if left.extent().columns() != right.extent().rows() {
        return Err(KernelError::Contract.into());
    }
    let zero = Nonnegative::new(P::ZERO).map_err(KernelError::from)?;
    let mut result = ProductBound {
        nominal: singleton(P::ZERO)?,
        contributions: [zero; 3],
    };
    for index in 0..left.extent().columns() {
        work.charge()?;
        let first = left.get(row, index)?;
        let second = right.get(index, column)?;
        result.nominal = result
            .nominal
            .add(
                singleton(first)?
                    .mul(singleton(second)?, context)
                    .map_err(KernelError::from)?,
                context,
            )
            .map_err(KernelError::from)?;
        let left_radius = Nonnegative::new(left.bound(row, index)?.unwrap_or(P::ZERO))
            .map_err(KernelError::from)?;
        let right_radius = Nonnegative::new(right.bound(index, column)?.unwrap_or(P::ZERO))
            .map_err(KernelError::from)?;
        let terms = [
            Nonnegative::new(first.abs())
                .map_err(KernelError::from)?
                .scale(right_radius, context)
                .map_err(KernelError::from)?,
            left_radius
                .scale(
                    Nonnegative::new(second.abs()).map_err(KernelError::from)?,
                    context,
                )
                .map_err(KernelError::from)?,
            left_radius
                .scale(right_radius, context)
                .map_err(KernelError::from)?,
        ];
        for (total, term) in result.contributions.iter_mut().zip(terms) {
            *total = total.add(term, context).map_err(KernelError::from)?;
        }
    }
    Ok(result)
}

/// Admit a finite represented number without using the candidate backend.
fn singleton<P: Scalar>(value: P) -> Result<Enclosure<P>, OperationError> {
    Ok(Enclosure::singleton(
        Finite::new(value).map_err(KernelError::from)?,
    ))
}

/// Work accounting is private and cannot be bypassed by a dimension metadata claim.
#[cfg(test)]
mod tests {
    use super::ContractionWork;
    use crate::OperationError;

    /// Exhaustion leaves zero remaining terms and never wraps the counter.
    /// ´test:unit:contraction-work-counter-refuses-before-an-extra-term´
    #[test]
    fn contraction_work_counter_refuses_before_an_extra_term() {
        let mut work = ContractionWork::new(1);
        assert!(work.charge().is_ok());
        assert_eq!(work.remaining, 0);
        assert!(matches!(work.charge(), Err(OperationError::ResourceLimit)));
        assert_eq!(work.remaining, 0);
    }
}
