// SPDX-License-Identifier: AGPL-3.0-only
//! Exact measure and origin maps select existing matrix conversion transactions.

use super::{DifferenceRole, Quantity, QuantityContext, QuantityError, QuantityRole, charge};
use crate::{Binding, ConversionContract, ConversionMode, Heap, OperationDraft, PrecisionContract};
use nrz_bindings::{Budget, CheckedMap, Descriptor};
use nrz_scalar::{LiteralLimits, Scalar};
use std::{marker::PhantomData, rc::Rc};

/// A normalized quantity and the identical checked map required to restore its origin and reference.
#[derive(Clone, Debug)]
pub struct NormalizedQuantity<P: Scalar> {
    /// Existing normalized ratio regions and their complete companions.
    value: Quantity<P, DifferenceRole>,
    /// Original source interpretation, exact reference/origin identities and inverse relation.
    map: Rc<CheckedMap>,
}

impl<P: Scalar> NormalizedQuantity<P> {
    /// Normalized coordinates remain a checked difference quantity with intrinsic companions.
    #[must_use]
    pub const fn quantity(&self) -> &Quantity<P, DifferenceRole> {
        &self.value
    }

    /// Exact retained normalization map, including the declaration snapshot and origin.
    #[must_use]
    pub fn map(&self) -> &CheckedMap {
        &self.map
    }
}

impl QuantityContext {
    /// Convert compatible coordinates without changing kind or affine role.
    /// # Errors
    /// Refuses incompatible descriptors, stale inputs, unsupported constants or companion transport.
    pub fn convert<P: Scalar + 'static, R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<P, R>,
        target: &Descriptor,
        budget: &mut Budget,
    ) -> Result<Quantity<P, R>, QuantityError<P>> {
        charge(budget, 1)?;
        let map = self
            .declarations
            .conversion_map(input.descriptor(), target)
            .map_err(crate::ConversionError::from)
            .map_err(crate::OperationError::from)?;
        let output = self.map(heap, input, map, budget)?;
        Ok(Quantity {
            binding: output.binding,
            counts: input.counts.clone(),
            role: PhantomData,
        })
    }

    /// Remove a checked declared origin and divide by an explicit positive difference reference.
    /// # Errors
    /// Refuses missing, nonconstant or incompatible references/origins and failed conversions.
    pub fn normalize<P: Scalar + 'static, R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<P, R>,
        reference: &str,
        origin: Option<&str>,
        budget: &mut Budget,
    ) -> Result<NormalizedQuantity<P>, QuantityError<P>> {
        charge(budget, 1)?;
        let map = self
            .declarations
            .normalization_map(input.descriptor(), reference, origin)
            .map_err(crate::ConversionError::from)
            .map_err(crate::OperationError::from)?;
        let retained = Rc::new(map.clone());
        let value = self.map(heap, input, map, budget)?;
        Ok(NormalizedQuantity {
            value: Quantity {
                binding: value.binding,
                counts: value.counts,
                role: PhantomData,
            },
            map: retained,
        })
    }

    /// Restore through the exact inverse of the retained reference and origin map.
    /// # Errors
    /// Refuses foreign declaration snapshots, stale inputs and unavailable conversion evidence.
    pub fn restore<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        input: &NormalizedQuantity<P>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        charge(budget, 1)?;
        let inverse = input
            .map
            .inverse()
            .map_err(crate::ConversionError::from)
            .map_err(crate::OperationError::from)?;
        self.map(heap, &input.value, inverse, budget)
    }

    /// Widen coordinates while preserving the original numerical-error target and every companion.
    /// # Errors
    /// Refuses stale bindings, invalid companions or exhausted resources.
    pub fn widen<R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<f32, R>,
        budget: &mut Budget,
    ) -> Result<Quantity<f64, R>, QuantityError<f32>> {
        self.precision(heap, input, budget)
    }

    /// Narrow coordinates with outward representation error and complete companion transport.
    /// # Errors
    /// Refuses overflow, stale bindings, invalid companions or exhausted resources.
    pub fn narrow<R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<f64, R>,
        budget: &mut Budget,
    ) -> Result<Quantity<f32, R>, QuantityError<f64>> {
        self.precision(heap, input, budget)
    }

    /// Check source and destination tangent shapes, then execute the existing affine law.
    fn map<P: Scalar + 'static, R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<P, R>,
        map: CheckedMap,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        self.check_descriptor(input.descriptor())?;
        map.validate_snapshot(&self.declarations)
            .map_err(crate::OperationError::from)?;
        let target = map
            .id()
            .target()
            .descriptor()
            .ok_or(crate::OperationError::UnsupportedShape)?;
        let extent = input.view(heap)?.region().extent();
        charge(budget, extent.entries())?;
        let source_shape = self.input_shape(heap, input)?;
        let target_shape =
            if input.descriptor().difference().coherent() == target.difference().coherent() {
                source_shape.clone()
            } else {
                self.shape(target, extent)?
            };
        let contract = ConversionContract::new(
            source_shape,
            target_shape,
            map,
            LiteralLimits::default(),
            ConversionMode::Deterministic,
        )?;
        let operation = OperationDraft::convert(vec![contract]).check(self.execution)?;
        let mut output = self.execute(heap, operation, vec![input.binding().clone()], budget)?;
        output.counts.clone_from(&input.counts);
        Ok(output)
    }

    /// Keep distinct source/target scalar types while reusing the same precision transaction.
    fn precision<P: Scalar + 'static, Q: Scalar + 'static, R: QuantityRole>(
        &self,
        heap: &mut Heap,
        input: &Quantity<P, R>,
        budget: &mut Budget,
    ) -> Result<Quantity<Q, R>, QuantityError<P>> {
        charge(budget, 1)?;
        self.check_descriptor(input.descriptor())?;
        let extent = input.view(heap)?.region().extent();
        charge(budget, extent.entries())?;
        let shape = self.input_shape(heap, input)?;
        let contract = PrecisionContract::new(shape, input.descriptor())?;
        let operation = OperationDraft::precision(vec![contract], P::PRECISION, Q::PRECISION)
            .check(self.execution)?;
        let symbols = Self::outputs(&operation, budget)?;
        let binding =
            Binding::convert_precision(operation, vec![input.binding().handle()], self.execution);
        let output = Self::publish(heap, binding, symbols, budget)?;
        Ok(Quantity {
            binding: output.binding,
            counts: input.counts.clone(),
            role: PhantomData,
        })
    }
}
