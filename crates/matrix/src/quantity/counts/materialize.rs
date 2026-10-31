// SPDX-License-Identifier: AGPL-3.0-only
//! Exact ratio imports use the established scalar decoder and complete heap import transaction.

use super::ExactCountRatio;
use crate::Heap;
use crate::quantity::{Quantity, QuantityContext, QuantityError, QuantityInput, lineage};
use nrz_bindings::Budget;
use nrz_bindings::readings::algebra::Location;
use nrz_scalar::{ArithmeticContext, DecodedLiteral, LiteralLimits, NumericLiteral, Scalar};

impl QuantityContext {
    /// Represent an exact count ratio in an explicitly chosen sealed scalar format.
    ///
    /// The center is the lower endpoint of the existing literal enclosure. Its outward
    /// displacement is both the complete total and the numerical-only error, counted once.
    /// Original exact operands and qualified source evidence remain attached to the quantity.
    /// # Errors
    /// Refuses exhausted caller resources, unavailable numerical enclosures or heap capacity,
    /// publishing neither a center nor a companion prefix on failure.
    pub fn materialize_count_ratio<P: Scalar + 'static>(
        &self,
        heap: &mut Heap,
        ratio: &ExactCountRatio,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        let counts = lineage::single(ratio, budget)?;
        budget.with_retained_payload(
            ratio.retained_payload_bytes(),
            Location::new("materialized count ratio"),
            |budget| {
                let decoded = decode::<P>(ratio, budget)?;
                let center = [decoded.center().get()];
                let error = [decoded.displacement().get()];
                let mut quantity = self.with_output_symbols(
                    ratio.descriptor(),
                    [1, 1],
                    budget,
                    |symbols, budget| {
                        self.import(
                            heap,
                            symbols,
                            QuantityInput::bounded(&center, &error, Some(&error)),
                            budget,
                        )
                    },
                )?;
                quantity.counts = counts;
                Ok(quantity)
            },
        )
    }
}

/// A fraction spelling fixes downward enclosure selection, including exact integer ratios.
fn decode<P: Scalar>(
    ratio: &ExactCountRatio,
    budget: &mut Budget,
) -> Result<DecodedLiteral<P>, QuantityError<P>> {
    let mut text = ratio.exact().exact_string(budget)?;
    if ratio.exact().is_integer() {
        text.push_str("/1");
    }
    let remaining = budget.limits().work.saturating_sub(budget.usage().work);
    let policy = LiteralLimits::default();
    let limits = LiteralLimits::new(
        policy.max_digits(),
        policy.max_exponent(),
        policy.max_work().min(remaining),
    );
    let context = ArithmeticContext::new()?;
    let literal = NumericLiteral::new(&text, limits)?;
    let mut used = 0;
    let result = literal.decode_with_work(&context, &mut used);
    budget.work(used, Location::new("count ratio literal"))?;
    Ok(result?)
}
