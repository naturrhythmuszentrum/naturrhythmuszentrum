// SPDX-License-Identifier: AGPL-3.0-only
//! Shared descriptor lowering survives cloned contracts and repeated executions.

use super::ConversionError;
use crate::coordinate_numeric::LoweredMap;
use nrz_scalar::{ArithmeticContext, LiteralLimits, Precision, Scalar};
use nrz_units::CheckedMap;
use std::any::Any;

/// Two sealed formats, each retaining its own coefficient evaluation bounds.
#[derive(Clone, Debug)]
enum Lowered {
    /// Binary32 lowering of the exact descriptor map.
    Single(LoweredMap<f32>),
    /// Binary64 lowering of the same exact definition.
    Double(LoweredMap<f64>),
}

/// Bounded per-definition cache, shared with every retained clone of its contract.
#[derive(Debug, Default)]
pub(crate) struct ConstantCache {
    /// At most one lowering per supported precision and current arithmetic revision.
    entries: Vec<Lowered>,
}

impl ConstantCache {
    /// Reuse the original lowering or perform the one checked export for this format.
    pub(crate) fn get<P: Scalar + 'static>(
        &mut self,
        map: &CheckedMap,
        limits: LiteralLimits,
        context: &ArithmeticContext,
    ) -> Result<LoweredMap<P>, ConversionError> {
        for entry in &self.entries {
            let erased: &dyn Any = match entry {
                Lowered::Single(value) => value,
                Lowered::Double(value) => value,
            };
            if let Some(value) = erased.downcast_ref::<LoweredMap<P>>()
                && value.revision == context.revision()
            {
                return Ok(value.clone());
            }
        }
        let lowered = LoweredMap::<P>::new(map, limits, context)?;
        let erased = &lowered as &dyn Any;
        let entry = match P::PRECISION {
            Precision::F32 => Lowered::Single(
                erased
                    .downcast_ref::<LoweredMap<f32>>()
                    .ok_or(ConversionError::Contract)?
                    .clone(),
            ),
            Precision::F64 => Lowered::Double(
                erased
                    .downcast_ref::<LoweredMap<f64>>()
                    .ok_or(ConversionError::Contract)?
                    .clone(),
            ),
        };
        if self.entries.len() >= 2 {
            return Err(ConversionError::ResourceLimit);
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| ConversionError::ResourceLimit)?;
        self.entries.push(entry);
        Ok(lowered)
    }
}
