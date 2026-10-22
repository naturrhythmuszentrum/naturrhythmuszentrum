// SPDX-License-Identifier: AGPL-3.0-only
//! Checked quantity and independent uncertainty descriptors.
//!
//! # Quantity and uncertainty meanings
//!
//! A descriptor combines a seven-axis SI dimension, an explicit semantic kind, an exact measure and an affine point or difference role. Dimensions use budgeted CAS rational arithmetic, with signed 64-bit numerators and unsigned 64-bit denominators as an explicit dimension admission boundary; an integral-only policy can reject fractional results. Kind identity is declared, never inferred from equal dimension vectors. Named kinds bind invariant dimensions in a registry. Nested kind relationships have a depth limit of 128.
//!
//! Radius, standard uncertainty, relative radius, relative standard uncertainty, variance and covariance have distinct kinds and their own measures. Absolute companions use difference dimensions and scales; variance squares them, and covariance takes the product of the two values' difference dimensions and scales. Relative descriptors require a compatible declared nonzero difference reference; the general descriptor constructor refuses a raw relative kind, and construction refuses a relative kind nested within any other uncertainty kind. Explicit relative conversion checks that reference, while ordinary division by a ratio coefficient preserves the uncertainty kind. Descriptor-only multiplication of standard uncertainties does not infer a variance, because value identity is unavailable at that layer. A standard uncertainty of a count is continuous, while the value remains discrete. These declarations provide neither numerical uncertainty propagation nor a proof of an assurance claim.

use nrz_cas::Rational;

use crate::exact::budget;
use crate::{Conversion, Dimension, Kind, Measure, Scale, UncertaintyMode, UnitError};

/// Affine coordinates and linear differences obey distinct operation rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// A linear difference, including ordinary multiplicative quantities.
    Difference,
    /// A coordinate relative to a declared affine origin.
    Point,
}

/// A validated data-time quantity descriptor, independent of numerical storage.
///
/// TODO ´todo:code:reuse-scalar-descriptors-on-ordered-matrix´: reuse scalar descriptors on ordered matrix axes without inferring value identity from equality under def:nrz:matrix-shape and dec:nrz:shape-descriptor-ownership.
///
/// TODO ´todo:code:share-immutable-quantity-schemas-without-merging´: share immutable quantity schemas without merging bound value identities under def:nrz:heap-descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    /// Cached dimension derived from the kind declaration.
    dimension: Dimension,
    /// Semantic identity, including any uncertainty or count contract.
    kind: Kind,
    /// Coordinate map to coherent measure.
    measure: Measure,
    /// Affine operation role, retained even at zero offset.
    role: Role,
}

impl Descriptor {
    /// Construct a descriptor consistent with its declared kind and role.
    ///
    /// # Errors
    /// Refuses unchecked or nested relative kinds, invalid relationships, point kinds, difference offsets
    /// or count measures. Relative descriptors use the checked uncertainty constructor.
    pub fn new(kind: Kind, measure: Measure, role: Role) -> Result<Self, UnitError> {
        if kind.is_relative() {
            return Err(UnitError::RelativeReference);
        }
        Self::validated(kind, measure, role)
    }

    /// Construct after the caller has checked any required relative reference.
    fn validated(kind: Kind, measure: Measure, role: Role) -> Result<Self, UnitError> {
        let dimension = kind.dimension()?;
        if role == Role::Point && !kind.admits_points() {
            return Err(UnitError::AffineRole);
        }
        if role == Role::Difference && !measure.offset().is_zero() {
            return Err(UnitError::OffsetOnDifference);
        }
        if kind.count_domain().is_some()
            && (measure.scale() != &Rational::one() || !measure.offset().is_zero())
        {
            return Err(UnitError::CountMeasure);
        }
        Ok(Self {
            dimension,
            kind,
            measure,
            role,
        })
    }

    /// A bare numeric coefficient's descriptor.
    #[must_use]
    pub fn ratio() -> Self {
        Self {
            dimension: Dimension::zero(),
            kind: Kind::Ratio,
            measure: Measure::coherent(),
            role: Role::Difference,
        }
    }

    /// Dimension, derived independently of the coordinate measure.
    #[must_use]
    pub const fn dimension(&self) -> &Dimension {
        &self.dimension
    }

    /// Semantic identity.
    #[must_use]
    pub const fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Source measure.
    #[must_use]
    pub const fn measure(&self) -> &Measure {
        &self.measure
    }

    /// Affine role.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Coherent coordinates preserve semantic identity and affine role.
    #[must_use]
    pub fn coherent(&self) -> Self {
        Self {
            dimension: self.dimension.clone(),
            kind: self.kind.clone(),
            measure: Measure::coherent(),
            role: self.role,
        }
    }

