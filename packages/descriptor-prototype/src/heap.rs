// SPDX-License-Identifier: AGPL-3.0-only
//! A small contiguous arena used only to investigate admission and identity.

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Neg, Sub};

use nrz_algebra::Rational;
use nrz_units::{UncertaintyMode, uncertainty};

use crate::{Error, SchemaPool, ShapeId, error::identity};

/// Private sealing keeps categories and precisions closed.
mod sealed {
    /// Only locally implemented types can be precision markers.
    pub trait Precision {}
    impl Precision for f32 {}
    impl Precision for f64 {}
}

/// The two plain scalar precisions used by the prototype.
pub trait Scalar:
    sealed::Precision
    + Copy
    + Debug
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// Exact zero.
    const ZERO: Self;
    /// Exact one.
    const ONE: Self;
    /// Whether a coefficient can enter this finite-only toy arena.
    fn finite(self) -> bool;
    /// Round an exact rational to this precision; no error bound is claimed.
    fn rational(value: Rational) -> Self;
}

impl Scalar for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    fn finite(self) -> bool {
        self.is_finite()
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "prototype maps round explicitly; fixture is dyadic"
    )]
    fn rational(value: Rational) -> Self {
        value.numerator() as Self / value.denominator() as Self
    }
}

impl Scalar for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    fn finite(self) -> bool {
        self.is_finite()
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "prototype maps round explicitly; fixture is dyadic"
    )]
    fn rational(value: Rational) -> Self {
        value.numerator() as Self / value.denominator() as Self
    }
}

/// Value category, independent of the quantity's physical meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Value {
    /// Prevent construction outside this module.
    _private: (),
}
/// Independently stored uncertainty category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Companion {
    /// Prevent construction outside this module.
    _private: (),
}
/// Opaque factor-resource category; no numerical factorization is implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactorResource {
    /// Prevent construction outside this module.
    _private: (),
}
/// Scratch category; never accepted as an input value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Workspace {
    /// Prevent construction outside this module.
    _private: (),
}

/// An unforgeable reference; resolution is still required on every access.
///
/// Companion handles cannot substitute for value handles.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Handle, Companion, Value};
/// fn misuse(handle: Handle<Companion, f64>) {
///     let _: Handle<Value, f64> = handle;
/// }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Handle, Companion};
/// fn accepted(handle: Handle<Companion, f64>) {
///     let _: Handle<Companion, f64> = handle;
/// }
/// ```
///
/// `FactorResource` handles cannot substitute for value handles.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Handle, FactorResource, Value};
/// fn misuse(handle: Handle<FactorResource, f64>) {
///     let _: Handle<Value, f64> = handle;
/// }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Handle, FactorResource};
/// fn accepted(handle: Handle<FactorResource, f64>) {
///     let _: Handle<FactorResource, f64> = handle;
/// }
/// ```
///
/// Workspace handles cannot substitute for value handles.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Handle, Workspace, Value};
/// fn misuse(handle: Handle<Workspace, f64>) {
///     let _: Handle<Value, f64> = handle;
/// }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Handle, Workspace};
/// fn accepted(handle: Handle<Workspace, f64>) {
///     let _: Handle<Workspace, f64> = handle;
/// }
/// ```
///
/// Precision cannot change through handle assignment.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Handle, Value};
/// fn misuse(handle: Handle<Value, f32>) {
///     let _: Handle<Value, f64> = handle;
/// }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Handle, Value};
/// fn accepted(handle: Handle<Value, f32>) {
///     let _: Handle<Value, f32> = handle;
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Handle<C, P> {
    /// Owning heap identity.
    heap: u64,
    /// Stable slot index.
    slot: usize,
    /// Slot lifecycle generation.
    generation: u64,
    /// Arena relocation generation.
    arena: u64,
    /// Closed category and precision carried without stored references.
    marker: PhantomData<(C, P)>,
}

