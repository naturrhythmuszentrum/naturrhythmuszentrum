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
    /// Stable per-precision inventory index.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Binary32 => 0,
            Self::Binary64 => 1,
        }
    }

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
    /// Storage relocation epoch; old physical maps never resolve after compaction.
    pub(crate) epoch: u64,
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
    /// Relocation revision, independent of mathematical value versions.
    epoch: u64,
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
            epoch: 1,
            single: Slots::new(),
            double: Slots::new(),
            elements: [0, 0],
        }
    }

    /// Addressable legacy elements, independently bounded in each precision.
    pub(crate) const fn elements(&self) -> [usize; 2] {
        self.elements
    }

    /// Occupied numerical owners remain resident even when an individual extent is empty.
    pub(crate) fn has_owners(&self) -> bool {
        (self.single.iter().count() + self.double.iter().count()) != 0
    }
    /// Native arena inventory visits every occupied or vacant slot once.
    pub(crate) const fn scan_work(&self) -> usize {
        self.single.scan_work() + self.double.scan_work()
    }

    /// Actual observable backend row capacities and complete slot buffers.
    pub(crate) fn owned_bytes(&self) -> Result<usize, HeapError> {
        let mut bytes = self
            .single
            .owned_bytes()?
            .checked_add(self.double.owned_bytes()?)
            .ok_or(HeapError::Capacity)?;
        for (_, matrix) in self.single.iter() {
            bytes = bytes
                .checked_add(
                    matrix
                        .col_stride()
                        .unsigned_abs()
                        .checked_mul(size_of::<f32>())
                        .ok_or(HeapError::Capacity)?,
                )
                .ok_or(HeapError::Capacity)?;
        }
        for (_, matrix) in self.double.iter() {
            bytes = bytes
                .checked_add(
                    matrix
                        .col_stride()
                        .unsigned_abs()
                        .checked_mul(size_of::<f64>())
                        .ok_or(HeapError::Capacity)?,
                )
                .ok_or(HeapError::Capacity)?;
        }
        Ok(bytes)
    }

    /// Legacy numerical slab capacity, excluding both fixed slot-table allocations.
    pub(crate) fn numerical_bytes(&self) -> Result<usize, HeapError> {
        let tables =
            crate::heap::resources::add(self.single.owned_bytes()?, self.double.owned_bytes()?)?;
        self.owned_bytes()?
            .checked_sub(tables)
            .ok_or(HeapError::Capacity)
    }

    /// Plan amortized table growth after same-call retirement reuse, before slab allocation.
    pub(crate) fn growth_bytes<P: Scalar + 'static>(
        &self,
        count: usize,
        retired: &(impl Iterator<Item = Region> + Clone),
    ) -> Result<(usize, usize), HeapError> {
        let reuses = |key| (*retired).clone().any(|region| region.arena == key);
        match Format::of::<P>() {
            Format::Binary32 => self.single.growth_bytes_for(count, None, reuses),
            Format::Binary64 => self.double.growth_bytes_for(count, None, reuses),
        }
    }

    /// Statistical publication replaces an unpublished reservation after its initial growth.
    pub(crate) fn growth_bytes_sequence<P: Scalar + 'static>(
        &self,
        first: usize,
        last: usize,
    ) -> Result<(usize, usize), HeapError> {
        match Format::of::<P>() {
            Format::Binary32 => self.single.growth_bytes_sequence(first, last),
            Format::Binary64 => self.double.growth_bytes_sequence(first, last),
        }
    }

    /// Dense relocation inserts one owner at a time into the matching empty arena.
    pub(crate) fn dense_backing_bytes(
        format: Format,
        old: usize,
        count: usize,
    ) -> Result<usize, HeapError> {
        match format {
            Format::Binary32 => Slots::<Mat<f32>>::dense_backing_bytes(old, count),
            Format::Binary64 => Slots::<Mat<f64>>::dense_backing_bytes(old, count),
        }
    }

    /// Current physical-layout epoch; it advances only on complete relocation.
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Prepare an empty replacement owner with a checked successor epoch.
    pub(crate) fn replacement(&self) -> Result<Self, HeapError> {
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or(HeapError::CounterExhausted)?;
        Ok(Self {
            epoch,
            ..Self::new()
        })
    }

    /// Prepare a complete typed storage change, including all retired allocations.
    pub(crate) fn prepare<P: Scalar + 'static>(
        &mut self,
        payloads: &[&[P]],
        retired: &[Region],
        limits: HeapLimits,
        permit: &crate::heap::resources::ResourcePermit,
    ) -> Result<PreparedStorage, HeapError> {
        self.prepare_iter(payloads.iter().copied(), retired, limits, permit)
    }

    /// Borrow payload slices directly from their owners without allocating a reference vector.
    pub(crate) fn prepare_iter<'a, P: Scalar + 'static>(
        &mut self,
        payloads: impl Iterator<Item = &'a [P]> + Clone,
        retired: &[Region],
        limits: HeapLimits,
        permit: &crate::heap::resources::ResourcePermit,
    ) -> Result<PreparedStorage, HeapError> {
        let count = payloads.clone().count();
        permit.storage(self.epoch)?;
        let (elements, retired_keys) = self.preflight::<P>(payloads.clone(), retired, limits)?;
        match Format::of::<P>() {
            Format::Binary32 => {
                let plan = self
                    .single
                    .change(count, None, &retired_keys, limits.descriptors)?;
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(count)
                    .map_err(|_| HeapError::Capacity)?;
                for values in payloads {
                    matrices.push(pack_single(values)?);
                }
                Ok(PreparedStorage::Single {
                    slots: plan.bind(matrices)?,
                    elements,
                    epoch: self.epoch,
                })
            }
            Format::Binary64 => {
                let plan = self
                    .double
                    .change(count, None, &retired_keys, limits.descriptors)?;
                let mut matrices = Vec::new();
                matrices
                    .try_reserve_exact(count)
                    .map_err(|_| HeapError::Capacity)?;
                for values in payloads {
                    matrices.push(pack_double(values)?);
                }
                Ok(PreparedStorage::Double {
                    slots: plan.bind(matrices)?,
                    elements,
                    epoch: self.epoch,
                })
            }
        }
    }

    /// Validate old allocations and account for the complete future live set.
    fn preflight<'a, P: Scalar + 'static>(
        &self,
        payloads: impl Iterator<Item = &'a [P]>,
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
            PreparedStorage::Single {
                slots,
                elements,
                epoch,
            } => {
                self.epoch = epoch;
                self.single.publish(slots);
                self.elements[0] = elements;
            }
            PreparedStorage::Double {
                slots,
                elements,
                epoch,
            } => {
                self.epoch = epoch;
                self.double.publish(slots);
                self.elements[1] = elements;
            }
        }
    }

    /// Validate the arena generation and range before lending its storage.
    pub(crate) fn resolve(&self, region: Region) -> Result<SlabRef<'_>, HeapError> {
        if region.epoch != self.epoch {
            return Err(HeapError::StaleLayout);
        }
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
fn payload_size<'a, P: Scalar + 'a>(
    payloads: impl Iterator<Item = &'a [P]>,
) -> Result<usize, HeapError> {
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
        /// Epoch of the owner for which the change was prepared.
        epoch: u64,
    },
    /// A binary64 change.
    Double {
        /// Prepared owners and arena occupations.
        slots: PreparedSlots<Mat<f64>>,
        /// Future total live entries in this precision.
        elements: usize,
        /// Epoch of the owner for which the change was prepared.
        epoch: u64,
    },
}

