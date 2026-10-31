// SPDX-License-Identifier: AGPL-3.0-only
//! Optional sealed affine markers never replace dynamic descriptor checking.

use super::{Quantity, QuantityError, charge};
use crate::Heap;
use nrz_bindings::{Budget, Role, UnitError};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Closed role marker vocabulary.
mod sealed {
    /// External crates cannot create a marker that bypasses a checked lift.
    pub trait Sealed {}
}

/// A sealed optional affine role, independent of dimension and numerical precision.
pub trait QuantityRole: sealed::Sealed + Clone + std::fmt::Debug {}

/// The complete affine role remains dynamically checked in the descriptor.
#[derive(Clone, Copy, Debug)]
pub struct DynamicRole;

/// The checked descriptor denotes an affine point.
#[derive(Clone, Copy, Debug)]
pub struct PointRole;

/// The checked descriptor denotes a difference.
#[derive(Clone, Copy, Debug)]
pub struct DifferenceRole;

impl sealed::Sealed for DynamicRole {}
impl sealed::Sealed for PointRole {}
impl sealed::Sealed for DifferenceRole {}
impl QuantityRole for DynamicRole {}
impl QuantityRole for PointRole {}
impl QuantityRole for DifferenceRole {}

impl<P: Scalar + 'static, R: QuantityRole> Quantity<P, R> {
    /// Check and retain a point role while sharing the original complete binding.
    /// # Errors
    /// Refuses a difference, stale binding or exhausted caller Work.
    pub fn point(
        &self,
        heap: &Heap,
        budget: &mut Budget,
    ) -> Result<Quantity<P, PointRole>, QuantityError<P>> {
        self.check_role(heap, Role::Point, budget)?;
        Ok(Quantity {
            binding: Rc::clone(&self.binding),
            counts: self.counts.clone(),
            role: PhantomData,
        })
    }

    /// Check and retain a difference role while sharing the original complete binding.
    /// # Errors
    /// Refuses a point, stale binding or exhausted caller Work.
    pub fn difference(
        &self,
        heap: &Heap,
        budget: &mut Budget,
    ) -> Result<Quantity<P, DifferenceRole>, QuantityError<P>> {
        self.check_role(heap, Role::Difference, budget)?;
        Ok(Quantity {
            binding: Rc::clone(&self.binding),
            counts: self.counts.clone(),
            role: PhantomData,
        })
    }

    /// Descriptor role checks precede live numerical resolution.
    fn check_role(
        &self,
        heap: &Heap,
        expected: Role,
        budget: &mut Budget,
    ) -> Result<(), QuantityError<P>> {
        charge(budget, 1)?;
        if self.descriptor().role() != expected {
            return Err(UnitError::AffineRole.into());
        }
        self.view(heap)?;
        Ok(())
    }
}