/// Read-only reference identity retained in client diagnostics, never an access capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceInfo {
    /// Heap identity.
    pub heap: u64,
    /// Descriptor slot.
    pub slot: usize,
    /// Slot lifecycle generation.
    pub generation: u64,
    /// Arena lifecycle generation.
    pub arena: u64,
}

impl<C, P> Handle<C, P> {
    /// Identify a reference for diagnostics without authorizing storage access.
    #[must_use]
    pub const fn reference(&self) -> ReferenceInfo {
        ReferenceInfo {
            heap: self.heap,
            slot: self.slot,
            generation: self.generation,
            arena: self.arena,
        }
    }
}

/// Runtime counterpart of the closed public handle categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Category {
    /// A physical value.
    Value,
    /// An independently stored uncertainty.
    Companion,
    /// An opaque factor resource.
    Factor,
    /// Scratch storage.
    Workspace,
}

/// A versioned selection of logical value entries, not a schema identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target<P> {
    /// Value reference captured at attachment time.
    pub(crate) value: Handle<Value, P>,
    /// Value version captured independently of the slot generation.
    pub(crate) version: u64,
    /// Ordered logical entry selection.
    pub(crate) entries: Vec<usize>,
}

/// One separately owned slot; values use column-major logical order here.
#[derive(Debug)]
struct Record<P> {
    /// Runtime category checked in addition to static typing.
    category: Category,
    /// Independent immutable shape schema.
    shape: ShapeId,
    /// Finite physical coefficients.
    values: Vec<P>,
    /// Lifecycle generation; dead slots are not reused in this prototype.
    generation: u64,
    /// Changes when coefficients change without replacing the slot.
    version: u64,
    /// Whether the slot can still be resolved.
    live: bool,
    /// Ordered companion targets; covariance has exactly two sides.
    targets: Vec<Target<P>>,
}

/// Precision-specific storage and owned immutable schema pool.
#[derive(Debug)]
pub struct Heap<P> {
    /// Heap scope.
    id: u64,
    /// Arena generation, distinct from every slot generation.
    arena: u64,
    /// Schema definitions cannot be edited after the heap takes ownership.
    schemas: SchemaPool,
    /// Append-only slots; invalidation never wraps a generation.
    records: Vec<Record<P>>,
}

impl<P: Scalar> Heap<P> {
    /// Take ownership of admitted schemas and allocate an independent heap.
    ///
    /// # Errors
    /// Refuses identity exhaustion.
    pub fn new(schemas: SchemaPool) -> Result<Self, Error> {
        Ok(Self {
            id: identity()?,
            arena: 0,
            schemas,
            records: Vec::new(),
        })
    }

    /// Immutable descriptor and shape lookup.
    #[must_use]
    pub const fn schemas(&self) -> &SchemaPool {
        &self.schemas
    }

    /// Count live objects, including companions and resources.
    #[must_use]
    pub fn live_count(&self) -> usize {
        self.records.iter().filter(|record| record.live).count()
    }

    /// Validate finite continuous storage, leaving discrete counts unsupported.
    pub(crate) fn validate_values(&self, shape: ShapeId, values: &[P]) -> Result<(), Error> {
        let definition = self.schemas.shape_definition(shape)?;
        if definition.entries.len() != values.len() {
            return Err(Error::Shape);
        }
        if values.iter().any(|value| !value.finite()) {
            return Err(Error::Numerical);
        }
        for quantity in &definition.entries {
            if self
                .schemas
                .descriptor(*quantity)?
                .kind()
                .count_domain()
                .is_some()
            {
                return Err(Error::Unsupported);
            }
        }
        Ok(())
    }

    /// Insert after validation; every public constructor fixes its category.
    fn insert<C>(
        &mut self,
        category: Category,
        shape: ShapeId,
        values: Vec<P>,
        targets: Vec<Target<P>>,
    ) -> Result<Handle<C, P>, Error> {
        self.validate_values(shape, &values)?;
        let handle = Handle {
            heap: self.id,
            slot: self.records.len(),
            generation: 0,
            arena: self.arena,
            marker: PhantomData,
        };
        self.records.push(Record {
            category,
            shape,
            values,
            generation: 0,
            version: 0,
            live: true,
            targets,
        });
        Ok(handle)
    }