    /// Translate a point descriptor into its associated difference descriptor.
    #[must_use]
    pub fn difference(&self) -> Self {
        Self {
            dimension: self.dimension.clone(),
            kind: self.kind.clone(),
            measure: self.measure.difference(),
            role: Role::Difference,
        }
    }

    /// Check dimension and kind compatibility independently of role or measure.
    ///
    /// TODO ´todo:code:validate-axis-coordinate-meaning-beside-scalar´: validate axis coordinate meaning beside scalar kind compatibility at shape assembly under rule:nrz:shape-composition.
    ///
    /// # Errors
    /// Refuses unequal dimensions or incompatible semantic kinds.
    pub fn compatible_kind(&self, other: &Self) -> Result<(), UnitError> {
        if self.dimension != other.dimension {
            Err(UnitError::DimensionMismatch)
        } else if self.kind != other.kind {
            Err(UnitError::KindMismatch)
        } else {
            Ok(())
        }
    }

    /// Convert coordinates only after checking dimensions, kinds and affine roles.
    ///
    /// TODO ´todo:code:compose-scalar-conversions-into-declared-axis´: compose scalar conversions into declared axis maps and paired covariance conversions under def:nrz:shape-normalization and def:nrz:covariance-shape.
    ///
    /// TODO ´todo:code:key-reusable-conversion-maps-by-both´: key reusable conversion maps by both bound descriptor snapshots under rule:nrz:binding-validation.
    ///
    /// # Errors
    /// Refuses incompatible descriptors and exact conversion overflow.
    pub fn conversion_to(&self, other: &Self) -> Result<Conversion, UnitError> {
        self.compatible_kind(other)?;
        if self.role != other.role {
            return Err(UnitError::AffineRole);
        }
        self.measure.conversion_to(&other.measure, self.role)
    }

    /// Check an exact count import without reinterpreting any bare integer coefficient.
    ///
    /// # Errors
    /// Refuses a non-count descriptor or a value outside its declared integer domain.
    pub fn check_count_literal(&self, value: &Rational) -> Result<(), UnitError> {
        self.kind
            .count_domain()
            .ok_or(UnitError::KindMismatch)?
            .check(value)
    }
}

/// Derive an uncertainty's own descriptor, including squared or product measures.
///
/// Relative modes additionally require a compatible difference reference and a declared
/// nonzero condition. The condition is an input assumption, not a numerical proof.
///
/// TODO ´todo:code:derive-each-indexed-companion-descriptor-independently´: derive each indexed companion descriptor independently of its target extent and representation under def:nrz:companion-shape and def:nrz:covariance-shape.
///
/// TODO ´todo:code:instantiate-companion-schemas-separately-for-each´: instantiate companion schemas separately for each bound target identity under def:nrz:heap-descriptor.
///
/// # Errors
/// Refuses malformed relationships, incompatible relative references and exact overflow.
pub fn uncertainty(
    mode: UncertaintyMode,
    value: &Descriptor,
    other: Option<&Descriptor>,
    reference: Option<(&Descriptor, bool)>,
) -> Result<Descriptor, UnitError> {
    let relative = matches!(
        mode,
        UncertaintyMode::RelativeRadius | UncertaintyMode::RelativeStandard
    );
    if (mode == UncertaintyMode::Covariance) != other.is_some() {
        return Err(UnitError::UncertaintyRelationship);
    }
    if relative {
        let (reference, nonzero) = reference.ok_or(UnitError::RelativeReference)?;
        if !nonzero
            || reference.role != Role::Difference
            || value.compatible_kind(reference).is_err()
        {
            return Err(UnitError::RelativeReference);
        }
    } else if reference.is_some() {
        return Err(UnitError::UncertaintyRelationship);
    }
    let scale = match mode {
        UncertaintyMode::Radius | UncertaintyMode::Standard => value.measure.scale().clone(),
        UncertaintyMode::RelativeRadius | UncertaintyMode::RelativeStandard => {
            nrz_cas::ExactConstant::one()
        }
        UncertaintyMode::Variance => value.measure.scale().pow(2, &mut budget()?)?,
        UncertaintyMode::Covariance => value.measure.scale().mul(
            other
                .ok_or(UnitError::UncertaintyRelationship)?
                .measure
                .scale(),
            &mut budget()?,
        )?,
    };
    let kind = Kind::Uncertainty(
        mode,
        Box::new(value.kind.clone()),
        other.map(|other| Box::new(other.kind.clone())),
    );
    Descriptor::validated(
        kind,
        Measure::new("uncertainty", Scale::Expression(scale), Rational::zero())?,
        Role::Difference,
    )
}
