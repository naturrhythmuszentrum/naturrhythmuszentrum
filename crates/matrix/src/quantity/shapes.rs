// SPDX-License-Identifier: AGPL-3.0-only
//! Homogeneous array axes share the same frozen authority and explicit logical order.

use super::{Quantity, QuantityContext, QuantityRole};
use crate::{
    Axis, AxisEntry, BlockDraft, CheckedShape, EntryDraft, Extent, Partition, ShapeDraft,
    StructureScope,
};
use nrz_bindings::Descriptor;
use std::rc::Rc;

impl QuantityContext {
    /// Published results retain their exact ordered axes; imports receive a first shape.
    pub(super) fn input_shape<P: nrz_scalar::Scalar + 'static, R: QuantityRole>(
        &self,
        heap: &crate::Heap,
        input: &Quantity<P, R>,
    ) -> Result<CheckedShape, crate::OperationError> {
        let extent = input.view(heap)?.region().extent();
        if let Some(shape) = heap.quantity_shape(input.binding().handle())? {
            shape
                .declarations
                .id()
                .validate(self.declarations.id())
                .map_err(nrz_bindings::MapError::from)?;
            if shape.extent() != extent {
                return Err(crate::OperationError::UnsupportedShape);
            }
            return Ok(shape.clone());
        }
        self.shape(input.descriptor(), extent)
    }

    /// Admit tangent-coordinate shape metadata without inspecting any floating coordinate.
    pub(super) fn shape(
        &self,
        quantity: &Descriptor,
        extent: Extent,
    ) -> Result<CheckedShape, crate::OperationError> {
        self.shapes.extent(extent.rows(), extent.columns())?;
        let tangent = quantity.difference().coherent();
        let domain = self.axis(extent.columns(), &Descriptor::ratio())?;
        let codomain = self.axis(extent.rows(), &tangent)?;
        let blocks = if extent.entries() == 0 {
            Vec::new()
        } else {
            vec![BlockDraft::new(
                0..extent.rows(),
                0..extent.columns(),
                vec![EntryDraft::Declared(tangent); extent.entries()],
            )]
        };
        Ok(
            ShapeDraft::new(domain, codomain, blocks, StructureScope::WholeFamily)
                .check(Rc::clone(&self.declarations), self.shapes)?,
        )
    }

    /// Ordered coordinate names are structural metadata, never physical counts.
    fn axis(&self, length: usize, quantity: &Descriptor) -> Result<Axis, crate::ShapeError> {
        let entries = (0..length)
            .map(|index| AxisEntry::new(format!("quantity-coordinate-{index}"), quantity.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        Axis::new(entries, Partition::whole(length, self.shapes)?, self.shapes)
    }
}
