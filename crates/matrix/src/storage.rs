// SPDX-License-Identifier: AGPL-3.0-only
//! Owned one-column faer slabs, separated by precision and accessed through checked indices.

use std::any::{Any, TypeId};

use faer::Mat;
use nrz_scalar::Scalar;

use crate::heap::identity::Key;
use crate::heap::slots::Slots;
use crate::{HeapError, HeapLimits};

/// Runtime format tag; external scalar implementations are sealed by `Scalar`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    /// IEEE binary32.
    Binary32,
    /// IEEE binary64.
    Binary64,
}

impl Format {
    /// Resolve the sealed primitive format without inspecting any numerical value.
    pub(crate) fn of<P: Scalar + 'static>() -> Self {
        if TypeId::of::<P>() == TypeId::of::<f32>() {
            Self::Binary32
        } else {
            Self::Binary64
        }
    }
}

/// A contiguous occupied range within a precision-specific slab.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    /// Arena occupation, deliberately absent from the external handle.
    pub(crate) arena: Key,
    /// Scalar format.
    pub(crate) format: Format,
    /// Starting element in the slab.
    pub(crate) offset: usize,
    /// Allocated element count, excluding backend capacity padding.
    pub(crate) length: usize,
}

/// Independent typed collections of owned slabs.
#[derive(Debug)]
pub(crate) struct Storage {
    /// Binary32 owners.
    single: Slots<Mat<f32>>,
    /// Binary64 owners.
    double: Slots<Mat<f64>>,
    /// Live element counts in binary32 and binary64 respectively.
    elements: [usize; 2],
}

impl Storage {
    /// Empty typed arenas.
    pub(crate) const fn new() -> Self {
        Self {
            single: Slots::new(),
            double: Slots::new(),
            elements: [0, 0],
        }
    }

    /// Check capacity and reserve all arena table growth before backend allocation.
    pub(crate) fn reserve<P: Scalar + 'static>(
        &mut self,
        count: usize,
        length: usize,
        limits: HeapLimits,
    ) -> Result<Vec<Key>, HeapError> {
        let bytes = length
            .checked_mul(size_of::<P>())
            .ok_or(HeapError::ExtentOverflow)?;
        // Leave room for backend alignment rounding as well as signed address arithmetic.
        if bytes > isize::MAX.unsigned_abs() / 2 {
            return Err(HeapError::ExtentOverflow);
        }
        let amount = length.checked_mul(count).ok_or(HeapError::Capacity)?;
        match Format::of::<P>() {
            Format::Binary32 => {
                check_capacity(self.elements[0], amount, limits.elements)?;
                self.single.prepare(count, limits.descriptors)
            }
            Format::Binary64 => {
                check_capacity(self.elements[1], amount, limits.elements)?;
                self.double.prepare(count, limits.descriptors)
            }
        }
    }

    /// Install only fully prepared matrices into previously reserved tables.
    pub(crate) fn publish(&mut self, prepared: PreparedStorage) {
        match prepared {
            PreparedStorage::Single { matrices, elements } => {
                self.single.publish(matrices.into_iter());
                self.elements[0] += elements;
            }
            PreparedStorage::Double { matrices, elements } => {
                self.double.publish(matrices.into_iter());
                self.elements[1] += elements;
            }
        }
    }

    /// Validate the arena generation and range before lending its storage.
    pub(crate) fn resolve(&self, region: Region) -> Result<SlabRef<'_>, HeapError> {
        let slab = match region.format {
            Format::Binary32 => SlabRef::Single(self.single.get(region.arena)?),
            Format::Binary64 => SlabRef::Double(self.double.get(region.arena)?),
        };
        let end = region
            .offset
            .checked_add(region.length)
            .ok_or(HeapError::ExtentOverflow)?;
        if end > slab.len()? {
            return Err(HeapError::InvalidLayout);
        }
        Ok(slab)
    }
}

/// Check the complete bundle against the precision's element capacity.
fn check_capacity(used: usize, amount: usize, limit: usize) -> Result<(), HeapError> {
    if used.checked_add(amount).is_none_or(|total| total > limit) {
        Err(HeapError::Capacity)
    } else {
        Ok(())
    }
}

