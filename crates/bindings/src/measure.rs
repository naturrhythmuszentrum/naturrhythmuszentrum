// SPDX-License-Identifier: AGPL-3.0-only
//! Exact CAS measure definitions and role-aware conversion recipes.

use nrz_cas::{ExactConstant, PositiveConstant, Rational};

use crate::{Role, UnitError, exact::budget};

/// A proposed exact scale definition; admission requires checked positivity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scale {
    /// An exact dimensionless rational scale.
    Exact(Rational),
    /// A finite closed CAS expression whose positivity is checked during admission.
    Expression(ExactConstant),
    /// Unchecked text is not mathematical evidence and is refused without approximation.
    Symbolic(String),
}

/// A reporting name and an exact map to coherent coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Measure {
    /// Human-readable unit name, never interpreted as a dimension or kind.
    name: String,
    /// Strictly positive scale.
    scale: PositiveConstant,
    /// Coherent-coordinate origin for points.
    offset: Rational,
}

impl Measure {
    /// Validate a measure definition without inferring semantics from its name.
    ///
    /// # Errors
    /// Refuses unchecked text, nonpositive or unproved-positive scales and empty names.
    pub fn new(name: impl Into<String>, scale: Scale, offset: Rational) -> Result<Self, UnitError> {
        let value = scale.value()?;
        let scale = value.positive().ok_or(UnitError::InvalidScale)?;
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
            scale: PositiveConstant::one(),
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
    pub const fn scale(&self) -> &ExactConstant {
        self.scale.value()
    }

    /// Extract a rational scale at an existing rational-only numerical boundary.
    /// # Errors
    /// Refuses a non-rational exact scale without a numerical approximation.
    pub fn rational_scale(&self) -> Result<&Rational, UnitError> {
        self.scale()
            .as_rational()
            .ok_or(UnitError::UnsupportedExactConstantExport)
    }

    /// Retained checked positivity and finiteness witness for the exact scale.
    #[must_use]
    pub const fn scale_witness(&self) -> &PositiveConstant {
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
        let scale = self.scale().div(target.scale(), &mut ledger)?;
        let shift = if role == Role::Point {
            ExactConstant::from(self.offset.sub(&target.offset, &mut ledger)?)
                .div(target.scale(), &mut ledger)?
        } else {
            ExactConstant::zero()
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
    pub scale: ExactConstant,
    /// Translation, always zero for differences.
    pub shift: ExactConstant,
}

impl Conversion {
    /// Export a bounded exact literal for this source-free coefficient definition.
    ///
    /// Signed and zero factors are retained without a floating approximation.
    /// # Errors
    /// Refuses the same exact arithmetic, output-byte and formatting-work limits as descriptor constants.
    pub fn export_scale(&self, limits: &crate::LimitsOverride) -> Result<String, crate::MapError> {
        let mut ledger = nrz_cas::limits::Budget::new(crate::Limits::default().resolve(limits)?)?;
        Ok(self.rational_scale()?.exact_string(&mut ledger)?)
    }

    /// Apply the map to an exact coordinate for conversion oracles and exact imports.
    ///
    /// TODO ´todo:code:apply-scalar-coordinate-maps-through-entrywise´: apply scalar coordinate maps through entrywise or paired tensor transformations at owners under def:nrz:covariance-shape and rule:nrz:shape-sensitivity-propagation.
    ///
    /// # Errors
    /// Refuses exact arithmetic overflow; no floating conversion bound is implied.
    pub fn apply(&self, coordinate: &Rational) -> Result<Rational, UnitError> {
        self.apply_exact(&coordinate.clone().into())?
            .as_rational()
            .cloned()
            .ok_or(UnitError::UnsupportedExactConstantExport)
    }

    /// Apply an exact map to a source-free exact coordinate, retaining irrational constants.
    /// # Errors
    /// Refuses exhausted exact arithmetic or CAS graph budgets.
    pub fn apply_exact(&self, coordinate: &ExactConstant) -> Result<ExactConstant, UnitError> {
        let mut ledger = budget()?;
        Ok(self
            .scale
            .mul(coordinate, &mut ledger)?
            .add(&self.shift, &mut ledger)?)
    }

    /// Extract the exact rational factor required by a rational-only consumer.
    /// # Errors
    /// Refuses a non-rational factor without approximation.
    pub fn rational_scale(&self) -> Result<&Rational, UnitError> {
        self.scale
            .as_rational()
            .ok_or(UnitError::UnsupportedExactConstantExport)
    }

    /// Extract the exact rational shift required by a rational-only consumer.
    /// # Errors
    /// Refuses a non-rational shift without approximation.
    pub fn rational_shift(&self) -> Result<&Rational, UnitError> {
        self.shift
            .as_rational()
            .ok_or(UnitError::UnsupportedExactConstantExport)
    }
}

impl Scale {
    /// Propose the exact circle constant through the CAS's typed mathematical constructor.
    /// # Errors
    /// Refuses exhausted exact graph construction or proof budgets.
    pub fn pi() -> Result<Self, UnitError> {
        Ok(Self::Expression(ExactConstant::named(
            nrz_cas::syntax::Constant::Pi,
            &mut budget()?,
        )?))
    }

    /// Text cannot acquire graph or positivity authority through a reporting spelling.
    pub(crate) fn value(self) -> Result<ExactConstant, UnitError> {
        match self {
            Self::Exact(value) => Ok(value.into()),
            Self::Expression(value) => Ok(value),
            Self::Symbolic(definition) => Err(UnitError::UnsupportedMeasureScale(definition)),
        }
    }
}
