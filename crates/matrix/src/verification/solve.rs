// SPDX-License-Identifier: AGPL-3.0-only
//! Independent heap-view residual and inverse-defect enclosures, never LU reconstruction claims.

use crate::binding::buffer;
use crate::{Extent, NormalizedView, SolveError};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Private inverse proof in the saved, unequilibrated normalized coordinates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InverseWitness<P: Scalar> {
    /// Frobenius upper bound on `I - Z A`.
    pub(crate) defect: P,
    /// Frobenius upper bound on the approximate inverse.
    pub(crate) magnitude: P,
    /// Positive lower bound on `1 - defect`.
    pub(crate) margin: P,
    /// Spectral inverse-norm upper bound `magnitude / margin`.
    pub(crate) inverse: P,
}

/// Independently enclosed represented-center discrepancy and candidate magnitude.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CenterCertificate<P: Scalar> {
    /// Frobenius upper bound on the exact represented residual.
    pub(crate) residual: P,
    /// Frobenius upper bound on the complete multiple-column candidate.
    pub(crate) magnitude: P,
    /// Numerical-only forward bound, overlapping every later total.
    pub(crate) radius: P,
}

/// A checked counter for each actual increasing-index contraction term.
#[derive(Debug)]
pub(crate) struct SolveWork {
    /// Unspent contraction terms; subtraction never wraps.
    remaining: usize,
}

impl SolveWork {
    /// Start a finite verifier allowance computed before backend entry.
    pub(crate) const fn new(terms: usize) -> Self {
        Self { remaining: terms }
    }

    /// Refuse before reading or evaluating an additional contraction term.
    fn charge(&mut self) -> Result<(), SolveError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(SolveError::ResourceLimit)?;
        Ok(())
    }
}

/// Certify a supplied inverse candidate without consulting any backend factors.
pub(crate) fn inverse<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    candidate: &[P],
    work: &mut SolveWork,
    context: &ArithmeticContext,
) -> Result<InverseWitness<P>, SolveError> {
    let extent = operator.extent();
    if extent.rows() == 0
        || (extent.rows(), candidate.len()) != (extent.columns(), extent.entries())
    {
        return Err(SolveError::Contract);
    }
    let mut defects = buffer(candidate.len())?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let dot = inverse_dot(operator, candidate, row, column, work, context)?;
            let identity = if row == column { P::ONE } else { P::ZERO };
            defects.push(point(identity)?.sub(dot, context)?.abs().upper().get());
        }
    }
    let defect = nrz_scalar::norm_upper(&defects, context)?.get();
    if defect >= P::ONE {
        return Err(SolveError::CannotCertifyInverse);
    }
    let margin = point(P::ONE)?.sub(point(defect)?, context)?.lower().get();
    if margin <= P::ZERO {
        return Err(SolveError::CannotCertifyInverse);
    }
    let magnitude = nrz_scalar::norm_upper(candidate, context)?.get();
    let inverse = point(magnitude)?
        .div(point(margin)?, context)?
        .upper()
        .get();
    Ok(InverseWitness {
        defect,
        magnitude,
        margin,
        inverse,
    })
}

/// Enclose one row of Z times one column of A in increasing contraction order.
fn inverse_dot<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    candidate: &[P],
    row: usize,
    column: usize,
    work: &mut SolveWork,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, SolveError> {
    let mut total = point(P::ZERO)?;
    for index in 0..operator.extent().rows() {
        work.charge()?;
        let coefficient = candidate
            .get(operator.extent().index(row, index)?)
            .copied()
            .ok_or(SolveError::Contract)?;
        let product = point(coefficient)?.mul(point(operator.get(index, column)?)?, context)?;
        total = total.add(product, context)?;
    }
    Ok(total)
}

/// Enclose the residual over saved heap views, separately from inverse identity columns.
pub(crate) fn center<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    rhs: &NormalizedView<'_, P>,
    candidate: &[P],
    witness: &InverseWitness<P>,
    work: &mut SolveWork,
    context: &ArithmeticContext,
) -> Result<CenterCertificate<P>, SolveError> {
    let extent = rhs.extent();
    if extent.rows() != operator.extent().rows()
        || extent.columns() == 0
        || candidate.len() != extent.entries()
    {
        return Err(SolveError::Contract);
    }
    let mut residuals = buffer(candidate.len())?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let dot = solution_dot(operator, candidate, extent, row, column, work, context)?;
            residuals.push(
                point(rhs.get(row, column)?)?
                    .sub(dot, context)?
                    .abs()
                    .upper()
                    .get(),
            );
        }
    }
    let residual = nrz_scalar::norm_upper(&residuals, context)?.get();
    let magnitude = nrz_scalar::norm_upper(candidate, context)?.get();
    let radius = point(witness.inverse)?
        .mul(point(residual)?, context)?
        .upper()
        .get();
    Ok(CenterCertificate {
        residual,
        magnitude,
        radius,
    })
}

/// Independent dot products use checked candidate indices and never faer reductions.
fn solution_dot<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    candidate: &[P],
    extent: Extent,
    row: usize,
    column: usize,
    work: &mut SolveWork,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, SolveError> {
    let mut total = point(P::ZERO)?;
    for index in 0..operator.extent().columns() {
        work.charge()?;
        let coordinate = candidate
            .get(extent.index(index, column)?)
            .copied()
            .ok_or(SolveError::Contract)?;
        let product = point(operator.get(row, index)?)?.mul(point(coordinate)?, context)?;
        total = total.add(product, context)?;
    }
    Ok(total)
}

/// Finite singletons are inputs to verification, never an assertion of exact floating arithmetic.
pub(crate) fn point<P: Scalar>(value: P) -> Result<Enclosure<P>, SolveError> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}

#[cfg(test)]
mod tests {
    use super::SolveWork;
    use crate::SolveError;

    /// Exhaustion refuses the extra term before decrementing zero.
    /// ´test:unit:solve-work-counter-never-wraps´
    #[test]
    fn solve_work_counter_never_wraps() {
        let mut work = SolveWork::new(1);
        assert!(work.charge().is_ok());
        assert_eq!(work.remaining, 0);
        assert!(matches!(work.charge(), Err(SolveError::ResourceLimit)));
        assert_eq!(work.remaining, 0);
    }
}
