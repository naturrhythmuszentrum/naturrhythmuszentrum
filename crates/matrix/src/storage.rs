// SPDX-License-Identifier: AGPL-3.0-only
//! Owned one-column faer slabs, separated by precision and accessed through checked indices.

use std::any::{Any, TypeId};

use faer::Mat;
use nrz_scalar::Scalar;

use crate::heap::identity::Key;
use crate::heap::slots::{PreparedSlots, Slots};
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

    /// Prepare a complete typed storage change, including all retired allocations.
    pub(crate) fn prepare<P: Scalar + 'static>(
        &mut self,
        payloads: &[&[P]],
        retired: &[Region],
        limits: HeapLimits,
    ) -> Result<PreparedStorage, HeapError> {
        let (elements, retired_keys) = self.preflight::<P>(payloads, retired, limits)?;
        match Format::of::<P>() {
            Format::Binary32 => {
                let plan =
                    self.single
                        .change(payloads.len(), None, &retired_keys, limits.descriptors)?;
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(payloads.len())
                    .map_err(|_| HeapError::Capacity)?;
                for values in payloads {
                    matrices.push(pack_single(values)?);
                }
                Ok(PreparedStorage::Single {
                    slots: plan.bind(matrices)?,
                    elements,
                })
            }
            Format::Binary64 => {
                let plan =
                    self.double
                        .change(payloads.len(), None, &retired_keys, limits.descriptors)?;
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(payloads.len())
                    .map_err(|_| HeapError::Capacity)?;
                for values in payloads {
                    matrices.push(pack_double(values)?);
                }
                Ok(PreparedStorage::Double {
                    slots: plan.bind(matrices)?,
                    elements,
                })
            }
        }
    }

    /// Validate old allocations and account for the complete future live set.
    fn preflight<P: Scalar + 'static>(
        &self,
        payloads: &[&[P]],
        retired: &[Region],
        limits: HeapLimits,
    ) -> Result<(usize, Vec<Key>), HeapError> {
        let mut removed = 0_usize;
        let mut keys = Vec::new();
        keys.try_reserve_exact(retired.len())
            .map_err(|_| HeapError::Capacity)?;
        for region in retired {
            if region.format != Format::of::<P>() {
                return Err(HeapError::PrecisionMismatch);
            }
            let slab = self.resolve(*region)?;
            if region.offset != 0 || region.length != slab.len()? || keys.contains(&region.arena) {
                return Err(HeapError::InvalidLayout);
            }
            removed = removed
                .checked_add(region.length)
                .ok_or(HeapError::Capacity)?;
            keys.push(region.arena);
        }
        let amount = payload_size(payloads)?;
        let used = match Format::of::<P>() {
            Format::Binary32 => self.elements[0],
            Format::Binary64 => self.elements[1],
        };
        let total = used
            .checked_sub(removed)
            .and_then(|remaining| remaining.checked_add(amount))
            .ok_or(HeapError::Capacity)?;
        if total > limits.elements {
            return Err(HeapError::Capacity);
        }
        Ok((total, keys))
    }

    /// Install only complete typed changes; their counters were checked in preparation.
    pub(crate) fn publish(&mut self, prepared: PreparedStorage) {
        match prepared {
            PreparedStorage::Single { slots, elements } => {
                self.single.publish(slots);
                self.elements[0] = elements;
            }
            PreparedStorage::Double { slots, elements } => {
                self.double.publish(slots);
                self.elements[1] = elements;
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

/// Check scalar address arithmetic for every scratch allocation before entering faer.
fn payload_size<P: Scalar>(payloads: &[&[P]]) -> Result<usize, HeapError> {
    let mut amount = 0_usize;
    for values in payloads {
        let bytes = values
            .len()
            .checked_mul(size_of::<P>())
            .ok_or(HeapError::ExtentOverflow)?;
        // Reserve space for backend alignment rounding and signed address arithmetic.
        if bytes > isize::MAX.unsigned_abs() / 2 {
            return Err(HeapError::ExtentOverflow);
        }
        amount = amount
            .checked_add(values.len())
            .ok_or(HeapError::Capacity)?;
    }
    Ok(amount)
}

/// Complete private backend allocations and their checked occupation changes.
#[derive(Debug)]
pub(crate) enum PreparedStorage {
    /// A binary32 change.
    Single {
        /// Prepared owners and arena occupations.
        slots: PreparedSlots<Mat<f32>>,
        /// Future total live entries in this precision.
        elements: usize,
    },
    /// A binary64 change.
    Double {
        /// Prepared owners and arena occupations.
        slots: PreparedSlots<Mat<f64>>,
        /// Future total live entries in this precision.
        elements: usize,
    },
}

impl PreparedStorage {
    /// The exact future arena keys, before any visible publication.
    pub(crate) fn keys(&self) -> &[Key] {
        match self {
            Self::Single { slots, .. } => slots.keys(),
            Self::Double { slots, .. } => slots.keys(),
        }
    }
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
