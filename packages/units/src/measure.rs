// SPDX-License-Identifier: AGPL-3.0-only
//! Exact rational measure definitions and role-aware conversion recipes.

use nrz_algebra::Rational;

use crate::{Role, UnitError};

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
        if scale <= Rational::ZERO {
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
            scale: Rational::ONE,
            offset: Rational::ZERO,
        }
    }

    /// Reporting name, which carries no inferred semantics.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact scale relative to coherent coordinates.
    #[must_use]
    pub const fn scale(&self) -> Rational {
        self.scale
    }

    /// Coherent-coordinate origin.
    #[must_use]
    pub const fn offset(&self) -> Rational {
        self.offset
    }

    /// The difference measure keeps the scale and removes the affine origin.
    #[must_use]
    pub fn difference(&self) -> Self {
        Self {
            name: self.name.clone(),
            scale: self.scale,
            offset: Rational::ZERO,
        }
    }

    /// Construct a role-aware exact conversion; dimension and kind checks belong to `Descriptor`.
    ///
    /// # Errors
    /// Refuses offset-bearing differences and exact arithmetic overflow.
    pub fn conversion_to(&self, target: &Self, role: Role) -> Result<Conversion, UnitError> {
        if role == Role::Difference
            && (self.offset != Rational::ZERO || target.offset != Rational::ZERO)
        {
            return Err(UnitError::OffsetOnDifference);
        }
        let scale = self.scale.checked_div(target.scale)?;
        let shift = if role == Role::Point {
            self.offset
                .checked_sub(target.offset)?
                .checked_div(target.scale)?
        } else {
            Rational::ZERO
        };
        Ok(Conversion { scale, shift })
    }
}

/// An exact coordinate map; it does not change the quantity's role or kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Conversion {
    /// Multiplicative coordinate factor.
    pub scale: Rational,
    /// Translation, always zero for differences.
    pub shift: Rational,
}

impl Conversion {
    /// Apply the map to an exact coordinate for conversion oracles and exact imports.
    ///
    /// # Errors
    /// Refuses exact arithmetic overflow; no floating conversion bound is implied.
    pub fn apply(self, coordinate: Rational) -> Result<Rational, UnitError> {
        Ok(self
            .scale
            .checked_mul(coordinate)?
            .checked_add(self.shift)?)
    }
}
