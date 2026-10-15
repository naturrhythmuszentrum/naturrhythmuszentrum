// SPDX-License-Identifier: AGPL-3.0-only
//! Sufficient outward PSD certificates for a whole symmetric parameter family.

use super::{JointError, JointLimits};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Sufficient proof, never a test that establishes indefiniteness on failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsdCertificate {
    /// Every diagonal lower endpoint dominates the outward absolute off-diagonal sum.
    DiagonalDominance,
    /// Interval symmetric elimination proves strictly positive pivot lower endpoints.
    PositivePivots,
}
/// A finite symmetric covariance family admitted by a sufficient PSD certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenseCovariance<P: Scalar> {
    /// Square source order.
    order: usize,
    /// Finite displayed parameters in column order.
    centers: Vec<P>,
    /// The complete admitted symmetric parameter family.
    family: Vec<Enclosure<P>>,
    /// Outward error of each displayed covariance parameter, never a source bound.
    errors: Vec<P>,
    /// Sufficient whole-family proof.
    certificate: PsdCertificate,
}
impl<P: Scalar> DenseCovariance<P> {
    /// Admit exact represented symmetric entries.
    ///
    /// # Errors
    /// Refuses nonfinite/asymmetric data, invalid extents, limits or an unresolved PSD proof.
    pub fn new(
        order: usize,
        centers: &[P],
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Self, JointError> {
        let count = limits.matrix(order, order)?;
        if centers.len() != count {
            return Err(JointError::LengthMismatch);
        }
        let family = centers
            .iter()
            .map(|value| Finite::new(*value).map(Enclosure::singleton))
            .collect::<Result<Vec<_>, _>>()?;
        Self::with_enclosures(order, centers, &family, context, limits)
    }
    /// Admit every symmetric matrix in the supplied entrywise parameter enclosures.
    ///
    /// The represented symmetric centers must lie inside the symmetric family.
    /// A positive certificate for the centers alone is insufficient.
    ///
    /// # Errors
    /// Refuses invalid lengths, asymmetry, centers outside the family, resource limits,
    /// arithmetic overflow in error construction or failure of both sufficient PSD tests.
    pub fn with_enclosures(
        order: usize,
        centers: &[P],
        family: &[Enclosure<P>],
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Self, JointError> {
        let count = limits.matrix(order, order)?;
        let work = count
            .checked_mul(order)
            .and_then(|value| value.checked_add(count))
            .ok_or(JointError::ResourceLimit)?;
        limits.check_work(work)?;
        if centers.len() != count || family.len() != count {
            return Err(JointError::LengthMismatch);
        }
        validate_family(order, centers, family)?;
        let certificate = if dominant(order, family, context).unwrap_or(false) {
            PsdCertificate::DiagonalDominance
        } else if positive_pivots(order, family, context).unwrap_or(false) {
            PsdCertificate::PositivePivots
        } else {
            return Err(JointError::CannotValidateCovariance);
        };
        let errors = centers
            .iter()
            .zip(family)
            .map(|(center, interval)| {
                interval
                    .displacement_from(Finite::new(*center)?, context)
                    .map(Finite::get)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            order,
            centers: centers.to_vec(),
            family: family.to_vec(),
            errors,
            certificate,
        })
    }
    /// Square source order.
    #[must_use]
    pub const fn order(&self) -> usize {
        self.order
    }
    /// Exact displayed parameter centers.
    #[must_use]
    pub fn centers(&self) -> &[P] {
        &self.centers
    }
    /// Complete admitted family in column order.
    #[must_use]
    pub fn family(&self) -> &[Enclosure<P>] {
        &self.family
    }
    /// Outward parameter errors, not physical uncertainty bounds.
    #[must_use]
    pub fn errors(&self) -> &[P] {
        &self.errors
    }
    /// The sufficient whole-family proof used at admission.
    #[must_use]
    pub const fn certificate(&self) -> PsdCertificate {
        self.certificate
    }
    /// Read one admitted parameter enclosure.
    ///
    /// # Errors
    /// Refuses out-of-range source coordinates.
    pub fn entry(&self, row: usize, column: usize) -> Result<Enclosure<P>, JointError> {
        if row >= self.order || column >= self.order {
            return Err(JointError::LengthMismatch);
        }
        self.family
            .get(row + self.order * column)
            .copied()
            .ok_or(JointError::LengthMismatch)
    }
    /// A singleton zero is the exact sufficient zero-covariance witness.
    pub(crate) fn zero(&self, row: usize, column: usize) -> bool {
        self.entry(row, column)
            .is_ok_and(|entry| entry.lower() == Finite::ZERO && entry.upper() == Finite::ZERO)
    }
}
/// Exact represented symmetry also applies to the parameter-family endpoints.
fn validate_family<P: Scalar>(
    order: usize,
    centers: &[P],
    family: &[Enclosure<P>],
) -> Result<(), JointError> {
    for column in 0..order {
        for row in 0..order {
            let index = row + order * column;
            let transpose = column + order * row;
            let center = Finite::new(centers[index])?;
            if centers[index] != centers[transpose] || family[index] != family[transpose] {
                return Err(JointError::Asymmetric);
            }
            if !family[index].contains(center) {
                return Err(JointError::CannotValidateCovariance);
            }
        }
    }
    Ok(())
}
/// Symmetric nonnegative diagonal dominance proves PSD for every admitted entry choice.
fn dominant<P: Scalar>(
    order: usize,
    family: &[Enclosure<P>],
    context: &ArithmeticContext,
) -> Result<bool, nrz_scalar::ArithmeticFailure> {
    for row in 0..order {
        let mut radius = Enclosure::singleton(Finite::ZERO);
        for column in 0..order {
            if row != column {
                radius = radius.add(family[row + order * column].abs(), context)?;
            }
        }
        if family[row + order * row].lower() < radius.upper() {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Fixed-order interval Schur complements enclose the exact symmetric LDL elimination.
fn positive_pivots<P: Scalar>(
    order: usize,
    family: &[Enclosure<P>],
    context: &ArithmeticContext,
) -> Result<bool, nrz_scalar::ArithmeticFailure> {
    let mut schur = family.to_vec();
    for pivot in 0..order {
        let diagonal = schur[pivot + order * pivot];
        if diagonal.lower() <= Finite::ZERO {
            return Ok(false);
        }
        for column in pivot + 1..order {
            for row in column..order {
                let correction = schur[row + order * pivot]
                    .mul(schur[column + order * pivot], context)?
                    .div(diagonal, context)?;
                let next = schur[row + order * column].sub(correction, context)?;
                schur[row + order * column] = next;
                schur[column + order * row] = next;
            }
        }
    }
    Ok(true)
}
