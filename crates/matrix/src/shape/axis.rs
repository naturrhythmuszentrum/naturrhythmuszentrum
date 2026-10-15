// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered coordinate identities and formal reciprocal duals.

use super::{Partition, ShapeError, ShapeLimits};
use nrz_units::{Conversion, Descriptor, Dimension, Measure, PairingWitness, Role};
use std::rc::Rc;

/// A coordinate identity, compared by ownership rather than its display label.
#[derive(Clone, Debug)]
struct Identity(Rc<()>);

impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Identity {}

/// One declared coordinate and its scalar or formal-dual interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxisEntry {
    /// Clones retain identity; independent declarations never compare equal.
    identity: Identity,
    /// Display text, not an identity lookup key.
    name: String,
    /// Original scalar quantity, retained even for its formal dual.
    quantity: Descriptor,
    /// Reciprocal difference pairing when this is a dual coordinate.
    dual: Option<PairingWitness>,
}

impl AxisEntry {
    /// Declare a fresh coordinate; clone it to preserve identity in another axis.
    ///
    /// # Errors
    /// Refuses an empty coordinate label.
    pub fn new(name: impl Into<String>, quantity: Descriptor) -> Result<Self, ShapeError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ShapeError::EmptyName);
        }
        Ok(Self {
            identity: Identity(Rc::new(())),
            name,
            quantity,
            dual: None,
        })
    }

    /// Human-readable coordinate label.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Scalar descriptor, unavailable for a formal dual without a scalar kind.
    #[must_use]
    pub const fn quantity(&self) -> Option<&Descriptor> {
        if self.dual.is_some() {
            None
        } else {
            Some(&self.quantity)
        }
    }

    /// Original quantity to which a dual pairs.
    #[must_use]
    pub const fn primal_quantity(&self) -> &Descriptor {
        &self.quantity
    }

    /// Exact dimension, reciprocal for a dual coordinate.
    #[must_use]
    pub fn dimension(&self) -> &Dimension {
        self.dual
            .as_ref()
            .map_or_else(|| self.quantity.dimension(), PairingWitness::dimension)
    }

    /// Exact measure, reciprocal for a dual coordinate.
    #[must_use]
    pub fn measure(&self) -> &Measure {
        self.dual
            .as_ref()
            .map_or_else(|| self.quantity.measure(), PairingWitness::measure)
    }

    /// Formal reciprocal evidence, without registering a scalar reciprocal kind.
    #[must_use]
    pub const fn dual_pairing(&self) -> Option<&PairingWitness> {
        self.dual.as_ref()
    }

    /// Change presentation while retaining this coordinate's identity.
    ///
    /// # Errors
    /// Refuses a different kind or role and presentation changes to formal duals.
    pub fn in_coordinates(&self, quantity: Descriptor) -> Result<Self, ShapeError> {
        self.scalar()?.conversion_to(&quantity)?;
        Ok(Self {
            quantity,
            ..self.clone()
        })
    }

    /// Remove a point role explicitly while retaining coordinate identity.
    #[must_use]
    pub fn difference(&self) -> Self {
        Self {
            quantity: self.quantity.difference(),
            ..self.clone()
        }
    }

    /// Declare affine coordinates for this same physical coordinate identity.
    ///
    /// # Errors
    /// Refuses a non-point target, incompatible kind or formal dual coordinate.
    pub fn with_origin(&self, quantity: Descriptor) -> Result<Self, ShapeError> {
        if quantity.role() != Role::Point {
            return Err(ShapeError::Origin);
        }
        self.scalar()?
            .difference()
            .conversion_to(&quantity.difference())?;
        Ok(Self {
            quantity,
            ..self.clone()
        })
    }

    /// Construct the formal dual, or recover the original on a second dualization.
    pub(crate) fn reciprocal(&self) -> Result<Self, ShapeError> {
        if self.quantity.role() != Role::Difference {
            return Err(ShapeError::LinearAxis);
        }
        let dual = if self.dual.is_some() {
            None
        } else {
            Some(PairingWitness::sensitivity(
                &Descriptor::ratio(),
                &self.quantity,
            )?)
        };
        Ok(Self {
            dual,
            ..self.clone()
        })
    }

    /// Scalar-only contracts refuse a formal dual explicitly.
    pub(crate) fn scalar(&self) -> Result<&Descriptor, ShapeError> {
        self.quantity()
            .ok_or(ShapeError::Unsupported("scalar operation on a formal dual"))
    }

    /// Exact coordinate identity and role must precede a measure conversion.
    pub(crate) fn conversion_from(&self, source: &Self) -> Result<Conversion, ShapeError> {
        if self.identity != source.identity || self.dual.is_some() != source.dual.is_some() {
            return Err(ShapeError::AxisMismatch);
        }
        source.quantity.conversion_to(&self.quantity)?;
        Ok(source
            .measure()
            .conversion_to(self.measure(), Role::Difference)?)
    }
}

/// An ordered axis with complete compatible partitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Axis {
    /// Coordinate order is part of compatibility.
    pub(crate) entries: Vec<AxisEntry>,
    /// Block boundaries belong to this ordered coordinate sequence.
    pub(crate) partition: Partition,
}

impl Axis {
    /// Validate an ordered coordinate sequence and its partition length.
    ///
    /// # Errors
    /// Refuses an unmatched partition endpoint or exhausted axis limit.
    pub fn new(
        entries: Vec<AxisEntry>,
        partition: Partition,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        if entries.len() > limits.axis {
            return Err(ShapeError::Limit);
        }
        if partition.boundaries().last() != Some(&entries.len()) {
            return Err(ShapeError::Partition);
        }
        Ok(Self { entries, partition })
    }

    /// Ordered coordinate declarations.
    #[must_use]
    pub fn entries(&self) -> &[AxisEntry] {
        &self.entries
    }

    /// Complete block partition.
    #[must_use]
    pub const fn partition(&self) -> &Partition {
        &self.partition
    }

    /// Derive reciprocal dual coordinates with unchanged ordered identities.
    ///
    /// # Errors
    /// Refuses point axes or an unsupported units pairing.
    pub fn dual(&self) -> Result<Self, ShapeError> {
        Ok(Self {
            entries: self
                .entries
                .iter()
                .map(AxisEntry::reciprocal)
                .collect::<Result<_, _>>()?,
            partition: self.partition.clone(),
        })
    }

    /// Explicit point-to-difference coordinate relation, preserving order.
    #[must_use]
    pub fn differences(&self) -> Self {
        Self {
            entries: self.entries.iter().map(AxisEntry::difference).collect(),
            partition: self.partition.clone(),
        }
    }

    /// Require ordered identities and exact descriptors, independent of partition refinement.
    pub(crate) fn same_coordinates(&self, other: &Self) -> Result<(), ShapeError> {
        if self.entries == other.entries {
            Ok(())
        } else {
            Err(ShapeError::AxisMismatch)
        }
    }
}
