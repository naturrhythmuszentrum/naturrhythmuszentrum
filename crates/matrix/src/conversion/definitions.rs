// SPDX-License-Identifier: AGPL-3.0-only
//! Preserve shape definition identity while exporting independent units schema constants.

use crate::{CoordinateMap, OperationError, PointOrigin, ReferenceScale};
use nrz_units::{OriginDefinition, ReferenceDefinition};

/// One ordered axis's immutable exact definitions.
pub(super) struct AxisDefinitions {
    /// Positive source-free row or column scales.
    pub(super) references: Vec<ReferenceDefinition>,
    /// Origin only for a point coordinate.
    pub(super) origins: Vec<Option<OriginDefinition>>,
}

/// Cloned H definitions share exported identity throughout one complete schema bundle.
#[derive(Default)]
pub(super) struct DefinitionPool {
    /// Structural equality includes H's independent definition identity.
    references: Vec<(ReferenceScale, ReferenceDefinition)>,
    /// Equal coordinates alone never identify distinct point origins.
    origins: Vec<(PointOrigin, OriginDefinition)>,
}

impl DefinitionPool {
    /// Export source-free definitions, never interpret a human label as a declaration key.
    pub(super) fn axis(
        &mut self,
        coordinates: &CoordinateMap,
    ) -> Result<AxisDefinitions, OperationError> {
        let mut references = crate::binding::buffer(coordinates.references().len())?;
        let mut origins = crate::binding::buffer(coordinates.origins().len())?;
        for reference in coordinates.references() {
            let definition = if let Some((_, retained)) = self
                .references
                .iter()
                .find(|(original, _)| original == reference)
            {
                retained.clone()
            } else {
                let definition = ReferenceDefinition::unit(
                    reference.reference().name(),
                    reference.descriptor(),
                )?;
                self.references
                    .push((reference.clone(), definition.clone()));
                definition
            };
            references.push(definition);
        }
        for origin in coordinates.origins() {
            origins.push(
                origin
                    .as_ref()
                    .map(|origin| self.origin(origin))
                    .transpose()?,
            );
        }
        Ok(AxisDefinitions {
            references,
            origins,
        })
    }

    /// Reuse the units identity exactly when the original point definition is reused.
    fn origin(&mut self, original: &PointOrigin) -> Result<OriginDefinition, OperationError> {
        if let Some((_, retained)) = self
            .origins
            .iter()
            .find(|(candidate, _)| candidate == original)
        {
            return Ok(retained.clone());
        }
        let definition = OriginDefinition::new(original.name(), original.descriptor())?;
        self.origins.push((original.clone(), definition.clone()));
        Ok(definition)
    }
}
