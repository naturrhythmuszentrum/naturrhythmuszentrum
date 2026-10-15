// SPDX-License-Identifier: AGPL-3.0-only
//! Named reference assumptions are retained separately from scalar kind admission.

use crate::{Descriptor, Role, UnitError};

/// A schema-local named difference reference with an explicit nonzero assumption.
///
/// Its key is a declaration/reference key, not a heap identity or a source identity.
/// The numerical owner must bind the key and discharge any numerical requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedReference {
    /// Caller-declared reference key, never an inferred point center.
    name: String,
    /// Difference descriptor whose nonzero condition was explicitly supplied.
    descriptor: Descriptor,
}

impl NamedReference {
    /// Admit a named nonzero difference-reference assumption.
    ///
    /// # Errors
    /// Refuses empty names, points, relative references and absent nonzero evidence.
    pub fn new(
        name: impl Into<String>,
        descriptor: &Descriptor,
        nonzero: bool,
    ) -> Result<Self, UnitError> {
        let name = name.into();
        if name.trim().is_empty()
            || !nonzero
            || descriptor.role() != Role::Difference
            || descriptor.kind().is_relative()
        {
            return Err(UnitError::RelativeReference);
        }
        Ok(Self {
            name,
            descriptor: descriptor.clone(),
        })
    }

    /// Declared reference key, whose concrete identity belongs to the numerical owner.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The reference's checked difference schema.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}
