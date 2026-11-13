// SPDX-License-Identifier: AGPL-3.0-only
//! Separate safe one-column owners never expose a writable slice or hole read.

use super::{LoanError, registry::Token};
use faer::Mat;
use nrz_scalar::Scalar;
use std::any::{Any, TypeId};

/// Precision-tagged native owner with no interior mutability.
pub(super) enum Numerical {
    /// Binary32 storage.
    Single(Mat<f32>),
    /// Binary64 storage.
    Double(Mat<f64>),
}
impl std::fmt::Debug for Numerical {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (format, rows) = match self {
            Self::Single(matrix) => (crate::storage::Format::Binary32, matrix.nrows()),
            Self::Double(matrix) => (crate::storage::Format::Binary64, matrix.nrows()),
        };
        // Formatting never loads selected values or holes ``rule:nrz:matrix-loans-selected-loads``.
        formatter
            .debug_struct("Numerical")
            .field("format", &format)
            .field("rows", &rows)
            .finish_non_exhaustive()
    }
}
impl Numerical {
    /// Check format and logical extent without loading any numerical component.
    pub(super) fn matches(&self, format: crate::storage::Format, length: usize) -> bool {
        match self {
            Self::Single(matrix) => {
                format == crate::storage::Format::Binary32 && matrix.nrows() == length
            }
            Self::Double(matrix) => {
                format == crate::storage::Format::Binary64 && matrix.nrows() == length
            }
        }
    }
    /// Construct only after rounded capacity and initialization Work are reserved.
    pub(super) fn zeroed<P: Scalar + 'static>(length: usize) -> Result<Self, LoanError> {
        let expected = crate::heap::resources::slab_bytes::<P>(length)? / size_of::<P>();
        #[cfg(test)]
        if length != 0 {
            super::fault::before()?;
        }
        let owner = if TypeId::of::<P>() == TypeId::of::<f32>() {
            let matrix = Mat::from_fn(length, 1, |_, _| 0.0_f32);
            if matrix.col_stride().unsigned_abs() != expected {
                return Err(LoanError::AllocationRefused);
            }
            Self::Single(matrix)
        } else {
            let matrix = Mat::from_fn(length, 1, |_, _| 0.0_f64);
            if matrix.col_stride().unsigned_abs() != expected {
                return Err(LoanError::AllocationRefused);
            }
            Self::Double(matrix)
        };
        Ok(owner)
    }
    /// Resolve the primitive storage type once for an authenticated selected traversal.
    pub(super) fn typed<P: Scalar + 'static>(&self) -> Result<&Mat<P>, LoanError> {
        let matrix: &dyn Any = match self {
            Self::Single(matrix) => matrix,
            Self::Double(matrix) => matrix,
        };
        matrix
            .downcast_ref::<Mat<P>>()
            .ok_or(LoanError::LayoutMismatch)
    }
    /// Read only after the caller checks initialized selected coverage.
    pub(super) fn read<P: Scalar + 'static>(&self, index: usize) -> Result<P, LoanError> {
        let value: &dyn Any = match self {
            Self::Single(matrix) if index < matrix.nrows() => &matrix[(index, 0)],
            Self::Double(matrix) if index < matrix.nrows() => &matrix[(index, 0)],
            Self::Single(_) | Self::Double(_) => return Err(LoanError::LayoutMismatch),
        };
        #[cfg(test)]
        super::fault::loaded();
        value
            .downcast_ref::<P>()
            .copied()
            .ok_or(LoanError::LayoutMismatch)
    }
    /// Checked initialization writes have no externally mutable alias.
    pub(super) fn write<P: Scalar + 'static>(
        &mut self,
        index: usize,
        value: P,
    ) -> Result<(), LoanError> {
        let value: &dyn Any = &value;
        match self {
            Self::Single(matrix) if index < matrix.nrows() => {
                matrix[(index, 0)] = *value
                    .downcast_ref::<f32>()
                    .ok_or(LoanError::LayoutMismatch)?;
            }
            Self::Double(matrix) if index < matrix.nrows() => {
                matrix[(index, 0)] = *value
                    .downcast_ref::<f64>()
                    .ok_or(LoanError::LayoutMismatch)?;
            }
            Self::Single(_) | Self::Double(_) => return Err(LoanError::LayoutMismatch),
        }
        #[cfg(test)]
        super::fault::stored();
        Ok(())
    }
}
/// Test sentinels run after the actual native backing has been destroyed, without retaining it.
#[cfg(test)]
impl Drop for Numerical {
    fn drop(&mut self) {
        let (length, bytes) = match self {
            Self::Single(matrix) => {
                let counts = (
                    matrix.nrows(),
                    matrix.col_stride().unsigned_abs() * size_of::<f32>(),
                );
                drop(std::mem::replace(matrix, Mat::new()));
                counts
            }
            Self::Double(matrix) => {
                let counts = (
                    matrix.nrows(),
                    matrix.col_stride().unsigned_abs() * size_of::<f64>(),
                );
                drop(std::mem::replace(matrix, Mat::new()));
                counts
            }
        };
        if length != 0 {
            super::fault::released_numerical(length, bytes);
        }
    }
}
/// A channel's separately owned payload and both initialization maps.
#[derive(Debug)]
pub(super) struct LoanChannel {
    /// One contiguous native numerical allocation.
    pub(super) matrix: Numerical,
    /// One flag per addressable logical component.
    pub(super) components: Vec<u64>,
    /// Derived complete-cell flags, recalculated before handback.
    pub(super) complete: Vec<u64>,
    /// Unique payload and tracking reservations.
    pub(super) tokens: [Token; 3],
}
/// Immutable numerical payload; initialization maps have been destroyed.
pub(super) struct PublishedChannel {
    /// Moved numerical allocation.
    pub(super) matrix: Numerical,
    /// Its original unique destruction entitlement and debit identity.
    pub(super) token: Token,
}
impl std::fmt::Debug for PublishedChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PublishedChannel")
            .field("matrix", &self.matrix)
            .field("token", &self.token)
            .finish()
    }
}
/// Read a private initialization bit without interpreting padding as a cell.
pub(super) fn bit(flags: &[u64], index: usize) -> bool {
    flags
        .get(index / 64)
        .is_some_and(|word| word & (1_u64 << (index % 64)) != 0)
}
/// Bounds are checked before any mutating setter begins.
pub(super) fn set(flags: &mut [u64], index: usize, value: bool) {
    if let Some(word) = flags.get_mut(index / 64) {
        let mask = 1_u64 << (index % 64);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
    }
}
