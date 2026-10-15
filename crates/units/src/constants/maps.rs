// SPDX-License-Identifier: AGPL-3.0-only
//! Exact map construction checks both schemas and resolves source-free reference declarations.

use nrz_cas::Rational;

use super::{CheckedMap, MapError, MapIdentity, MapKind, model::ConstantDeclaration};
use crate::{
    CheckedDeclarations, Conversion, Declaration, Descriptor, NamedReference, Role, UnitError,
};

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
                source: source.clone(),
                target: target.clone(),
                reference: None,
                named_reference: None,
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
        self.registry().validate(value)?;
        if value.kind().count_domain().is_some() {
            return Err(UnitError::CountDomain.into());
        }
        if value.kind().is_relative() {
            return Err(UnitError::RelativeReference.into());
        }
        let reference = positive_reference(self, value, reference)?;
        let named_reference =
            NamedReference::new(reference.key.clone(), &reference.descriptor, true)?;
        let origin = checked_origin(self, value, origin)?;
        let conversion = normalization(value, &reference, origin.as_ref())?;
        Ok(CheckedMap::new(
            MapIdentity {
                snapshot: self.id().clone(),
                kind: MapKind::Normalization,
                source: value.clone(),
                target: Descriptor::ratio(),
                reference: Some(reference),
                named_reference: Some(named_reference),
                origin,
            },
            conversion,
        ))
    }
}

/// Resolve only explicit exact definitions; runtime inputs and derived observables are not constants.
fn constant(owner: &CheckedDeclarations, key: &str) -> Result<ConstantDeclaration, MapError> {
    let Some(Declaration::Constant {
        descriptor, value, ..
    }) = owner.declarations().get(key)
    else {
        return Err(MapError::ConstantDeclaration(key.into()));
    };
    Ok(ConstantDeclaration {
        key: key.into(),
        descriptor: descriptor.clone(),
        value: value.clone(),
    })
}

/// Positivity is exact descriptor arithmetic on the declaration, not a numerical input assumption.
fn positive_reference(
    owner: &CheckedDeclarations,
    value: &Descriptor,
    key: &str,
) -> Result<ConstantDeclaration, MapError> {
    let reference = constant(owner, key)?;
    if reference.descriptor.role() != Role::Difference
        || reference.value.is_zero()
        || reference.value.is_negative()
    {
        return Err(MapError::Reference);
    }
    value.compatible_kind(&reference.descriptor)?;
    Ok(reference)
}

/// The chosen origin is part of identity even if another origin has equal coordinates.
fn checked_origin(
    owner: &CheckedDeclarations,
    value: &Descriptor,
    key: Option<&str>,
) -> Result<Option<ConstantDeclaration>, MapError> {
    match (value.role(), key) {
        (Role::Difference, None) => Ok(None),
        (Role::Point, Some(key)) => {
            let origin = constant(owner, key)?;
            if origin.descriptor.role() != Role::Point {
                return Err(MapError::Origin);
            }
            value.compatible_kind(&origin.descriptor)?;
            Ok(Some(origin))
        }
        _ => Err(MapError::Origin),
    }
}

/// Derive scale and offset once using the existing bounded rational descriptor operations.
fn normalization(
    value: &Descriptor,
    reference: &ConstantDeclaration,
    origin: Option<&ConstantDeclaration>,
) -> Result<Conversion, MapError> {
    let mut ledger = crate::exact::budget()?;
    let magnitude = reference
        .value
        .mul(reference.descriptor.measure().scale(), &mut ledger)?;
    let origin = if let Some(origin) = origin {
        origin
            .value
            .mul(origin.descriptor.measure().scale(), &mut ledger)?
            .add(origin.descriptor.measure().offset(), &mut ledger)?
    } else {
        Rational::zero()
    };
    let scale = value.measure().scale().div(&magnitude, &mut ledger)?;
    let shift = value
        .measure()
        .offset()
        .sub(&origin, &mut ledger)?
        .div(&magnitude, &mut ledger)?;
    Ok(Conversion { scale, shift })
}
