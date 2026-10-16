// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational measure definitions and role-aware conversion recipes.

use nrz_cas::Rational;

use crate::{Role, UnitError, exact::budget};

/// A proposed scale definition, including the explicitly unsupported future boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scale {
    /// An exact dimensionless rational scale.
    Exact(Rational),
    /// An irrational or symbolic definition, refused without approximation.
    Symbolic(String),
}

/// A reporting name and an exact map to coherent coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Measure {
    /// Human-readable unit name, never interpreted as a dimension or kind.
    name: String,
    /// Strictly positive scale.
    scale: Rational,
    /// Coherent-coordinate origin for points.
    offset: Rational,
}

impl Measure {
    /// Validate a measure definition without inferring semantics from its name.
    ///
    /// # Errors
    /// Refuses symbolic scales, nonpositive scales and empty names.
    pub fn new(name: impl Into<String>, scale: Scale, offset: Rational) -> Result<Self, UnitError> {
        let scale = match scale {
            Scale::Exact(value) => value,
            Scale::Symbolic(definition) => {
                return Err(UnitError::UnsupportedMeasureScale(definition));
            }
        };
        if scale.is_negative() || scale.is_zero() {
            return Err(UnitError::InvalidScale);
        }
        let name = name.into();
        if name.is_empty() {
            return Err(UnitError::InvalidMeasureName);
        }
        Ok(Self {
            name,
            scale,
            offset,
        })
    }

    /// Coherent coordinates with unit scale and zero origin.
    #[must_use]
    pub fn coherent() -> Self {
        Self {
            name: "SI".to_owned(),
            scale: Rational::one(),
            offset: Rational::zero(),
        }
    }

    /// Reporting name, which carries no inferred semantics.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact scale relative to coherent coordinates.
    #[must_use]
    pub const fn scale(&self) -> &Rational {
        &self.scale
    }

    /// Coherent-coordinate origin.
    #[must_use]
    pub const fn offset(&self) -> &Rational {
        &self.offset
    }

    /// The difference measure keeps the scale and removes the affine origin.
    #[must_use]
    pub fn difference(&self) -> Self {
        Self {
            name: self.name.clone(),
            scale: self.scale.clone(),
            offset: Rational::zero(),
        }
    }

    /// Construct a role-aware exact conversion; dimension and kind checks belong to `Descriptor`.
    ///
    /// TODO ´todo:code:derive-difference-scale-maps-for-both´: derive difference scale maps for both uncertainty axes without translating covariance under def:nrz:covariance-shape.
    ///
    /// # Errors
    /// Refuses offset-bearing differences and exact arithmetic overflow.
    pub fn conversion_to(&self, target: &Self, role: Role) -> Result<Conversion, UnitError> {
        if role == Role::Difference
            && (self.offset != Rational::zero() || target.offset != Rational::zero())
        {
            return Err(UnitError::OffsetOnDifference);
        }
        let mut ledger = budget()?;
        let scale = self.scale.div(&target.scale, &mut ledger)?;
        let shift = if role == Role::Point {
            self.offset
                .sub(&target.offset, &mut ledger)?
                .div(&target.scale, &mut ledger)?
        } else {
            Rational::zero()
        };
        Ok(Conversion { scale, shift })
    }
}

/// An exact coordinate map; it does not change the quantity's role or kind.
///
/// TODO ´todo:code:compose-coordinate-conversions-with-explicit-operator´: compose coordinate conversions with explicit operator and companion maps at shape owners under def:nrz:shape-normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conversion {
    /// Multiplicative coordinate factor.
    pub scale: Rational,
    /// Translation, always zero for differences.
    pub shift: Rational,
}

impl Conversion {
    /// Export a bounded exact literal for this source-free coefficient definition.
    ///
    /// Signed and zero factors are retained without a floating approximation.
    /// # Errors
    /// Refuses the same exact arithmetic, output-byte and formatting-work limits as descriptor constants.
    pub fn export_scale(&self, limits: &crate::LimitsOverride) -> Result<String, crate::MapError> {
        let mut ledger = nrz_cas::limits::Budget::new(crate::Limits::default().resolve(limits)?)?;
        Ok(self.scale.exact_string(&mut ledger)?)
    }

    /// Apply the map to an exact coordinate for conversion oracles and exact imports.
    ///
    /// TODO ´todo:code:apply-scalar-coordinate-maps-through-entrywise´: apply scalar coordinate maps through entrywise or paired tensor transformations at owners under def:nrz:covariance-shape and rule:nrz:shape-sensitivity-propagation.
    ///
    /// # Errors
    /// Refuses exact arithmetic overflow; no floating conversion bound is implied.
    pub fn apply(&self, coordinate: &Rational) -> Result<Rational, UnitError> {
        let mut ledger = budget()?;
        Ok(self
            .scale
            .mul(coordinate, &mut ledger)?
            .add(&self.shift, &mut ledger)?)
    }
}
