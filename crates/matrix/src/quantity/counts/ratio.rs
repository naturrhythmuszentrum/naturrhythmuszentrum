// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit ratios retain exact count operands and use the existing rational representation.

use super::{CountError, CountQuantity, charge, envelope, integer};
use crate::quantity::QuantityContext;
use nrz_bindings::readings::algebra::Rational;
use nrz_bindings::{Budget, Descriptor};
use std::rc::Rc;

/// An exact continuous ratio of compatible counts, before numerical representation is chosen.
#[derive(Clone, Debug)]
pub struct ExactCountRatio(Rc<Data>);

/// Bindings-owned rational arithmetic is the sole exact ratio implementation.
#[derive(Debug)]
struct Data {
    /// Checked continuous interpretation returned by the bindings count-ratio rule.
    descriptor: Descriptor,
    /// Existing exact rational representation; no floating approximation has been made.
    value: Rational,
    /// Both original counts remain visible, including the nonzero divisor obligation.
    operands: [CountQuantity; 2],
}

impl ExactCountRatio {
    /// Continuous ratio descriptor, distinct from either discrete counted kind.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.descriptor
    }
    /// Exact quotient, with no implicit choice of floating precision or physical uncertainty.
    #[must_use]
    pub fn exact(&self) -> &Rational {
        &self.0.value
    }
    /// Original ordered numerator and denominator with their exact count evidence.
    #[must_use]
    pub fn operands(&self) -> &[CountQuantity; 2] {
        &self.0.operands
    }
}

impl QuantityContext {
    /// Form an explicit same-kind count ratio through bindings and bounded exact arithmetic.
    /// # Errors
    /// Refuses incompatible counted meanings, a zero original divisor and exhausted resources.
    pub fn count_ratio(
        &self,
        left: &CountQuantity,
        right: &CountQuantity,
        budget: &mut Budget,
    ) -> Result<ExactCountRatio, CountError> {
        charge(budget)?;
        let descriptor = self
            .declarations
            .registry()
            .count_ratio(left.descriptor(), right.descriptor())?;
        if right.value() == 0 {
            return Err(CountError::ZeroDivisor);
        }
        let numerator = integer(left.value(), budget)?;
        let denominator = integer(right.value(), budget)?;
        let value = numerator.div(&denominator, budget)?;
        let parents = left
            .0
            .bytes
            .checked_add(right.0.bytes)
            .ok_or(CountError::Capacity)?;
        let depth = left
            .0
            .depth
            .max(right.0.depth)
            .checked_add(1)
            .ok_or(CountError::Capacity)?;
        let components = value.component_bits();
        let own = u64::try_from(size_of::<Data>() + 2 * size_of::<usize>())
            .map_err(|_| CountError::Capacity)?;
        let bytes = parents
            .checked_add(own)
            .and_then(|bytes| bytes.checked_add(components.0.div_ceil(8)))
            .and_then(|bytes| bytes.checked_add(components.1.div_ceil(8)))
            .ok_or(CountError::Capacity)?;
        envelope(bytes, depth, budget)?;
        Ok(ExactCountRatio(Rc::new(Data {
            descriptor,
            value,
            operands: [left.clone(), right.clone()],
        })))
    }
}
