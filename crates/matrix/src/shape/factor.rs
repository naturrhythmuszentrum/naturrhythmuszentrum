// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor-only factor envelopes; no numerical factor or reconstruction witness.

use super::{Axis, CheckedShape, ShapeError, ShapeLimits};
use nrz_bindings::{Conversion, Descriptor, Measure, Role, Scale};

/// Ordered homogeneous QR descriptors with an explicit intermediate axis.
///
/// This checks only shape and scalar meanings. It proves neither `A = Q R`
/// nor orthogonality, and it cannot authorize a numerical factor export.
#[derive(Clone, Debug)]
pub struct QrShapeContract {
    /// Original homogeneous operator contract.
    input: CheckedShape,
    /// Proposed dimensionless Q shape.
    orthogonal: CheckedShape,
    /// Proposed R shape carrying the input's physical coefficient kind.
    triangular: CheckedShape,
    /// Checked composition envelope with retained ordered contraction evidence.
    reconstruction: CheckedShape,
}

impl QrShapeContract {
    /// Check dimensionless Q, input-typed R and every intermediate-axis contraction.
    ///
    /// # Errors
    /// Refuses heterogeneous entries, wrong scalar kinds, intermediate axes or limits.
    pub fn check(
        input: &CheckedShape,
        orthogonal: &CheckedShape,
        triangular: &CheckedShape,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        input.same_snapshot(orthogonal)?;
        input.same_snapshot(triangular)?;
        let quantity = homogeneous(input)?;
        let ratio = Descriptor::ratio();
        require_kind(orthogonal, &ratio)?;
        require_kind(triangular, &quantity)?;
        let reconstruction = orthogonal.product(triangular, limits)?;
        input.domain.same_coordinates(&reconstruction.domain)?;
        input.codomain.same_coordinates(&reconstruction.codomain)?;
        require_kind(&reconstruction, &quantity)?;
        Ok(Self {
            input: input.clone(),
            orthogonal: orthogonal.clone(),
            triangular: triangular.clone(),
            reconstruction,
        })
    }

    /// Original homogeneous operator.
    #[must_use]
    pub const fn input(&self) -> &CheckedShape {
        &self.input
    }
    /// Dimensionless factor envelope, without an orthogonality claim.
    #[must_use]
    pub const fn orthogonal(&self) -> &CheckedShape {
        &self.orthogonal
    }
    /// Input-typed factor envelope, without a numerical triangularity claim.
    #[must_use]
    pub const fn triangular(&self) -> &CheckedShape {
        &self.triangular
    }
    /// Explicit intermediate coordinate identities, retained by both factors.
    #[must_use]
    pub const fn intermediate(&self) -> &Axis {
        &self.orthogonal.domain
    }
    /// Composition envelope; equality of numerical entries remains unproved.
    #[must_use]
    pub const fn reconstruction(&self) -> &CheckedShape {
        &self.reconstruction
    }
}

/// Bindings-admitted half-dimension scalar descriptor for a homogeneous Cholesky factor.
///
/// This admits a scalar root kind only. Positive definiteness, a physical metric,
/// factor axes and numerical Cholesky evidence remain independent requirements.
#[derive(Clone, Debug)]
pub struct CholeskyRootContract {
    /// Original homogeneous input, retaining the exponent and kind policy snapshot.
    input: CheckedShape,
    /// Scalar root admitted by that frozen registry.
    root: Descriptor,
}

impl CholeskyRootContract {
    /// Request the half-power through bindings' explicit exponent and semantic kind rules.
    ///
    /// # Errors
    /// Refuses heterogeneous input, fractional-dimension policy or an unregistered root kind.
    pub fn check(input: &CheckedShape) -> Result<Self, ShapeError> {
        let quantity = homogeneous(input)?;
        let coherent = Measure::coherent();
        let twice = Conversion {
            scale: coherent.scale().clone(),
            shift: coherent.scale().clone(),
        }
        .apply(coherent.rational_scale()?)?;
        let double = Measure::new(
            "root-exponent-denominator",
            Scale::Exact(twice),
            coherent.offset().clone(),
        )?;
        let half = coherent.conversion_to(&double, Role::Difference)?;
        let root = input
            .declarations
            .registry()
            .power(&quantity, half.rational_scale()?)?;
        Ok(Self {
            input: input.clone(),
            root,
        })
    }

    /// Original homogeneous operator and its unchanged exponent policy.
    #[must_use]
    pub const fn input(&self) -> &CheckedShape {
        &self.input
    }
    /// Admitted scalar root descriptor; it is not a matrix square-root guarantee.
    #[must_use]
    pub const fn root(&self) -> &Descriptor {
        &self.root
    }
}

/// Check scalar identity as well as a common physical dimension.
fn require_kind(shape: &CheckedShape, quantity: &Descriptor) -> Result<(), ShapeError> {
    for entry in &shape.entries {
        let actual = entry.scalar_descriptor().ok_or(ShapeError::Unsupported(
            "factor needs concrete scalar descriptors",
        ))?;
        quantity.conversion_to(actual)?;
    }
    Ok(())
}

/// A homogeneous factor convention cannot invent a common unit for heterogeneous entries.
fn homogeneous(shape: &CheckedShape) -> Result<Descriptor, ShapeError> {
    let quantity = shape
        .entries
        .first()
        .and_then(super::EntryContract::scalar_descriptor)
        .ok_or(ShapeError::Unsupported(
            "factor needs a nonempty concrete homogeneous shape",
        ))?
        .coherent();
    require_kind(shape, &quantity)?;
    Ok(quantity)
}
