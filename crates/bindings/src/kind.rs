// SPDX-License-Identifier: AGPL-3.0-only
//! Semantic identity and discrete count-domain declarations.

use crate::exact::{budget, ratio};
use nrz_cas::Rational;

use crate::{Dimension, ExponentPolicy, UnitError};

/// A mathematical integer domain, separate from its eventual numerical storage type.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CountDomain {
    /// Optional inclusive mathematical lower bound.
    minimum: Option<i64>,
    /// Optional inclusive mathematical upper bound.
    maximum: Option<i64>,
}

impl CountDomain {
    /// All mathematical integers; finite imports still require exact representation.
    pub const SIGNED: Self = Self {
        minimum: None,
        maximum: None,
    };
    /// Nonnegative mathematical integers.
    pub const NONNEGATIVE: Self = Self {
        minimum: Some(0),
        maximum: None,
    };

    /// Declare an optional inclusive range.
    ///
    /// # Errors
    /// Refuses an inverted range.
    pub fn new(minimum: Option<i64>, maximum: Option<i64>) -> Result<Self, UnitError> {
        if minimum.zip(maximum).is_some_and(|(a, b)| a > b) {
            Err(UnitError::CountDomain)
        } else {
            Ok(Self { minimum, maximum })
        }
    }

    /// Check a literal without rounding a fractional count into an integer.
    ///
    /// # Errors
    /// Refuses a fractional value or one outside the declared range.
    pub fn check(self, value: &Rational) -> Result<(), UnitError> {
        if !value.is_integer() {
            return Err(UnitError::CountValue);
        }
        let mut ledger = budget()?;
        for (bound, forbidden) in [
            (self.minimum, std::cmp::Ordering::Less),
            (self.maximum, std::cmp::Ordering::Greater),
        ] {
            if let Some(bound) = bound
                && value.compare(&ratio(bound, 1)?, &mut ledger)? == forbidden
            {
                return Err(UnitError::CountValue);
            }
        }
        Ok(())
    }

    /// Whether unconstrained addition is closed in this supported domain.
    #[must_use]
    pub fn admits_addition(self) -> bool {
        self == Self::SIGNED || self == Self::NONNEGATIVE
    }

    /// Whether unconstrained subtraction is closed in this supported domain.
    #[must_use]
    pub fn admits_subtraction(self) -> bool {
        self == Self::SIGNED
    }

    /// Verify whole-number scaling under a conservative closure rule.
    ///
    /// # Errors
    /// Refuses a fractional multiplier or an unproved domain condition.
    pub fn admits_scale(self, scale: &Rational) -> Result<(), UnitError> {
        if !scale.is_integer() {
            return Err(UnitError::CountValue);
        }
        if scale == &Rational::one()
            || self == Self::SIGNED
            || (self == Self::NONNEGATIVE && !scale.is_negative())
            || (scale.is_zero() && self.check(&Rational::zero()).is_ok())
        {
            Ok(())
        } else {
            Err(UnitError::CountDomain)
        }
    }
}

/// The relationship of an uncertainty expression to its values, not an assurance proof.
///
/// TODO ´todo:code:declare-correlation-and-expanded-modes-with´: declare correlation and expanded modes with their separate reference and coverage contracts under rule:nrz:correlation-and-relative-shapes and rule:nrz:expanded-uncertainty-shapes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UncertaintyMode {
    /// An absolute deterministic difference bound.
    Radius,
    /// A continuous standard uncertainty.
    Standard,
    /// A deterministic bound divided by a compatible nonzero reference.
    RelativeRadius,
    /// A standard uncertainty divided by a compatible nonzero reference.
    RelativeStandard,
    /// A variance with twice its value's dimension.
    Variance,
    /// A cross-covariance with the product of the two difference dimensions.
    Covariance,
}

/// Semantic meaning, kept even when dimension vectors cancel.
///
/// TODO ´todo:code:retain-paired-target-kinds-during-covariance´: retain paired target kinds during covariance contractions without equating dual roles to automatic scalar reciprocals under def:nrz:covariance-shape and rule:nrz:shape-transpose-and-duality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A continuous dimensionless ratio.
    Ratio,
    /// An angle, with coherent measure radians.
    Angle,
    /// A discrete tally of a declared thing in a declared integer domain.
    Count(String, CountDomain),
    /// A declared physical kind with its invariant dimension.
    Named(String, Box<Dimension>),
    /// A distinct uncertainty kind with its referenced value kinds.
    Uncertainty(UncertaintyMode, Box<Self>, Option<Box<Self>>),
}

impl Kind {
    /// Whether this kind requires an explicit compatible nonzero reference.
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        matches!(
            self,
            Self::Uncertainty(
                UncertaintyMode::RelativeRadius | UncertaintyMode::RelativeStandard,
                _,
                _
            )
        )
    }

    /// Derive this kind's dimension without inferring a kind from any dimension vector.
    ///
    /// # Errors
    /// Refuses a malformed relationship, nested relative kind or exponent overflow.
    pub fn dimension(&self) -> Result<Dimension, UnitError> {
        self.dimension_at(0)
    }

    /// Bound recursive relationship traversal before deriving any nested dimension.
    fn dimension_at(&self, depth: usize) -> Result<Dimension, UnitError> {
        if depth >= 128 {
            return Err(UnitError::KindDepth);
        }
        if depth != 0 && self.is_relative() {
            return Err(UnitError::RelativeReference);
        }
        match self {
            Self::Ratio | Self::Angle | Self::Count(_, _) => Ok(Dimension::zero()),
            Self::Named(_, dimension) => Ok(dimension.as_ref().clone()),
            Self::Uncertainty(mode, value, other) => {
                let dimension = value.dimension_at(depth + 1)?;
                if (*mode == UncertaintyMode::Covariance) != other.is_some() {
                    return Err(UnitError::UncertaintyRelationship);
                }
                match mode {
                    UncertaintyMode::Radius | UncertaintyMode::Standard => Ok(dimension),
                    UncertaintyMode::RelativeRadius | UncertaintyMode::RelativeStandard => {
                        Ok(Dimension::zero())
                    }
                    UncertaintyMode::Variance => {
                        dimension.power(&ratio(2, 1)?, ExponentPolicy::Rational)
                    }
                    UncertaintyMode::Covariance => dimension.product(
                        &other
                            .as_ref()
                            .ok_or(UnitError::UncertaintyRelationship)?
                            .dimension_at(depth + 1)?,
                    ),
                }
            }
        }
    }

    /// A count's declared integer domain, if this is a discrete count.
    #[must_use]
    pub const fn count_domain(&self) -> Option<CountDomain> {
        if let Self::Count(_, domain) = self {
            Some(*domain)
        } else {
            None
        }
    }

    /// Whether this kind permits an affine point interpretation.
    #[must_use]
    pub const fn admits_points(&self) -> bool {
        matches!(self, Self::Named(_, _) | Self::Angle)
    }
}
