// SPDX-License-Identifier: AGPL-3.0-only
//! Checked physical maps and fixed role markers, separate from shape identity.

use std::marker::PhantomData;

use nrz_algebra::Rational;
use nrz_units::{Descriptor, Role};

use crate::{Error, Handle, Heap, Scalar, Value};

/// A role engine code deliberately fixes to affine points.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    /// Prevent construction outside this module.
    _private: (),
}
/// A role engine code deliberately fixes to linear differences.
#[derive(Clone, Copy, Debug)]
pub struct Difference {
    /// Prevent construction outside this module.
    _private: (),
}

/// Exact physical reference coordinates for one normalization axis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateMap {
    /// Physical quantity in which stored coefficients are expressed.
    descriptor: Descriptor,
    /// Positive reference expressed in the stored difference measure.
    scale: Rational,
    /// Affine origin expressed in the stored point measure, or zero for differences.
    origin: Rational,
}

impl CoordinateMap {
    /// Check a typed positive difference reference and, for points, a typed origin.
    ///
    /// # Errors
    /// Refuses incompatible kinds, roles, missing origins and nonpositive scales.
    pub fn new(
        descriptor: Descriptor,
        reference: (&Descriptor, Rational),
        origin: Option<(&Descriptor, Rational)>,
    ) -> Result<Self, Error> {
        if reference.0.role() != Role::Difference {
            return Err(Error::Map);
        }
        let scale = reference
            .0
            .conversion_to(&descriptor.difference())?
            .apply(reference.1)?;
        if scale <= Rational::ZERO {
            return Err(Error::Map);
        }
        let origin = match (descriptor.role(), origin) {
            (Role::Point, Some((quantity, value))) => {
                quantity.conversion_to(&descriptor)?.apply(value)?
            }
            (Role::Difference, None) => Rational::ZERO,
            _ => return Err(Error::Map),
        };
        Ok(Self {
            descriptor,
            scale,
            origin,
        })
    }

    /// Physical schema to which the map applies.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Positive physical difference reference in storage coordinates.
    #[must_use]
    pub const fn scale(&self) -> Rational {
        self.scale
    }

    /// Affine point origin, or zero for a difference.
    #[must_use]
    pub const fn origin(&self) -> Rational {
        self.origin
    }

    /// Apply the exact normalization map to an exact coordinate.
    ///
    /// # Errors
    /// Refuses rational overflow.
    pub fn normalize_exact(&self, value: Rational) -> Result<Rational, Error> {
        Ok(value.checked_sub(self.origin)?.checked_div(self.scale)?)
    }

    /// Restore a rounded numerical coordinate, retaining explicit finiteness checks.
    pub(crate) fn restore<P: Scalar>(&self, value: P) -> Result<P, Error> {
        let restored = value * P::rational(self.scale) + P::rational(self.origin);
        if !restored.finite() {
            return Err(Error::Numerical);
        }
        Ok(restored)
    }
}

/// Physical storage with an explicitly checked role; this is not a kernel input.
///
/// Fixed point and difference roles remain distinct.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Difference, PhysicalView, Point};
/// fn difference(_: PhysicalView<'_, f64, Difference>) {}
/// fn misuse(view: PhysicalView<'_, f64, Point>) { difference(view); }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Difference, PhysicalView};
/// fn difference(_: PhysicalView<'_, f64, Difference>) {}
/// fn accepted(view: PhysicalView<'_, f64, Difference>) { difference(view); }
/// ```
#[derive(Debug)]
pub struct PhysicalView<'a, P, R> {
    /// Borrowed physical coefficients, never normalized implicitly.
    values: &'a [P],
    /// Fixed role established by a runtime lift.
    role: PhantomData<R>,
}

impl<P, R> PhysicalView<'_, P, R> {
    /// Physical coefficients; a physical view cannot be passed to the toy kernel.
    #[must_use]
    pub const fn values(&self) -> &[P] {
        self.values
    }
}

/// Check every logical entry before fixing the role in a view's type.
fn physical<P: Scalar, R>(
    heap: &Heap<P>,
    value: Handle<Value, P>,
    role: Role,
) -> Result<PhysicalView<'_, P, R>, Error> {
    let shape = heap.schemas().shape_definition(heap.contract(value)?.0)?;
    for quantity in &shape.entries {
        if heap.schemas().descriptor(*quantity)?.role() != role {
            return Err(Error::Map);
        }
    }
    Ok(PhysicalView {
        values: heap.values(value)?,
        role: PhantomData,
    })
}

/// Lift a physical difference view after checking the dynamic descriptors.
///
/// # Errors
/// Refuses stale references and entries with another role.
pub fn difference_view<P: Scalar>(
    heap: &Heap<P>,
    value: Handle<Value, P>,
) -> Result<PhysicalView<'_, P, Difference>, Error> {
    physical(heap, value, Role::Difference)
}

/// Lift a physical point view after checking the dynamic descriptors.
///
/// # Errors
/// Refuses stale references and entries with another role.
pub fn point_view<P: Scalar>(
    heap: &Heap<P>,
    value: Handle<Value, P>,
) -> Result<PhysicalView<'_, P, Point>, Error> {
    physical(heap, value, Role::Point)
}