/// Backend allocations still private to a transaction.
#[derive(Debug)]
pub(crate) enum PreparedStorage {
    /// A binary32 import bundle.
    Single {
        /// Owned value and optional bound slabs.
        matrices: Vec<Mat<f32>>,
        /// Total prepared entries.
        elements: usize,
    },
    /// A binary64 import bundle.
    Double {
        /// Owned value and optional bound slabs.
        matrices: Vec<Mat<f64>>,
        /// Total prepared entries.
        elements: usize,
    },
}

/// Pack a primitive format through safe, bounds-checked faer indexing.
macro_rules! pack {
    ($function:ident, $ty:ty) => {
        /// Copy into a one-column slab without changing scalar representation.
        fn $function<P: Scalar + 'static>(values: &[P]) -> Result<Mat<$ty>, HeapError> {
            let mut matrix = Mat::<$ty>::from_fn(values.len(), 1, |_, _| 0.0);
            for (index, value) in values.iter().enumerate() {
                let canonical = if *value == P::ZERO { P::ZERO } else { *value };
                let entry = (&canonical as &dyn Any)
                    .downcast_ref::<$ty>()
                    .ok_or(HeapError::PrecisionMismatch)?;
                if index >= matrix.nrows() || matrix.ncols() != 1 {
                    return Err(HeapError::InvalidLayout);
                }
                matrix[(index, 0)] = *entry;
            }
            Ok(matrix)
        }
    };
}

pack!(pack_single, f32);
pack!(pack_double, f64);

impl PreparedStorage {
    /// Prepare every slab; no public descriptor points here until publication.
    pub(crate) fn new<P: Scalar + 'static>(
        centers: &[P],
        bounds: Option<&[P]>,
    ) -> Result<Self, HeapError> {
        let count = 1 + usize::from(bounds.is_some());
        let elements = centers
            .len()
            .checked_mul(count)
            .ok_or(HeapError::Capacity)?;
        match Format::of::<P>() {
            Format::Binary32 => {
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(count)
                    .map_err(|_| HeapError::Capacity)?;
                for values in [Some(centers), bounds].into_iter().flatten() {
                    matrices.push(pack_single(values)?);
                }
                Ok(Self::Single { matrices, elements })
            }
            Format::Binary64 => {
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(count)
                    .map_err(|_| HeapError::Capacity)?;
                for values in [Some(centers), bounds].into_iter().flatten() {
                    matrices.push(pack_double(values)?);
                }
                Ok(Self::Double { matrices, elements })
            }
        }
    }
}

/// Read-only borrow of one checked slab; never an external backend view.
#[derive(Debug)]
pub(crate) enum SlabRef<'h> {
    /// Binary32 storage.
    Single(&'h Mat<f32>),
    /// Binary64 storage.
    Double(&'h Mat<f64>),
}

impl SlabRef<'_> {
    /// Allocated addressable entries; every slab has one column.
    fn len(&self) -> Result<usize, HeapError> {
        match self {
            Self::Single(matrix) if matrix.ncols() == 1 => Ok(matrix.nrows()),
            Self::Double(matrix) if matrix.ncols() == 1 => Ok(matrix.nrows()),
            Self::Single(_) | Self::Double(_) => Err(HeapError::InvalidLayout),
        }
    }

    /// Read an exact primitive after range and format checks, without a numeric cast.
    pub(crate) fn entry<P: Scalar + 'static>(&self, index: usize) -> Result<P, HeapError> {
        if index >= self.len()? {
            return Err(HeapError::InvalidLayout);
        }
        let value: &dyn Any = match self {
            Self::Single(matrix) if matrix.ncols() == 1 => &matrix[(index, 0)],
            Self::Double(matrix) if matrix.ncols() == 1 => &matrix[(index, 0)],
            Self::Single(_) | Self::Double(_) => return Err(HeapError::InvalidLayout),
        };
        value
            .downcast_ref::<P>()
            .copied()
            .ok_or(HeapError::PrecisionMismatch)
    }
}
