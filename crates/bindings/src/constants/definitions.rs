// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable source-free schema definitions retain identity without changing a frozen owner.

use std::rc::Rc;

use nrz_cas::{Rational, limits::Budget};

use super::MapError;
use crate::{
    CheckedDeclarations, Declaration, Descriptor, NamedReference, Role, Scale, SnapshotId,
    UnitError,
};

/// Independent schema definitions stay distinct even when their complete descriptions coincide.
#[derive(Clone, Debug)]
struct SchemaIdentity(Rc<()>);

impl PartialEq for SchemaIdentity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SchemaIdentity {}

/// A frozen declaration or an independently owned immutable schema definition.
#[derive(Clone, Debug, Eq, PartialEq)]
enum DefinitionIdentity {
    /// The declaration key is meaningful only in this exact semantic snapshot.
    Declaration(SnapshotId),
    /// Clone preserves identity; constructing another definition creates another identity.
    Schema(SchemaIdentity),
}

/// Exact descriptor definitions never contain a heap coordinate or a source relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConstantDeclaration {
    /// Identity separate from descriptive spelling and equal rational coordinates.
    identity: DefinitionIdentity,
    /// Declaration key or schema label.
    name: String,
    /// Original physical coordinate descriptor.
    descriptor: Descriptor,
    /// Exact definition in the descriptor's declared measure.
    value: Rational,
}

impl ConstantDeclaration {
    /// Resolve only a source-free constant in an already checked owner.
    fn named(owner: &CheckedDeclarations, key: &str) -> Result<Self, MapError> {
        let Some(Declaration::Constant {
            descriptor, value, ..
        }) = owner.declarations().get(key)
        else {
            return Err(MapError::ConstantDeclaration(key.into()));
        };
        Ok(Self {
            identity: DefinitionIdentity::Declaration(owner.id().clone()),
            name: key.into(),
            descriptor: descriptor.clone(),
            value: value.clone(),
        })
    }

    /// A schema definition is separate immutable evidence, never an addition to the owner.
    fn schema(name: String, descriptor: &Descriptor, value: Rational) -> Self {
        Self {
            identity: DefinitionIdentity::Schema(SchemaIdentity(Rc::new(()))),
            name,
            descriptor: descriptor.clone(),
            value,
        }
    }

    /// Named constants cannot be transplanted from another frozen declaration inventory.
    fn validate_snapshot(&self, owner: &CheckedDeclarations) -> Result<(), MapError> {
        if let DefinitionIdentity::Declaration(snapshot) = &self.identity {
            snapshot.validate(owner.id())?;
        }
        Ok(())
    }

    /// Convert a definition, never a stored coordinate, with the caller's shared ledger.
    fn coherent(&self, ledger: &mut Budget) -> Result<Rational, MapError> {
        Ok(self
            .value
            .mul(self.descriptor.measure().scale(), ledger)?
            .add(self.descriptor.measure().offset(), ledger)?)
    }
}

/// A positive exact difference reference with immutable definition identity.
///
/// New schema definitions are source-free declarations, not uncertain value operands.
/// They do not add names to a frozen context. Clone a definition to retain its identity;
/// separate constructions remain distinct even with identical names and coordinates.
#[doc = include_str!("../../tests/conformance/schema_constants.inc")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceDefinition {
    /// Exact source-free definition and its origin of authority.
    declaration: ConstantDeclaration,
    /// Checked nonzero difference-reference schema.
    reference: NamedReference,
}

impl ReferenceDefinition {
    /// Declare a positive exact reference coordinate in the supplied difference measure.
    ///
    /// # Errors
    /// Refuses symbolic, zero or negative definitions, empty names, points, counts and relative kinds.
    pub fn new(
        name: impl Into<String>,
        descriptor: &Descriptor,
        value: Scale,
    ) -> Result<Self, MapError> {
        let value = match value {
            Scale::Exact(value) => value,
            Scale::Symbolic(text) => return Err(UnitError::UnsupportedMeasureScale(text).into()),
        };
        Self::checked(ConstantDeclaration::schema(name.into(), descriptor, value))
    }

    /// Declare one unit of an exact difference measure, as used by matrix axis references.
    ///
    /// # Errors
    /// Refuses the same descriptor and name errors as `Self::new`.
    pub fn unit(name: impl Into<String>, descriptor: &Descriptor) -> Result<Self, MapError> {
        Self::new(name, descriptor, Scale::Exact(Rational::one()))
    }

