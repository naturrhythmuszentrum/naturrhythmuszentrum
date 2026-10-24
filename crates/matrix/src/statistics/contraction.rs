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
/// Enclose coefficient error and every arithmetic step of an ordinary statistical contraction.
pub(crate) fn multiply<P: Scalar + 'static>(
    left: &Matrix<P>,
    right: &Matrix<P>,
    context: &ArithmeticContext,
    work: &mut Work,
) -> Result<Matrix<P>, OperationError> {
    let extent = product_extent(left, right)?;
    let centers = match P::PRECISION {
        nrz_scalar::Precision::F32 => single(left, right)?,
        nrz_scalar::Precision::F64 => double(left, right)?,
    };
    let mut truth = buffer(extent.entries())?;
    truth.resize(extent.entries(), Enclosure::singleton(Finite::<P>::ZERO));
    let mut result = Matrix {
        extent,
        centers: centers.into(),
        truth: truth.into(),
    };
    verify(left, right, &mut result, context, work)?;
    Ok(result)
}
/// The solve's original source relation predeclares all operands, copies and truth endpoints.
pub(crate) fn multiply_planned<P: Scalar + 'static>(
    left: &Matrix<P>,
    right: &Matrix<P>,
    context: &ArithmeticContext,
    work: &mut Work,
    attempt: crate::solve::scratch::Attempt<'_>,
) -> Result<Matrix<P>, OperationError> {
    use crate::solve::scratch::Stage;
    let extent = product_extent(left, right)?;
    if extent
        != attempt
            .run
            .extent(Stage::ContractionCenters, Some(attempt.index))?
    {
        return Err(StatisticsError::Contract.into());
    }
    let centers = match P::PRECISION {
        nrz_scalar::Precision::F32 => planned_single(left, right, attempt)?,
        nrz_scalar::Precision::F64 => planned_double(left, right, attempt)?,
    };
    let mut truth = attempt.buffer(Stage::ContractionTruth)?;
    for _ in 0..extent.entries() {
        truth.push(Enclosure::singleton(Finite::<P>::ZERO))?;
    }
    let mut result = Matrix {
        extent,
        centers,
        truth,
    };
    verify(left, right, &mut result, context, work)?;
    if attempt.index != 1 {
        result.centers = result.centers.into_candidate()?;
        result.truth = result.truth.into_candidate()?;
    }
    Ok(result)
}
/// Both logical operands are checked before calling any backend allocator.
fn product_extent<P: Scalar>(
    left: &Matrix<P>,
    right: &Matrix<P>,
) -> Result<crate::Extent, OperationError> {
    if left.extent.columns() != right.extent.rows() {
        return Err(StatisticsError::Contract.into());
    }
    Ok(crate::Extent::new(
        left.extent.rows(),
        right.extent.columns(),
    )?)
}
/// Increasing-index interval arithmetic independently verifies every already owned candidate entry.
fn verify<P: Scalar>(
    left: &Matrix<P>,
    right: &Matrix<P>,
    result: &mut Matrix<P>,
    context: &ArithmeticContext,
    work: &mut Work,
) -> Result<(), OperationError> {
    for index in 0..result.extent.entries() {
        let row = index % result.extent.rows();
        let column = index / result.extent.rows();
        let mut truth = Enclosure::singleton(Finite::ZERO);
        for term in 0..left.extent.columns() {
            work.charge()?;
            truth = truth.add(
                left.entry(row, term)?
                    .mul(right.entry(term, column)?, context)?,
                context,
            )?;
        }
        result.set(
            row,
            column,
            *result.centers.get(index).ok_or(StatisticsError::Contract)?,
            truth,
        )?;
    }
    Ok(())
}
/// Concrete sealed adapters expose only logical backend shapes, never hidden padding or capacity.
macro_rules! backend {
    ($name:ident, $planned:ident, $pack:ident, $format:ty) => {
        /// Pack finite candidates into one safe logical backend rectangle.
        fn $pack<P: Scalar + 'static>(
            input: &Matrix<P>,
        ) -> Result<faer::Mat<$format>, OperationError> {
            let mut result =
                faer::Mat::<$format>::zeros(input.extent.rows(), input.extent.columns());
            for (index, value) in input.centers.iter().enumerate() {
                result[(index % input.extent.rows(), index / input.extent.rows())] =
                    crate::conversion::same_format(*value)?;
            }
            Ok(result)
        }
        /// Ordinary statistical routes retain one scalar candidate copy and no redundant zero copy.
        fn $name<P: Scalar + 'static>(
            left: &Matrix<P>,
            right: &Matrix<P>,
        ) -> Result<Vec<P>, OperationError> {
            let first = $pack(left)?;
            let second = $pack(right)?;
            let candidate = &first * &second;
            let extent = product_extent(left, right)?;
            let mut result = buffer(extent.entries())?;
            for index in 0..extent.entries() {
                result.push(crate::conversion::same_format(
                    candidate[(index % extent.rows(), index / extent.rows())],
                )?);
            }
            Ok(result)
        }
        /// Each covered backend owner retains its permit until the scalar copy is complete.
        fn $planned<P: Scalar + 'static>(
            left: &Matrix<P>,
            right: &Matrix<P>,
            attempt: crate::solve::scratch::Attempt<'_>,
        ) -> Result<crate::private::Buffer<P>, OperationError> {
            use crate::solve::scratch::Stage;
            if attempt
                .run
                .extent(Stage::ContractionLeft, Some(attempt.index))?
                != left.extent
                || attempt
                    .run
                    .extent(Stage::ContractionRight, Some(attempt.index))?
                    != right.extent
            {
                return Err(StatisticsError::Contract.into());
            }
            let first = attempt
                .run
                .backend(Stage::ContractionLeft, attempt.index, || $pack(left))?;
            let second = attempt
                .run
                .backend(Stage::ContractionRight, attempt.index, || $pack(right))?;
            let candidate = attempt
                .run
                .backend(Stage::ContractionResult, attempt.index, || {
                    Ok(first.get() * second.get())
                })?;
            let extent = product_extent(left, right)?;
            let mut result = attempt.buffer(Stage::ContractionCenters)?;
            for index in 0..extent.entries() {
                result.push(crate::conversion::same_format(
                    candidate.get()[(index % extent.rows(), index / extent.rows())],
                )?)?;
            }
            Ok(result)
        }
    };
}
backend!(single, planned_single, pack_single, f32);
backend!(double, planned_double, pack_double, f64);

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
