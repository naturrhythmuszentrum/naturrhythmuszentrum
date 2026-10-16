// SPDX-License-Identifier: AGPL-3.0-only
//! Finite parameter centers and independent interval truth in logical column order.

use super::StatisticsError;
use crate::binding::buffer;
use crate::{Extent, OperationError};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Temporary enclosed matrix; parameter errors never describe physical input support.
#[derive(Clone, Debug)]
pub(crate) struct Matrix<P: Scalar> {
    /// Checked logical rectangle.
    pub(crate) extent: Extent,
    /// Candidate entries independent of interval verification.
    pub(crate) centers: Vec<P>,
    /// Mathematical parameter enclosures.
    pub(crate) truth: Vec<Enclosure<P>>,
}
impl<P: Scalar> Matrix<P> {
    /// Allocate exact zeros after the operation's work preflight.
    pub(crate) fn zeros(rows: usize, columns: usize) -> Result<Self, OperationError> {
        let extent = Extent::new(rows, columns)?;
        let mut centers = buffer(extent.entries())?;
        centers.resize(extent.entries(), P::ZERO);
        let mut truth = buffer(extent.entries())?;
        truth.resize(extent.entries(), Enclosure::singleton(Finite::ZERO));
        Ok(Self {
            extent,
            centers,
            truth,
        })
    }
    /// Install a finite candidate and its established enclosure.
    pub(crate) fn set(
        &mut self,
        row: usize,
        column: usize,
        center: P,
        truth: Enclosure<P>,
    ) -> Result<(), OperationError> {
        let index = self.extent.index(row, column)?;
        *self
            .centers
            .get_mut(index)
            .ok_or(StatisticsError::Contract)? = Finite::new(center)?.get();
        *self.truth.get_mut(index).ok_or(StatisticsError::Contract)? = truth;
        Ok(())
    }
    /// Read a parameter through checked logical indexing.
    pub(crate) fn entry(&self, row: usize, column: usize) -> Result<Enclosure<P>, OperationError> {
        Ok(*self
            .truth
            .get(self.extent.index(row, column)?)
            .ok_or(StatisticsError::Contract)?)
    }
    /// Bound each candidate's numerical displacement independently.
    pub(crate) fn errors(&self, context: &ArithmeticContext) -> Result<Vec<P>, OperationError> {
        if (self.centers.len(), self.truth.len()) != (self.extent.entries(), self.extent.entries())
        {
            return Err(StatisticsError::Contract.into());
        }
        let mut errors = buffer(self.centers.len())?;
        for (&center, &truth) in self.centers.iter().zip(&self.truth) {
            errors.push(
                truth
                    .displacement_from(Finite::new(center)?, context)?
                    .get(),
            );
        }
        Ok(errors)
    }
    /// Transpose both parameter candidates and their enclosures without arithmetic.
    pub(crate) fn transpose(&self) -> Result<Self, OperationError> {
        let mut result = Self::zeros(self.extent.columns(), self.extent.rows())?;
        for column in 0..self.extent.columns() {
            for row in 0..self.extent.rows() {
                let index = self.extent.index(row, column)?;
                result.set(
                    column,
                    row,
                    *self.centers.get(index).ok_or(StatisticsError::Contract)?,
                    self.entry(row, column)?,
                )?;
            }
        }
        Ok(result)
    }
}
/// Admit a represented number without assigning a random source.
pub(crate) fn singleton<P: Scalar>(value: P) -> Result<Enclosure<P>, OperationError> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}
/// Reconstruct a parameter enclosure from its own numerical error region.
pub(crate) fn enclosed<P: Scalar>(
    center: P,
    error: P,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, OperationError> {
    if error < P::ZERO {
        return Err(StatisticsError::Contract.into());
    }
    let value = singleton(center)?;
    let radius = singleton(error)?;
    Ok(Enclosure::new(
        value.sub(radius, context)?.lower().get(),
        value.add(radius, context)?.upper().get(),
    )?)
}