impl PreparedStorage {
    /// Epoch captured during preparation, before constructing physical region records.
    pub(crate) const fn epoch(&self) -> u64 {
        match self {
            Self::Single { epoch, .. } | Self::Double { epoch, .. } => *epoch,
        }
    }

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

/// Inaccessible relocation-counter exhaustion invariant.
#[cfg(test)]
mod tests {
    use super::{Format, HeapError, HeapLimits, Region, Storage};

    /// Exhausted physical epochs refuse a new owner and leave current storage readable.
    /// ´test:unit:layout-epoch-exhaustion-never-wraps´
    #[test]
    fn layout_epoch_exhaustion_never_wraps() -> Result<(), HeapError> {
        let mut storage = Storage::new();
        storage.epoch = u64::MAX;
        let permit = crate::heap::resources::ResourcePermit {
            scope: 0,
            account: None,
            epoch: u64::MAX,
            retained: 0,
            peak: 0,
        };
        let prepared = storage.prepare::<f64>(&[&[7.0]], &[], HeapLimits::default(), &permit)?;
        let arena = *prepared.keys().first().ok_or(HeapError::Capacity)?;
        storage.publish(prepared);
        let region = Region {
            epoch: u64::MAX,
            arena,
            format: Format::Binary64,
            offset: 0,
            length: 1,
        };
        assert!(matches!(
            storage.replacement(),
            Err(HeapError::CounterExhausted)
        ));
        assert_eq!(storage.epoch(), u64::MAX);
        assert_eq!(
            storage.resolve(region)?.entry::<f64>(0)?.to_bits(),
            7.0_f64.to_bits()
        );
        Ok(())
    }
}