    /// Positivity follows from the exact coordinate and the descriptor's positive measure scale.
    fn checked(declaration: ConstantDeclaration) -> Result<Self, MapError> {
        if declaration.descriptor.role() != Role::Difference
            || declaration.value.is_zero()
            || declaration.value.is_negative()
        {
            return Err(MapError::Reference);
        }
        check_continuous(&declaration.descriptor)?;
        let reference = NamedReference::new(&declaration.name, &declaration.descriptor, true)?;
        Ok(Self {
            declaration,
            reference,
        })
    }

    /// Named difference schema; its nonzero status follows from this exact definition.
    #[must_use]
    pub const fn reference(&self) -> &NamedReference {
        &self.reference
    }

    /// Original difference descriptor, before multiplication by the declared exact coordinate.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.declaration.descriptor
    }

    /// Resolve declaration provenance and registry compatibility against the frozen owner.
    pub(crate) fn validate(&self, owner: &CheckedDeclarations) -> Result<(), MapError> {
        self.declaration.validate_snapshot(owner)?;
        Ok(owner.registry().validate(self.descriptor())?)
    }

    /// Positive coherent definition, computed only within the supplied descriptor ledger.
    pub(crate) fn coherent(&self, ledger: &mut Budget) -> Result<Rational, MapError> {
        self.declaration.coherent(ledger)
    }
}

/// A source-free named exact point origin, with identity independent of its coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginDefinition {
    /// Original point declaration or independent schema definition.
    declaration: ConstantDeclaration,
}

impl OriginDefinition {
    /// Declare the zero coordinate of a point measure as its exact origin.
    ///
    /// This is the bindings-side input for a matrix point-origin definition. A named
    /// frozen constant with an arbitrary coordinate uses `CheckedDeclarations::origin_definition`.
    /// # Errors
    /// Refuses empty names and non-point descriptors.
    pub fn new(name: impl Into<String>, descriptor: &Descriptor) -> Result<Self, MapError> {
        Self::checked(ConstantDeclaration::schema(
            name.into(),
            descriptor,
            Rational::zero(),
        ))
    }

    /// Point role and an explicit nonempty declaration name are mandatory.
    fn checked(declaration: ConstantDeclaration) -> Result<Self, MapError> {
        if declaration.name.trim().is_empty() || declaration.descriptor.role() != Role::Point {
            return Err(MapError::Origin);
        }
        Ok(Self { declaration })
    }

    /// Declaration key or schema label, distinct from identity.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.declaration.name
    }

    /// Original point definition, including the exact measure origin.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.declaration.descriptor
    }

    /// Resolve declaration provenance and registry compatibility against the frozen owner.
    pub(crate) fn validate(&self, owner: &CheckedDeclarations) -> Result<(), MapError> {
        self.declaration.validate_snapshot(owner)?;
        Ok(owner.registry().validate(self.descriptor())?)
    }

    /// Exact coherent point origin for affine map construction.
    pub(crate) fn coherent(&self, ledger: &mut Budget) -> Result<Rational, MapError> {
        self.declaration.coherent(ledger)
    }
}

impl CheckedDeclarations {
    /// Resolve an existing positive exact difference constant without changing the snapshot.
    ///
    /// # Errors
    /// Refuses unknown/nonconstant declarations, nonpositive references and unsupported roles/kinds.
    pub fn reference_definition(&self, key: &str) -> Result<ReferenceDefinition, MapError> {
        ReferenceDefinition::checked(ConstantDeclaration::named(self, key)?)
    }

    /// Resolve an existing exact point constant without changing the snapshot.
    ///
    /// # Errors
    /// Refuses unknown/nonconstant declarations, empty names and non-point origins.
    pub fn origin_definition(&self, key: &str) -> Result<OriginDefinition, MapError> {
        OriginDefinition::checked(ConstantDeclaration::named(self, key)?)
    }
}

/// Counts and unnamed relative values cannot serve as continuous normalization coordinates.
pub(crate) fn check_continuous(descriptor: &Descriptor) -> Result<(), MapError> {
    if descriptor.kind().count_domain().is_some() {
        return Err(UnitError::CountDomain.into());
    }
    if descriptor.kind().is_relative() {
        return Err(UnitError::RelativeReference.into());
    }
    Ok(())
}
