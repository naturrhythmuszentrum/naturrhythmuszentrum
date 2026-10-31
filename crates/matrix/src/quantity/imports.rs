// SPDX-License-Identifier: AGPL-3.0-only
//! Quantity imports retain explicit coverage and the existing complete symbol inventory.

use super::{Quantity, QuantityContext, QuantityError, allocation, charge};
use crate::{Coverage, Heap};
use nrz_bindings::{BindingSymbols, Budget};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// Borrowed numerical channels; the existing heap owns their sole stored representation.
#[derive(Clone, Copy, Debug)]
pub struct QuantityInput<'a, P: Scalar> {
    /// Coordinates in the checked symbol's declared measure.
    centers: &'a [P],
    /// Optional total componentwise bounds, including numerical error when present.
    total: Option<&'a [P]>,
    /// Optional numerical-only import error with its own target symbol.
    error: Option<&'a [P]>,
    /// Explicit physical-input coverage; no exactness is inferred from absent uncertainty.
    coverage: Coverage,
}

impl<'a, P: Scalar> QuantityInput<'a, P> {
    /// Declare represented coordinates without asserting physical exactness.
    #[must_use]
    pub const fn represented(centers: &'a [P]) -> Self {
        Self {
            centers,
            total: None,
            error: None,
            coverage: Coverage::RepresentedInputs,
        }
    }

    /// Declare complete deterministic input bounds and an optional overlapping numerical error.
    #[must_use]
    pub const fn bounded(centers: &'a [P], total: &'a [P], error: Option<&'a [P]>) -> Self {
        Self {
            centers,
            total: Some(total),
            error,
            coverage: Coverage::DeclaredBounds,
        }
    }
}

impl QuantityContext {
    /// Import checked declared coordinates, including compatible presentation measures.
    ///
    /// The supplied symbols must already declare precisely the supplied companion inventory.
    /// # Errors
    /// Refuses counts, derived result symbols, invalid payloads, missing channels or exhausted resources.
    pub fn import<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        symbols: &BindingSymbols,
        input: QuantityInput<'_, P>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        charge(budget, 1)?;
        self.check_descriptor(symbols.value().descriptor())?;
        symbols.check_import().map_err(crate::HeapError::from)?;
        let channels = 1 + usize::from(input.total.is_some()) + usize::from(input.error.is_some());
        let work = input
            .centers
            .len()
            .checked_mul(channels)
            .ok_or(crate::HeapError::Capacity)?;
        charge(budget, work)?;
        allocation::<P>(budget)?;
        let binding = heap.import_quantity_symbols(
            symbols,
            input.centers,
            input.total,
            input.error,
            input.coverage,
        )?;
        Ok(Quantity {
            binding: Rc::new(binding),
            counts: None,
            role: PhantomData,
        })
    }
}
