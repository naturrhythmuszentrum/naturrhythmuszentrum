// SPDX-License-Identifier: AGPL-3.0-only
//! Checked schema exports use exact descriptor arithmetic and never evaluate a value operand.

use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};

use super::definitions::check_continuous;
use super::{
    CheckedMap, MapCoordinates, MapError, MapIdentity, MapKind, OriginDefinition,
    ReferenceDefinition,
};
use crate::{
    CheckedDeclarations, Conversion, Descriptor, Kind, Measure, PairingError, PairingKind,
    PairingWitness, Role,
};

impl CheckedDeclarations {
    /// Export a scalar normalization from immutable exact reference and origin definitions.
    ///
    /// Definitions may come from this frozen owner's named constants or independent
    /// source-free schemas. This method never changes the owner or its snapshot ID.
    /// Its result is identical to `Self::normalization_map` for the same named definitions.
    ///
    /// # Errors
    /// Refuses foreign declarations, incompatible kinds/roles, missing or extra origins,
    /// counts, relative kinds and exhausted exact arithmetic limits.
    pub fn schema_normalization_map(
        &self,
        value: &Descriptor,
        reference: &ReferenceDefinition,
        origin: Option<&OriginDefinition>,
        limits: &LimitsOverride,
    ) -> Result<CheckedMap, MapError> {
        let mut ledger = descriptor_budget(limits)?;
        self.registry().validate(value)?;
        check_continuous(value)?;
        reference.validate(self)?;
        value.compatible_kind(reference.descriptor())?;
        check_origin(self, value, origin)?;
        let magnitude = reference.coherent(&mut ledger)?;
        let conversion = normalization(value.measure(), &magnitude, origin, &mut ledger)?;
        Ok(checked_map(
            self,
            MapCoordinates::scalar(value.clone()),
            reference,
            None,
            origin,
            conversion,
        ))
    }

    /// Export a coherent physical entry's exact row-reference over column-reference normalization.
    ///
    /// `quotient` is the typed sensitivity pairing retained by a matrix entry schema;
    /// its two original descriptors must exactly match the ordered definitions. Each
    /// definition's exact coordinate participates in the quotient. An optional point
    /// origin requires ratio columns and a compatible row kind. Without one, the entry
    /// is a difference. Origins never enter the corresponding difference map.
    ///
    /// The inverse's shift exports the coherent origin for an affine RHS correction;
    /// matrix retains the correction's sign, row/column placement and operator product.
    /// No stored coordinate, correction product or numerical evaluation enters units.
    ///
    /// # Errors
    /// Refuses non-sensitivity pairings, mismatched definitions, foreign declarations,
    /// unsupported affine columns, incompatible origins and exhausted exact budgets.
    pub fn quotient_normalization_map(
        &self,
        quotient: &PairingWitness,
        row: &ReferenceDefinition,
        column: &ReferenceDefinition,
        origin: Option<&OriginDefinition>,
        limits: &LimitsOverride,
    ) -> Result<CheckedMap, MapError> {
        let mut ledger = descriptor_budget(limits)?;
        check_quotient(self, quotient, row, column)?;
        let role = if let Some(origin) = origin {
            if column.descriptor().kind() != &Kind::Ratio {
                return Err(MapError::Origin);
            }
            origin.validate(self)?;
            row.descriptor().compatible_kind(origin.descriptor())?;
            Role::Point
        } else {
            Role::Difference
        };
        let magnitude = row
            .coherent(&mut ledger)?
            .div(&column.coherent(&mut ledger)?, &mut ledger)?;
        let conversion = normalization(&Measure::coherent(), &magnitude, origin, &mut ledger)?;
        let source = MapCoordinates::formal(quotient.clone(), role);
        Ok(checked_map(
            self,
            source,
            row,
            Some(column),
            origin,
            conversion,
        ))
    }
}

/// One explicit shared ledger bounds all coefficient arithmetic for an exported map.
fn descriptor_budget(limits: &LimitsOverride) -> Result<Budget, MapError> {
    let mut ledger = Budget::new(Limits::default().resolve(limits)?)?;
    ledger.work(1, nrz_cas::Location::new("normalization schema export"))?;
    Ok(ledger)
}

/// Check both ordered reference sides without admitting a named scalar quotient kind.
fn check_quotient(
    owner: &CheckedDeclarations,
    quotient: &PairingWitness,
    row: &ReferenceDefinition,
    column: &ReferenceDefinition,
) -> Result<(), MapError> {
    if quotient.kind() != PairingKind::Sensitivity {
        return Err(PairingError::ExpectedSensitivity.into());
    }
    row.validate(owner)?;
    column.validate(owner)?;
    quotient.validate_targets(row.descriptor(), Some(column.descriptor()))?;
    let mut registry = owner.registry().clone();
    registry.admit(row.descriptor())?;
    registry.admit(column.descriptor())?;
    Ok(())
}

/// A point requires its compatible named origin; a difference cannot translate.
fn check_origin(
    owner: &CheckedDeclarations,
    value: &Descriptor,
    origin: Option<&OriginDefinition>,
) -> Result<(), MapError> {
    match (value.role(), origin) {
        (Role::Difference, None) => Ok(()),
        (Role::Point, Some(origin)) => {
            origin.validate(owner)?;
            Ok(value.compatible_kind(origin.descriptor())?)
        }
        _ => Err(MapError::Origin),
    }
}

/// Derive coefficients from exact definitions; input coordinates are never accepted here.
fn normalization(
    measure: &Measure,
    magnitude: &Rational,
    origin: Option<&OriginDefinition>,
    ledger: &mut Budget,
) -> Result<Conversion, MapError> {
    let origin = origin.map_or_else(|| Ok(Rational::zero()), |origin| origin.coherent(ledger))?;
    let scale = measure.scale().div(magnitude, ledger)?;
    let shift = measure
        .offset()
        .sub(&origin, ledger)?
        .div(magnitude, ledger)?;
    Ok(Conversion { scale, shift })
}

/// Identity includes the snapshot, ordered exact definitions and formal coordinate contract.
fn checked_map(
    owner: &CheckedDeclarations,
    source: MapCoordinates,
    reference: &ReferenceDefinition,
    column: Option<&ReferenceDefinition>,
    origin: Option<&OriginDefinition>,
    conversion: Conversion,
) -> CheckedMap {
    CheckedMap::new(
        MapIdentity {
            snapshot: owner.id().clone(),
            kind: MapKind::Normalization,
            source,
            target: MapCoordinates::scalar(Descriptor::ratio()),
            reference: Some(reference.clone()),
            column_reference: column.cloned(),
            origin: origin.cloned(),
        },
        conversion,
    )
}