    /// Import finite physical values; schema admission is not a plan check.
    ///
    /// # Errors
    /// Refuses invalid shapes, nonfinite data and floating count storage.
    pub fn value(&mut self, shape: ShapeId, values: Vec<P>) -> Result<Handle<Value, P>, Error> {
        self.insert(Category::Value, shape, values, Vec::new())
    }

    /// Allocate a toy factor resource with no factorization assurance.
    ///
    /// # Errors
    /// Refuses invalid storage contracts.
    pub fn factor(
        &mut self,
        shape: ShapeId,
        values: Vec<P>,
    ) -> Result<Handle<FactorResource, P>, Error> {
        self.insert(Category::Factor, shape, values, Vec::new())
    }

    /// Allocate finite scratch storage.
    ///
    /// # Errors
    /// Refuses invalid storage contracts.
    pub fn workspace(
        &mut self,
        shape: ShapeId,
        values: Vec<P>,
    ) -> Result<Handle<Workspace, P>, Error> {
        self.insert(Category::Workspace, shape, values, Vec::new())
    }

    /// Resolve identity, category, generation and range before lending data.
    #[expect(
        clippy::suspicious_operation_groupings,
        reason = "the heap's identity field is id while a handle names it heap; the comparison is intended"
    )]
    fn resolve<C>(&self, handle: &Handle<C, P>, category: Category) -> Result<&Record<P>, Error> {
        if handle.heap != self.id || handle.arena != self.arena {
            return Err(Error::Reference);
        }
        let record = self.records.get(handle.slot).ok_or(Error::Reference)?;
        if !record.live || record.generation != handle.generation || record.category != category {
            return Err(Error::Reference);
        }
        self.validate_values(record.shape, &record.values)?;
        Ok(record)
    }

    /// Read a physical value after fresh reference validation.
    ///
    /// # Errors
    /// Refuses stale or foreign references.
    pub fn values(&self, handle: Handle<Value, P>) -> Result<&[P], Error> {
        Ok(&self.resolve(&handle, Category::Value)?.values)
    }

    /// Resolve the current shape and value version.
    ///
    /// # Errors
    /// Refuses stale or foreign references.
    pub fn contract(&self, handle: Handle<Value, P>) -> Result<(ShapeId, u64), Error> {
        let record = self.resolve(&handle, Category::Value)?;
        Ok((record.shape, record.version))
    }

    /// Mutate coefficients without changing slot identity; retained evidence expires.
    ///
    /// # Errors
    /// Refuses invalid data, references or version exhaustion without mutation.
    pub fn replace(&mut self, handle: Handle<Value, P>, values: Vec<P>) -> Result<(), Error> {
        let (shape, version) = self.contract(handle)?;
        self.validate_values(shape, &values)?;
        let version = version.checked_add(1).ok_or(Error::Exhausted)?;
        let record = self.records.get_mut(handle.slot).ok_or(Error::Reference)?;
        record.values = values;
        record.version = version;
        Ok(())
    }

    /// Retire a value reference without reusing its slot.
    ///
    /// # Errors
    /// Refuses an invalid reference or generation exhaustion.
    pub fn retire(&mut self, handle: Handle<Value, P>) -> Result<(), Error> {
        self.resolve(&handle, Category::Value)?;
        let record = self.records.get_mut(handle.slot).ok_or(Error::Reference)?;
        record.generation = record.generation.checked_add(1).ok_or(Error::Exhausted)?;
        record.live = false;
        Ok(())
    }

    /// Invalidate every existing reference, modelling an arena lifecycle change.
    ///
    /// # Errors
    /// Refuses generation exhaustion without changing the arena.
    pub fn invalidate_arena(&mut self) -> Result<(), Error> {
        self.arena = self.arena.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(())
    }

    /// Capture an ordered selection and its current value version.
    ///
    /// # Errors
    /// Refuses empty, out-of-range, stale or foreign selections.
    pub fn target(&self, value: Handle<Value, P>, entries: Vec<usize>) -> Result<Target<P>, Error> {
        let record = self.resolve(&value, Category::Value)?;
        if entries.is_empty() || entries.iter().any(|entry| *entry >= record.values.len()) {
            return Err(Error::Shape);
        }
        Ok(Target {
            value,
            version: record.version,
            entries,
        })
    }

    /// Resolve every target entry against a still-current value version.
    fn target_quantities(&self, target: &Target<P>) -> Result<Vec<&nrz_units::Descriptor>, Error> {
        let record = self.resolve(&target.value, Category::Value)?;
        if record.version != target.version {
            return Err(Error::Version);
        }
        let shape = self.schemas.shape_definition(record.shape)?;
        target
            .entries
            .iter()
            .map(|entry| {
                let quantity = shape.entries.get(*entry).ok_or(Error::Shape)?;
                self.schemas.descriptor(*quantity)
            })
            .collect()
    }

    /// Attach full covariance with its own shape and both ordered target sides.
    ///
    /// # Errors
    /// Refuses stale targets, incorrect extents or non-CAS covariance descriptors.
    pub fn covariance(
        &mut self,
        shape: ShapeId,
        values: Vec<P>,
        left: Target<P>,
        right: Target<P>,
    ) -> Result<Handle<Companion, P>, Error> {
        let left_quantities = self.target_quantities(&left)?;
        let right_quantities = self.target_quantities(&right)?;
        let definition = self.schemas.shape_definition(shape)?;
        let left_shape = self
            .schemas
            .shape_definition(self.contract(left.value)?.0)?;
        let right_shape = self
            .schemas
            .shape_definition(self.contract(right.value)?.0)?;
        if left_shape.columns.len() != 1 || right_shape.columns.len() != 1 {
            return Err(Error::Unsupported);
        }
        let left_axes: Vec<_> = left
            .entries
            .iter()
            .map(|entry| left_shape.rows[*entry].clone())
            .collect();
        let right_axes: Vec<_> = right
            .entries
            .iter()
            .map(|entry| right_shape.rows[*entry].clone())
            .collect();
        if definition.rows != left_axes || definition.columns != right_axes {
            return Err(Error::Shape);
        }
        if definition.rows.len() != left_quantities.len()
            || definition.columns.len() != right_quantities.len()
        {
            return Err(Error::Shape);
        }
        for (column, right_quantity) in right_quantities.iter().enumerate() {
            for (row, left_quantity) in left_quantities.iter().enumerate() {
                let expected = uncertainty(
                    UncertaintyMode::Covariance,
                    left_quantity,
                    Some(right_quantity),
                    None,
                )?;
                let index = row + left_quantities.len() * column;
                let actual = self.schemas.descriptor(definition.entries[index])?;
                if actual != &expected {
                    return Err(Error::Shape);
                }
            }
        }
        self.insert(Category::Companion, shape, values, vec![left, right])
    }

    /// Read covariance only while all its captured target versions remain current.
    ///
    /// # Errors
    /// Refuses foreign, stale or invalidated companion evidence.
    pub fn companion_values(&self, handle: Handle<Companion, P>) -> Result<&[P], Error> {
        let record = self.resolve(&handle, Category::Companion)?;
        for target in &record.targets {
            self.target_quantities(target)?;
        }
        Ok(&record.values)
    }

    /// Require the exact value identities and selections expected by a caller.
    ///
    /// # Errors
    /// Refuses stale companions or mismatched ordered coverage.
    pub fn covariance_for(
        &self,
        handle: Handle<Companion, P>,
        left: &Target<P>,
        right: &Target<P>,
    ) -> Result<&[P], Error> {
        let record = self.resolve(&handle, Category::Companion)?;
        if record.targets.as_slice() != [left.clone(), right.clone()] {
            return Err(Error::Version);
        }
        self.companion_values(handle)
    }
}
