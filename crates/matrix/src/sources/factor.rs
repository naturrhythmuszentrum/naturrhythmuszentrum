// SPDX-License-Identifier: AGPL-3.0-only
//! A rectangular represented factor defines a PSD matrix without a rank assumption.

use super::{JointError, JointLimits};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Exact mathematical covariance `L diag(d) Lᵀ` in declared source coordinates.
///
/// The represented factor defines the model; finite nonnegative weights prove PSD.
/// Rectangular, empty-column and rank-deficient factors are admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFactor<P: Scalar> {
    /// Number of source coordinates.
    rows: usize,
    /// Column-ordered finite represented coefficients.
    coefficients: Vec<P>,
    /// Nonnegative finite represented diagonal weights.
    weights: Vec<P>,
}
impl<P: Scalar> SourceFactor<P> {
    /// Admit a finite rectangular factor under explicit storage limits.
    ///
    /// # Errors
    /// Refuses invalid lengths, nonfinite coefficients, negative weights and limits.
    pub fn new(
        rows: usize,
        coefficients: &[P],
        weights: &[P],
        limits: JointLimits,
    ) -> Result<Self, JointError> {
        let entries = limits.matrix(rows, weights.len())?;
        let work = entries
            .checked_add(weights.len())
            .ok_or(JointError::ResourceLimit)?;
        limits.check_work(work)?;
        if weights.len() > limits.entries {
            return Err(JointError::ResourceLimit);
        }
        if coefficients.len() != entries {
            return Err(JointError::LengthMismatch);
        }
        for &value in coefficients {
            Finite::new(value)?;
        }
        if weights
            .iter()
            .any(|value| !value.is_finite() || *value < P::ZERO)
        {
            return Err(JointError::InvalidWeight);
        }
        Ok(Self {
            rows,
            coefficients: coefficients.to_vec(),
            weights: weights.to_vec(),
        })
    }
    /// Source coordinate count.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }
    /// Factor column count; no full-rank claim follows.
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.weights.len()
    }
    /// Exact represented coefficients in column order.
    #[must_use]
    pub fn coefficients(&self) -> &[P] {
        &self.coefficients
    }
    /// Exact represented nonnegative weights.
    #[must_use]
    pub fn weights(&self) -> &[P] {
        &self.weights
    }
    /// Enclose one exact covariance parameter in increasing factor-column order.
    ///
    /// # Errors
    /// Refuses indices, work exhaustion or nonfinite outward arithmetic.
    pub fn entry(
        &self,
        row: usize,
        column: usize,
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Enclosure<P>, JointError> {
        if row >= self.rows || column >= self.rows {
            return Err(JointError::LengthMismatch);
        }
        limits.check_work(self.weights.len())?;
        let mut total = Enclosure::singleton(Finite::ZERO);
        for (index, &weight) in self.weights.iter().enumerate() {
            let start = index
                .checked_mul(self.rows)
                .ok_or(JointError::ResourceLimit)?;
            let left = *self
                .coefficients
                .get(start + row)
                .ok_or(JointError::LengthMismatch)?;
            let right = *self
                .coefficients
                .get(start + column)
                .ok_or(JointError::LengthMismatch)?;
            let product = Enclosure::singleton(Finite::new(left)?)
                .mul(Enclosure::singleton(Finite::new(weight)?), context)?
                .mul(Enclosure::singleton(Finite::new(right)?), context)?;
            total = total.add(product, context)?;
        }
        Ok(total)
    }
    /// Structural zero proof avoids numerical cancellation assumptions.
    pub(crate) fn zero(&self, row: usize, column: usize) -> bool {
        self.weights.iter().enumerate().all(|(index, weight)| {
            *weight == P::ZERO
                || self.coefficients[index * self.rows + row] == P::ZERO
                || self.coefficients[index * self.rows + column] == P::ZERO
        })
    }
}
