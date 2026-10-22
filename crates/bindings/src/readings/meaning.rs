// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable entry meanings retain exact reference and origin identities.

use super::{ReadingError, support};
use crate::{Descriptor, OriginDefinition, ReferenceDefinition, Role};
use nrz_cas::{ExactConstant, Rational, limits::Budget, readings::AffineMap};
use std::rc::Rc;

/// One physical coordinate, independently of its location in a numerical buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryMeaning(Rc<Meaning>);

/// Shared immutable physical coordinate definition; cloning a reading preserves identity.
#[derive(Debug, Eq, PartialEq)]
struct Meaning {
    /// Original kind, measure and affine role.
    descriptor: Descriptor,
    /// Source-free positive reference with independent identity.
    reference: ReferenceDefinition,
    /// Source-free point origin, absent for differences.
    origin: Option<OriginDefinition>,
    /// Exact coherent magnitude of the reference.
    magnitude: ExactConstant,
    /// Exact coherent origin retained separately from scaling.
    shift: ExactConstant,
}

impl EntryMeaning {
    /// Check a continuous entry and its fixed source-free reference and optional origin.
    /// # Errors
    /// Refuses counts, incompatible kinds/roles, absent point origins and exact limits.
    pub fn new(
        descriptor: Descriptor,
        reference: ReferenceDefinition,
        origin: Option<OriginDefinition>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        support::capacity::<Meaning>(1, budget)?;
        if descriptor.kind().count_domain().is_some() || descriptor.kind().is_relative() {
            return Err(ReadingError::Unsupported);
        }
        descriptor.compatible_kind(reference.descriptor())?;
        let shift = match (descriptor.role(), &origin) {
            (Role::Difference, None) => ExactConstant::zero(),
            (Role::Point, Some(value)) => {
                descriptor.compatible_kind(value.descriptor())?;
                value.coherent(budget)?
            }
            _ => return Err(ReadingError::Meaning),
        };
        let magnitude = reference.coherent(budget)?;
        Ok(Self(Rc::new(Meaning {
            descriptor,
            reference,
            origin,
            magnitude,
            shift,
        })))
    }

    /// Match complete physical coordinates, including independent reference identities.
    /// # Errors
    /// Refuses changed kinds, units, roles, references or origins by their invariant.
    pub fn check(&self, expected: &Self) -> Result<(), ReadingError> {
        self.descriptor().compatible_kind(expected.descriptor())?;
        if self.descriptor().role() != expected.descriptor().role() {
            return Err(ReadingError::Role);
        }
        if self.descriptor().measure() != expected.descriptor().measure() {
            return Err(ReadingError::Measure);
        }
        if self.reference() != expected.reference() {
            return Err(ReadingError::Reference);
        }
        if self.origin() != expected.origin() {
            return Err(ReadingError::Origin);
        }
        Ok(())
    }

    /// Per-entry kind, unit and affine role.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.descriptor
    }
    /// Exact positive reference, including definition identity.
    #[must_use]
    pub fn reference(&self) -> &ReferenceDefinition {
        &self.0.reference
    }
    /// Exact point origin; absent for differences.
    #[must_use]
    pub fn origin(&self) -> Option<&OriginDefinition> {
        self.0.origin.as_ref()
    }
    /// Exact coherent reference magnitude, never a stored value.
    #[must_use]
    pub fn magnitude(&self) -> &ExactConstant {
        &self.0.magnitude
    }
    /// Exact coherent origin, zero for differences.
    #[must_use]
    pub fn shift(&self) -> &ExactConstant {
        &self.0.shift
    }
}

/// Check all original kind declarations, including coordinates later multiplied by zero.
pub(super) fn validate(entries: &[EntryMeaning], budget: &mut Budget) -> Result<(), ReadingError> {
    support::capacity::<EntryMeaning>(entries.len(), budget)?;
    let mut names = Vec::new();
    for entry in entries {
        check_kind(entry.descriptor().kind(), &mut names, budget)?;
    }
    Ok(())
}

/// Every nonzero coefficient connects identical physical coordinates; affine points preserve origin weight.
pub(super) fn map_meanings(
    map: &AffineMap,
    inputs: &[EntryMeaning],
    outputs: &[EntryMeaning],
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    validate(inputs, budget)?;
    validate(outputs, budget)?;
    let width = usize::try_from(map.inputs()).map_err(|_| ReadingError::Shape)?;
    if inputs.len() != width
        || outputs.len() != usize::try_from(map.outputs()).map_err(|_| ReadingError::Shape)?
    {
        return Err(ReadingError::Shape);
    }
    for (row, output) in outputs.iter().enumerate() {
        let mut sum = Rational::zero();
        for (column, input) in inputs.iter().enumerate() {
            budget.work(1, support::AT)?;
            let coefficient = map
                .coefficients()
                .get(row * width + column)
                .ok_or(ReadingError::Shape)?;
            if !coefficient.is_zero() {
                input.check(output)?;
            }
            sum = sum.add(coefficient, budget)?;
        }
        if output.descriptor().role() == Role::Point && sum != Rational::one() {
            return Err(ReadingError::Meaning);
        }
    }
    Ok(())
}

/// Check shared kind names without resetting the exact arithmetic ledger.
fn check_kind<'a>(
    kind: &'a crate::Kind,
    names: &mut Vec<(&'a str, &'a crate::Dimension)>,
    budget: &mut Budget,
) -> Result<(), ReadingError> {
    budget.work(1, support::AT)?;
    match kind {
        crate::Kind::Named(name, dimension) => {
            budget.work(
                u64::try_from(name.len() + names.len()).map_err(|_| ReadingError::Shape)?,
                support::AT,
            )?;
            if names
                .iter()
                .any(|(key, value)| *key == name && *value != dimension.as_ref())
            {
                return Err(crate::UnitError::ConflictingKind(name.clone()).into());
            }
            names
                .try_reserve(1)
                .map_err(|_| nrz_cas::Fault::allocation(support::AT))?;
            names.push((name, dimension));
        }
        crate::Kind::Uncertainty(_, target, other) => {
            check_kind(target, names, budget)?;
            if let Some(other) = other {
                check_kind(other, names, budget)?;
            }
        }
        crate::Kind::Ratio | crate::Kind::Angle | crate::Kind::Count(_, _) => {}
    }
    Ok(())
}
