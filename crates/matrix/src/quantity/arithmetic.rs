// SPDX-License-Identifier: AGPL-3.0-only
//! Bindings admits affine roles before an existing sum law reads coordinates.

use super::{
    DifferenceRole, PointRole, Quantity, QuantityContext, QuantityError, QuantityRole, charge,
};
use crate::{Heap, OperationDraft, OperationError};
use nrz_bindings::{BinaryOp, Budget, Descriptor, Role};
use nrz_scalar::Scalar;
use std::marker::PhantomData;

impl QuantityContext {
    /// Add quantities after bindings checks dimensions, kinds and the affine role table.
    /// # Errors
    /// Refuses point plus point, unequal extents, incompatible measures and stale evidence.
    pub fn add<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        left: &Quantity<P>,
        right: &Quantity<P>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        self.sum(heap, left, right, BinaryOp::Add, budget)
    }

    /// Subtract quantities, including point minus point producing a difference.
    /// # Errors
    /// Refuses difference minus point and every incompatible or stale input contract.
    pub fn subtract<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        left: &Quantity<P>,
        right: &Quantity<P>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        self.sum(heap, left, right, BinaryOp::Subtract, budget)
    }

    /// Subtract two checked points and retain a statically checked difference result.
    /// # Errors
    /// Refuses incompatible measures, kinds, extents, versions or numerical execution.
    pub fn point_difference<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        left: &Quantity<P, PointRole>,
        right: &Quantity<P, PointRole>,
        budget: &mut Budget,
    ) -> Result<Quantity<P, DifferenceRole>, QuantityError<P>> {
        let result = self.sum(heap, left, right, BinaryOp::Subtract, budget)?;
        Ok(Quantity {
            binding: result.binding,
            counts: result.counts,
            role: PhantomData,
        })
    }

    /// Translate a checked point by a compatible difference without forgetting its origin.
    /// # Errors
    /// Refuses incompatible measures, kinds, extents, versions or numerical execution.
    pub fn translate<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        point: &Quantity<P, PointRole>,
        difference: &Quantity<P, DifferenceRole>,
        budget: &mut Budget,
    ) -> Result<Quantity<P, PointRole>, QuantityError<P>> {
        let result = self.sum(heap, point, difference, BinaryOp::Add, budget)?;
        Ok(Quantity {
            binding: result.binding,
            counts: result.counts,
            role: PhantomData,
        })
    }

    /// Derive the coordinate meaning through bindings before borrowing numerical entries.
    fn sum<P: Scalar + 'static, L: QuantityRole, R: QuantityRole>(
        &self,
        heap: &mut Heap,
        left: &Quantity<P, L>,
        right: &Quantity<P, R>,
        operation: BinaryOp,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        charge(budget, 1)?;
        let derived = self.declarations.registry().binary(
            operation,
            left.descriptor(),
            right.descriptor(),
        )?;
        self.check_descriptor(&derived)?;
        let output = common_measure(left.descriptor(), right.descriptor(), &derived)
            .ok_or(QuantityError::CommonMeasure)?;
        let extent = left.view(heap)?.region().extent();
        if extent != right.view(heap)?.region().extent() {
            return Err(crate::HeapError::LengthMismatch.into());
        }
        charge(budget, extent.entries())?;
        let tangent = if let Some(shape) = heap.quantity_shape(right.binding().handle())? {
            shape.clone()
        } else {
            self.input_shape(heap, left)?
        };
        let left_shape = heap
            .quantity_shape(left.binding().handle())?
            .unwrap_or(&tangent)
            .clone();
        let right_shape = heap
            .quantity_shape(right.binding().handle())?
            .unwrap_or(&tangent)
            .clone();
        for shape in [&left_shape, &right_shape] {
            shape
                .declarations
                .id()
                .validate(self.declarations.id())
                .map_err(nrz_bindings::MapError::from)
                .map_err(OperationError::from)?;
        }
        let draft = match operation {
            BinaryOp::Add => OperationDraft::add(left_shape, right_shape, self.shapes)?,
            BinaryOp::Subtract => OperationDraft::subtract(left_shape, right_shape, self.shapes)?,
            BinaryOp::Multiply | BinaryOp::Divide => {
                return Err(OperationError::UnsupportedShape.into());
            }
        };
        let checked = draft
            .quantity_sum(left.descriptor(), right.descriptor(), &output)?
            .check(self.execution)?;
        let counts = super::lineage::merge(&left.counts, &right.counts, budget)?;
        let mut result = self.execute(
            heap,
            checked,
            vec![left.binding().clone(), right.binding().clone()],
            budget,
        )?;
        result.counts = counts;
        Ok(result)
    }
}

/// Equal coordinate scales and compatible origins let the existing sum law execute directly.
fn common_measure(
    left: &Descriptor,
    right: &Descriptor,
    derived: &Descriptor,
) -> Option<Descriptor> {
    if left.measure().scale() != right.measure().scale() {
        return None;
    }
    let left_point = left.role() == Role::Point;
    let right_point = right.role() == Role::Point;
    let both_points = left_point && right_point;
    if both_points && left.measure().offset() != right.measure().offset() {
        return None;
    }
    if derived.role() == Role::Point {
        Some(if left.role() == Role::Point {
            left.clone()
        } else {
            right.clone()
        })
    } else {
        Some(left.difference())
    }
}
