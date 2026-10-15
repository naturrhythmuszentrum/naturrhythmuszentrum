// SPDX-License-Identifier: AGPL-3.0-only
//! Map and coefficient identities contain exact definitions, never floating approximations.

use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};

use super::MapError;
use crate::{CheckedDeclarations, Conversion, Descriptor, NamedReference, SnapshotId};

/// Mathematical interpretation and direction of a checked affine coordinate map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapKind {
    /// Conversion between compatible measures of one quantity kind and role.
    Measure,
    /// Remove a named origin when required, then divide by a positive exact reference.
    Normalization,
    /// Inverse of the retained normalization, restoring the same reference and origin.
    Restoration,
}

/// An exact declaration retained in a map's identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConstantDeclaration {
    /// Declaration key within the frozen semantic state.
    pub(crate) key: String,
    /// Original descriptor, including any point origin.
    pub(crate) descriptor: Descriptor,
    /// Exact declared descriptor/reference coordinate, never a heap value.
    pub(crate) value: Rational,
}

/// Full immutable map identity for caches keyed additionally by precision and arithmetic revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapIdentity {
    /// Exact frozen declarations and rule revision.
    pub(crate) snapshot: SnapshotId,
    /// Meaning and direction, retained independently of equal coefficients.
    pub(crate) kind: MapKind,
    /// Input coordinate descriptor.
    pub(crate) source: Descriptor,
    /// Output coordinate descriptor.
    pub(crate) target: Descriptor,
    /// Exact positive difference-reference declaration when normalizing.
    pub(crate) reference: Option<ConstantDeclaration>,
    /// Explicit nonzero difference-reference schema checked through the pairing surface.
    pub(crate) named_reference: Option<NamedReference>,
    /// Exact point-origin declaration when normalizing a point.
    pub(crate) origin: Option<ConstantDeclaration>,
}

impl MapIdentity {
    /// Semantic identity required before reusing lowered coefficients.
    #[must_use]
    pub const fn snapshot_id(&self) -> &SnapshotId {
        &self.snapshot
    }

    /// Mathematical purpose and direction.
    #[must_use]
    pub const fn kind(&self) -> MapKind {
        self.kind
    }

    /// Input descriptor of this direction.
    #[must_use]
    pub const fn source(&self) -> &Descriptor {
        &self.source
    }

    /// Output descriptor of this direction.
    #[must_use]
    pub const fn target(&self) -> &Descriptor {
        &self.target
    }

    /// Named exact difference reference, if normalization requires one.
    #[must_use]
    pub const fn reference(&self) -> Option<&NamedReference> {
        self.named_reference.as_ref()
    }

    /// Named point-origin declaration, if this map removes or restores one.
    #[must_use]
    pub fn origin(&self) -> Option<&str> {
        self.origin.as_ref().map(|origin| origin.key.as_str())
    }
}

/// Coefficient role within one exact map, keeping offsets separate from difference scales.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantKind {
    /// Multiplicative coordinate coefficient.
    Scale,
    /// Additive coordinate coefficient; zero for difference maps.
    Shift,
}

/// A private-construction exact descriptor coefficient with its full map identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorConstant {
    /// Exact map and declaration identity; equal spellings alone do not identify constants.
    pub(crate) map: MapIdentity,
    /// Scale versus shift within that map.
    pub(crate) kind: ConstantKind,
    /// CAS rational definition of a descriptor constant only.
    pub(crate) value: Rational,
}

impl DescriptorConstant {
    /// Exact map identity for lowering once per map, format and arithmetic revision.
    #[must_use]
    pub const fn map_id(&self) -> &MapIdentity {
        &self.map
    }

    /// Coefficient position within the map.
    #[must_use]
    pub const fn kind(&self) -> ConstantKind {
        self.kind
    }

    /// Export a bounded exact integer or literal-fraction spelling for a numerical decoder.
    ///
    /// # Errors
    /// Returns the CAS's exact arithmetic, output-byte or formatting-work refusal.
    /// No approximate number is produced and no stored coordinate is accepted.
    pub fn export(&self, limits: &LimitsOverride) -> Result<String, MapError> {
        let mut ledger = Budget::new(Limits::default().resolve(limits)?)?;
        Ok(self.value.exact_string(&mut ledger)?)
    }
}

/// A checked exact affine map `x_out = scale*x_in + shift`, without an evaluator.
#[doc = include_str!("../../tests/conformance/constants.inc")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMap {
    /// Complete map definition and direction.
    pub(crate) id: MapIdentity,
    /// Exact multiplicative coefficient.
    pub(crate) scale: DescriptorConstant,
    /// Exact additive coefficient.
    pub(crate) shift: DescriptorConstant,
}

impl CheckedMap {
    /// Construct only after descriptor and reference checks have succeeded.
    pub(crate) fn new(id: MapIdentity, conversion: Conversion) -> Self {
        let scale = DescriptorConstant {
            map: id.clone(),
            kind: ConstantKind::Scale,
            value: conversion.scale,
        };
        let shift = DescriptorConstant {
            map: id.clone(),
            kind: ConstantKind::Shift,
            value: conversion.shift,
        };
        Self { id, scale, shift }
    }

    /// Exact map definition; precision is a separate numerical lowering key.
    #[must_use]
    pub const fn id(&self) -> &MapIdentity {
        &self.id
    }

    /// Exact multiplicative coefficient with map identity.
    #[must_use]
    pub const fn scale(&self) -> &DescriptorConstant {
        &self.scale
    }

    /// Exact additive coefficient with map identity.
    #[must_use]
    pub const fn shift(&self) -> &DescriptorConstant {
        &self.shift
    }

    /// Validate reuse against a frozen declaration owner.
    ///
    /// # Errors
    /// Refuses a foreign context or a different semantic revision.
    pub fn validate_snapshot(&self, declarations: &CheckedDeclarations) -> Result<(), MapError> {
        Ok(self.id.snapshot.validate(declarations.id())?)
    }

    /// Reverse the exact map, retaining reference and origin identity.
    ///
    /// # Errors
    /// Refuses bounded exact descriptor arithmetic failure.
    pub fn inverse(&self) -> Result<Self, MapError> {
        let mut ledger = crate::exact::budget()?;
        let scale = Rational::one().div(&self.scale.value, &mut ledger)?;
        let shift = self
            .shift
            .value
            .negated(&mut ledger)?
            .div(&self.scale.value, &mut ledger)?;
        let mut id = self.id.clone();
        std::mem::swap(&mut id.source, &mut id.target);
        id.kind = match id.kind {
            MapKind::Measure => MapKind::Measure,
            MapKind::Normalization => MapKind::Restoration,
            MapKind::Restoration => MapKind::Normalization,
        };
        Ok(Self::new(id, Conversion { scale, shift }))
    }
}
