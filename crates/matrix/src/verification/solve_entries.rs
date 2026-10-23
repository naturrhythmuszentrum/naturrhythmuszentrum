// SPDX-License-Identifier: AGPL-3.0-only
//! The Real verifier reads admitted coordinates without owning their physical interpretation.

use crate::{Extent, NormalizedView, SolveError, solve::RightHandSide};
use nrz_scalar::Scalar;

/// Private read-only numerical interface shared by heap views and checked phasor blocks.
pub(crate) trait SolveMatrix<P: Scalar> {
    /// Complete finite matrix extent in logical column order.
    fn extent(&self) -> Extent;
    /// One already admitted represented coordinate.
    fn get(&self, row: usize, column: usize) -> Result<P, SolveError>;
    /// Its optional total or numerical radius, selected by the invoking theorem.
    fn bound(&self, row: usize, column: usize) -> Result<Option<P>, SolveError>;
}

impl<P: Scalar + 'static> SolveMatrix<P> for NormalizedView<'_, P> {
    fn extent(&self) -> Extent {
        self.extent()
    }
    fn get(&self, row: usize, column: usize) -> Result<P, SolveError> {
        Ok(self.get(row, column)?)
    }
    fn bound(&self, row: usize, column: usize) -> Result<Option<P>, SolveError> {
        Ok(self.bound(row, column)?)
    }
}

impl<P: Scalar + 'static> SolveMatrix<P> for RightHandSide<'_, '_, P> {
    fn extent(&self) -> Extent {
        self.extent()
    }
    fn get(&self, row: usize, column: usize) -> Result<P, SolveError> {
        self.get(row, column)
    }
    fn bound(&self, row: usize, column: usize) -> Result<Option<P>, SolveError> {
        Ok(Some(self.bound(row, column)?))
    }
}
