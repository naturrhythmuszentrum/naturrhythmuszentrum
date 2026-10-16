// SPDX-License-Identifier: AGPL-3.0-only
//! Faer candidates and independent increasing-index interval contractions.

use super::{StatisticsError, matrix::Matrix};
use crate::{OperationError, binding::buffer};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Charge before every independent scalar contraction term.
#[derive(Debug)]
pub(crate) struct Work {
    /// Unspent checked terms.
    remaining: usize,
}
impl Work {
    /// Capture the preflight allowance.
    pub(crate) const fn new(remaining: usize) -> Self {
        Self { remaining }
    }
    /// Refuse before exceeding the budget; exhausted counters never wrap.
    pub(crate) fn charge(&mut self) -> Result<(), OperationError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(StatisticsError::ResourceLimit)?;
        Ok(())
    }
}
/// Enclose coefficient error and every arithmetic step of the contraction.
pub(crate) fn multiply<P: Scalar + 'static>(
    left: &Matrix<P>,
    right: &Matrix<P>,
    context: &ArithmeticContext,
    work: &mut Work,
) -> Result<Matrix<P>, OperationError> {
    if left.extent.columns() != right.extent.rows() {
        return Err(StatisticsError::Contract.into());
    }
    let centers = match P::PRECISION {
        nrz_scalar::Precision::F32 => single(left, right)?,
        nrz_scalar::Precision::F64 => double(left, right)?,
    };
    let mut result = Matrix::zeros(left.extent.rows(), right.extent.columns())?;
    for column in 0..result.extent.columns() {
        for row in 0..result.extent.rows() {
            let mut truth = Enclosure::singleton(Finite::ZERO);
            for index in 0..left.extent.columns() {
                work.charge()?;
                truth = truth.add(
                    left.entry(row, index)?
                        .mul(right.entry(index, column)?, context)?,
                    context,
                )?;
            }
            let index = result.extent.index(row, column)?;
            result.set(
                row,
                column,
                *centers.get(index).ok_or(StatisticsError::Contract)?,
                truth,
            )?;
        }
    }
    Ok(result)
}
/// Concrete sealed adapters make no assumption about the backend reduction order.
macro_rules! backend {
    ($name:ident, $format:ty) => {
        /// Generate only private candidates through safe backend accessors.
        fn $name<P: Scalar + 'static>(
            left: &Matrix<P>,
            right: &Matrix<P>,
        ) -> Result<Vec<P>, OperationError> {
            let mut first = faer::Mat::<$format>::zeros(left.extent.rows(), left.extent.columns());
            let mut second =
                faer::Mat::<$format>::zeros(right.extent.rows(), right.extent.columns());
            for (index, value) in left.centers.iter().enumerate() {
                first[(index % left.extent.rows(), index / left.extent.rows())] =
                    crate::conversion::same_format(*value)?;
            }
            for (index, value) in right.centers.iter().enumerate() {
                second[(index % right.extent.rows(), index / right.extent.rows())] =
                    crate::conversion::same_format(*value)?;
            }
            let candidate = &first * &second;
            let mut result = buffer(
                left.extent
                    .rows()
                    .checked_mul(right.extent.columns())
                    .ok_or(StatisticsError::ResourceLimit)?,
            )?;
            for column in 0..right.extent.columns() {
                for row in 0..left.extent.rows() {
                    result.push(crate::conversion::same_format(candidate[(row, column)])?);
                }
            }
            Ok(result)
        }
    };
}
backend!(single, f32);
backend!(double, f64);

#[cfg(test)]
mod tests {
    use super::Work;
    use crate::{OperationError, StatisticsError};
    /// A covariance contraction cannot execute an unbudgeted term.
    /// ´test:unit:statistical-contraction-work-never-wraps´
    #[test]
    fn statistical_contraction_work_never_wraps() {
        let mut work = Work::new(1);
        assert!(work.charge().is_ok());
        assert_eq!(work.remaining, 0);
        assert!(matches!(
            work.charge(),
            Err(OperationError::Statistics(StatisticsError::ResourceLimit))
        ));
        assert_eq!(work.remaining, 0);
    }
}
