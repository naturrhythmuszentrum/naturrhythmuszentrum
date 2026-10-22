// SPDX-License-Identifier: AGPL-3.0-only
//! Exact measure maps and named normalization resolve through the same immutable definitions.

use super::{CheckedMap, MapCoordinates, MapError, MapIdentity, MapKind};
use crate::{CheckedDeclarations, Descriptor, LimitsOverride};

impl CheckedDeclarations {
    /// Construct an exact measure map tied to this frozen rule and declaration state.
    ///
    /// # Errors
    /// Refuses incompatible dimensions, kinds, roles, registry names or exact budgets.
    pub fn conversion_map(
        &self,
        source: &Descriptor,
        target: &Descriptor,
    ) -> Result<CheckedMap, MapError> {
        self.registry().validate(source)?;
        self.registry().validate(target)?;
        let conversion = source.conversion_to(target)?;
        Ok(CheckedMap::new(
            MapIdentity {
                snapshot: self.id().clone(),
                kind: MapKind::Measure,
                source: MapCoordinates::scalar(source.clone()),
                target: MapCoordinates::scalar(target.clone()),
                reference: None,
                column_reference: None,
                origin: None,
            },
            conversion,
        ))
    }

    /// Normalize by a positive exact declared difference reference and an optional point origin.
    ///
    /// Both named definitions must be `Declaration::Constant`. A point requires a
    /// compatible point origin; a difference forbids one. In coherent coordinates
    /// this map is `(value-origin)/reference`; its input still uses the supplied
    /// descriptor's declared measure. Counts and unnamed relative values refuse.
    ///
    /// # Errors
    /// Refuses missing or nonconstant declarations, incompatible kinds/roles,
    /// nonpositive references and bounded exact arithmetic failures.
    pub fn normalization_map(
        &self,
        value: &Descriptor,
        reference: &str,
        origin: Option<&str>,
    ) -> Result<CheckedMap, MapError> {
        let reference = self.reference_definition(reference)?;
        let origin = origin.map(|key| self.origin_definition(key)).transpose()?;
        self.schema_normalization_map(
            value,
            &reference,
            origin.as_ref(),
            &LimitsOverride::default(),
        )
    }
}
