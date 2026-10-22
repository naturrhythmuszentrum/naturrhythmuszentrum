// SPDX-License-Identifier: AGPL-3.0-only
//! Homogeneity is a closed bindings predicate, independent of storage.

use super::{AdmissionError, BindingSymbols};
use crate::{Descriptor, Role};

/// Checked coherent homogeneous difference recipe; no heap authority is implied.
#[derive(Clone, Debug)]
pub struct HomogeneousRecipe {
    /// Complete physical kind, role and coherent measure.
    quantity: Descriptor,
}

impl HomogeneousRecipe {
    /// Check every original entry, including typed zeros and formal refusals.
    ///
    /// Empty rectangles have ratio meaning. The supplied entry limit bounds the
    /// complete traversal; callers retain their independently checked shape.
    /// # Errors
    /// Refuses points, formal entries, unequal kinds and an exhausted entry limit.
    pub fn check<'a>(
        entries: impl ExactSizeIterator<Item = Option<&'a Descriptor>>,
        limit: usize,
    ) -> Result<Self, AdmissionError> {
        if entries.len() > limit {
            return Err(AdmissionError::ResourceLimit);
        }
        let mut quantity = None;
        for (index, entry) in entries.enumerate() {
            if index >= limit {
                return Err(AdmissionError::ResourceLimit);
            }
            let value = entry.ok_or(AdmissionError::UnsupportedRecipe)?.coherent();
            if value.role() != Role::Difference
                || quantity.as_ref().is_some_and(|previous| previous != &value)
            {
                return Err(AdmissionError::UnsupportedRecipe);
            }
            quantity = Some(value);
        }
        Ok(Self {
            quantity: quantity.unwrap_or_else(Descriptor::ratio),
        })
    }

    /// Exact admitted entry meaning, including kind rather than dimensions alone.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.quantity
    }

    /// Check an actual declaration against the closed coherent recipe.
    /// # Errors
    /// Refuses another kind, measure or affine role.
    pub fn check_descriptor(&self, actual: &Descriptor) -> Result<(), AdmissionError> {
        if actual == &self.quantity {
            Ok(())
        } else {
            Err(AdmissionError::ValueMismatch)
        }
    }

    /// Check all declared input and planned result symbols before numerical execution.
    /// # Errors
    /// Refuses mismatched values, companions or a result from another source.
    pub fn check_copy(
        &self,
        input: &BindingSymbols,
        result: &BindingSymbols,
    ) -> Result<CopyWitness, AdmissionError> {
        self.check_descriptor(input.value().descriptor())?;
        self.check_descriptor(result.value().descriptor())?;
        input.validate()?;
        result.validate()?;
        result.check_source(input)?;
        Ok(CopyWitness {
            input: input.clone(),
            result: result.clone(),
        })
    }
}

/// Checked exact correspondence for a selected homogeneous copy and its companions.
#[derive(Clone, Debug)]
pub struct CopyWitness {
    /// Original value and companions, with their immutable declared identities.
    input: BindingSymbols,
    /// Fresh planned result and companion inventory.
    result: BindingSymbols,
}

impl CopyWitness {
    /// Checked original symbol inventory.
    #[must_use]
    pub const fn input(&self) -> &BindingSymbols {
        &self.input
    }
    /// Exact planned output inventory that the numerical owner must publish together.
    #[must_use]
    pub const fn result(&self) -> &BindingSymbols {
        &self.result
    }
}
