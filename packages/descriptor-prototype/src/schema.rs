// SPDX-License-Identifier: AGPL-3.0-only
//! Exact semantic keys, deliberately independent of values and storage.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use nrz_units::{Descriptor, Registry};

use crate::{Error, error::identity};

/// Exact CAS descriptor equality with a deliberately coarse lawful hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuantityKey(
    /// The complete authoritative semantic descriptor.
    pub Descriptor,
);

impl Hash for QuantityKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.dimension().hash(state);
        self.0.measure().name().hash(state);
        self.0.measure().scale().hash(state);
        self.0.measure().offset().hash(state);
        std::mem::discriminant(&self.0.role()).hash(state);
        // Kind and count bounds are omitted; full equality resolves collisions.
    }
}

/// A quantity identity scoped to its immutable interner entries.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QuantityId {
    /// Owning pool identity.
    pool: u64,
    /// Append-only entry number.
    index: usize,
}

/// A shape identity scoped to its owning interner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ShapeId {
    /// Owning pool identity.
    pool: u64,
    /// Append-only entry number.
    index: usize,
}

/// One ordered logical coordinate, not a stored value identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Axis {
    /// Coordinate meaning, such as node voltage or resistor current.
    pub meaning: String,
    /// Coordinate quantity schema.
    pub quantity: QuantityId,
}

/// Dense rectangular scope with explicit column-major logical entry meanings.
///
/// The prototype supports only full dense structure and identity entry maps.
/// Storage layout and normalization maps deliberately do not enter this key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shape {
    /// Ordered row coordinates; the length is the row extent.
    pub rows: Vec<Axis>,
    /// Ordered column coordinates; the length is the column extent.
    pub columns: Vec<Axis>,
    /// Quantity of each logical entry, indexed by `row + row_count * column`.
    pub entries: Vec<QuantityId>,
}

/// Append-only schemas; registry admission is independent of schema identity.
#[derive(Debug)]
pub struct SchemaPool {
    /// Unique scope shared by all IDs minted here.
    id: u64,
    /// CAS registry used only to admit new quantity schemas.
    registry: Registry,
    /// Exact keys resolve coarse-hash collisions.
    quantities: HashMap<QuantityKey, QuantityId>,
    /// Stable index lookup.
    descriptors: Vec<Descriptor>,
    /// Interned logical shapes.
    shapes: HashMap<Shape, ShapeId>,
    /// Stable shape lookup.
    shape_values: Vec<Shape>,
}

impl SchemaPool {
    /// Create a pool with an owned registry snapshot.
    ///
    /// # Errors
    /// Refuses exhausted process identities.
    pub fn new(registry: Registry) -> Result<Self, Error> {
        Ok(Self {
            id: identity()?,
            registry,
            quantities: HashMap::new(),
            descriptors: Vec::new(),
            shapes: HashMap::new(),
            shape_values: Vec::new(),
        })
    }

    /// Admit and intern exact meaning; convertible measures may remain unequal.
    ///
    /// # Errors
    /// Preserves CAS admission failures, including rational exponent policy.
    pub fn quantity(&mut self, descriptor: Descriptor) -> Result<QuantityId, Error> {
        self.registry.admit(&descriptor)?;
        let key = QuantityKey(descriptor);
        if let Some(id) = self.quantities.get(&key) {
            return Ok(*id);
        }
        let id = QuantityId {
            pool: self.id,
            index: self.descriptors.len(),
        };
        self.descriptors.push(key.0.clone());
        self.quantities.insert(key, id);
        Ok(id)
    }

    /// Resolve an ID only in the scope that created it.
    ///
    /// # Errors
    /// Refuses foreign or absent IDs.
    pub fn descriptor(&self, id: QuantityId) -> Result<&Descriptor, Error> {
        if id.pool != self.id {
            return Err(Error::Schema);
        }
        self.descriptors.get(id.index).ok_or(Error::Schema)
    }

    /// Admit full dense shapes and intern their logical meanings.
    ///
    /// # Errors
    /// Refuses empty axes, foreign quantities, overflow and incorrect entry counts.
    pub fn shape(&mut self, shape: Shape) -> Result<ShapeId, Error> {
        if shape.rows.is_empty() || shape.columns.is_empty() {
            return Err(Error::Shape);
        }
        let size = shape
            .rows
            .len()
            .checked_mul(shape.columns.len())
            .ok_or(Error::Shape)?;
        if size != shape.entries.len() {
            return Err(Error::Shape);
        }
        for axis in shape.rows.iter().chain(&shape.columns) {
            if axis.meaning.is_empty() {
                return Err(Error::Shape);
            }
            self.descriptor(axis.quantity)?;
        }
        for quantity in &shape.entries {
            self.descriptor(*quantity)?;
        }
        if let Some(id) = self.shapes.get(&shape) {
            return Ok(*id);
        }
        let id = ShapeId {
            pool: self.id,
            index: self.shape_values.len(),
        };
        self.shape_values.push(shape.clone());
        self.shapes.insert(shape, id);
        Ok(id)
    }

    /// Resolve the immutable logical schema of a shape.
    ///
    /// # Errors
    /// Refuses foreign or absent IDs.
    pub fn shape_definition(&self, id: ShapeId) -> Result<&Shape, Error> {
        if id.pool != self.id {
            return Err(Error::Schema);
        }
        self.shape_values.get(id.index).ok_or(Error::Schema)
    }
}
