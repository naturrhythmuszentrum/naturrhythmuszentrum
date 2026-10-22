// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine structural expansions and their independently checked metric contract.

use super::{Axis, CheckedShape, ShapeError, ShapeLimits, StructureScope};
use crate::Extent;
use nrz_bindings::{Conversion, Descriptor, Measure, PairingWitness, Role};

/// A typed exact source-free scalar definition, never a heap coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCoefficient {
    /// Physical coefficient kind in coherent difference coordinates.
    descriptor: Descriptor,
    /// Exact coefficient in `scale`; `shift` is always zero.
    value: Conversion,
}

impl ExactCoefficient {
    /// Define a scalar constant by a zero-shift exact map factor in the supplied measure.
    ///
    /// Signed or zero factors are permitted. This schema-only constructor makes no
    /// assertion about a numerical operand; uncertain coefficients require propagation.
    /// # Errors
    /// Refuses point/count/relative descriptors, nonzero shifts or exact arithmetic failure.
    pub fn new(descriptor: &Descriptor, definition: &Conversion) -> Result<Self, ShapeError> {
        if descriptor.role() != Role::Difference
            || descriptor.kind().count_domain().is_some()
            || descriptor.kind().is_relative()
            || !definition.shift.is_zero()
        {
            return Err(ShapeError::Reference);
        }
        let value = descriptor
            .conversion_to(&descriptor.coherent())?
            .apply(&definition.scale)?;
        Ok(Self {
            descriptor: descriptor.coherent(),
            value: Conversion {
                scale: value,
                shift: Measure::coherent().offset().clone(),
            },
        })
    }

    /// Define exactly one unit of this scalar descriptor's difference measure.
    ///
    /// # Errors
    /// Refuses unsupported descriptors or exact conversion failure.
    pub fn unit(descriptor: &Descriptor) -> Result<Self, ShapeError> {
        let coherent = Measure::coherent();
        Self::new(
            descriptor,
            &coherent.conversion_to(&coherent, Role::Difference)?,
        )
    }

    /// Define a typed exact structural zero.
    ///
    /// # Errors
    /// Refuses unsupported descriptors.
    pub fn zero(descriptor: &Descriptor) -> Result<Self, ShapeError> {
        Self::new(descriptor, &zero_map())
    }

    /// Coherent physical scalar descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Exact constant factor with zero shift; numerical lowering remains unimplemented.
    #[must_use]
    pub const fn exact_map(&self) -> &Conversion {
        &self.value
    }
}

/// One nonzero or explicitly zero coefficient in an expansion row.
#[derive(Clone, Debug)]
pub struct ExpansionTerm {
    /// Free-coordinate index, preserving that coordinate's identity.
    coordinate: usize,
    /// Typed source-free coefficient definition.
    coefficient: ExactCoefficient,
}

impl ExpansionTerm {
    /// Propose a coefficient for one free coordinate; the expansion checks its meaning.
    #[must_use]
    pub const fn new(coordinate: usize, coefficient: ExactCoefficient) -> Self {
        Self {
            coordinate,
            coefficient,
        }
    }
}

/// One affine expansion row; omitted coefficients and offset mean typed structural zero.
#[derive(Clone, Debug)]
pub struct ExpansionRow {
    /// Explicit coefficients; duplicate coordinate positions are refused.
    terms: Vec<ExpansionTerm>,
    /// Optional exact physical offset.
    offset: Option<ExactCoefficient>,
}

impl ExpansionRow {
    /// Propose a row of `vec(A) = T z + d` without numerical data.
    #[must_use]
    pub const fn new(terms: Vec<ExpansionTerm>, offset: Option<ExactCoefficient>) -> Self {
        Self { terms, offset }
    }
}

/// A checked coefficient or formal typed zero in the full logical expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpansionEntry {
    /// Expected output-entry over free-coordinate relation.
    pairing: PairingWitness,
    /// Exact factor in the quotient's coordinates, with zero shift.
    value: Conversion,
    /// Supplied original scalar definition, absent for a formal structural zero.
    definition: Option<ExactCoefficient>,
}

impl ExpansionEntry {
    /// Typed quotient even at structurally absent positions.
    #[must_use]
    pub const fn pairing(&self) -> &PairingWitness {
        &self.pairing
    }
    /// Exact coefficient in the checked quotient coordinates.
    #[must_use]
    pub const fn exact_map(&self) -> &Conversion {
        &self.value
    }
    /// Supplied typed definition, if this coefficient was explicitly declared.
    #[must_use]
    pub const fn definition(&self) -> Option<&ExactCoefficient> {
        self.definition.as_ref()
    }
}

/// A requested norm interpretation for free-coordinate differences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpansionNorm {
    /// Full logical Frobenius norm, using the exact induced metric `T^T T`.
    LogicalFrobenius,
    /// Unweighted free-coordinate Euclidean norm, admitted only when `T^T T = I`.
    PackedEuclidean,
}

/// Exact Gram matrix for differences in an admitted homogeneous expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpansionMetric {
    /// Square free-coordinate extent.
    extent: Extent,
    /// Exact zero-shift Gram factors in column order.
    entries: Vec<Conversion>,
    /// Physical difference quantity whose squared norm this metric measures.
    quantity: Descriptor,
    /// Admitted norm request.
    norm: ExpansionNorm,
}

impl ExpansionMetric {
    /// Free-coordinate square extent.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Exact `T^T T` entries; no rounded numerical Gram matrix is substituted.
    #[must_use]
    pub fn entries(&self) -> &[Conversion] {
        &self.entries
    }
    /// Common coherent difference descriptor of the logical entries.
    #[must_use]
    pub const fn quantity(&self) -> &Descriptor {
        &self.quantity
    }
    /// Requested norm that this exact Gram matrix admits.
    #[must_use]
    pub const fn norm(&self) -> ExpansionNorm {
        self.norm
    }
}

/// Bounded exact affine expansion retaining free coordinates and every logical entry.
#[derive(Clone, Debug)]
pub struct StructuredExpansion {
    /// Original logical matrix and its entry identities.
    logical: CheckedShape,
    /// Ordered independent coordinates, not independent random sources.
    free: Axis,
    /// `logical_entry_count` rows and `free_coordinate_count` columns.
    extent: Extent,
    /// Full logical expansion matrix, including typed structural zeros.
    entries: Vec<ExpansionEntry>,
    /// Exact physical offsets, including explicit typed zeros.
    offsets: Vec<ExactCoefficient>,
    /// Whether companions must obey this expansion.
    scope: StructureScope,
}

impl StructuredExpansion {
    /// Check a source-free affine expansion and every supplied scalar coefficient.
    ///
    /// The logical shape needs concrete scalar entry descriptors. Formal inverse/solve
    /// quotient entries require an explicitly admitted scalar schema before expansion.
    /// # Errors
    /// Refuses wrong coefficients, duplicate positions, incompatible offsets or limits.
    pub fn check(
        logical: &CheckedShape,
        free: Axis,
        rows: &[ExpansionRow],
        scope: StructureScope,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let extent = limits.extent(logical.entries.len(), free.entries.len())?;
        if rows.len() != extent.rows() {
            return Err(ShapeError::LengthMismatch);
        }
        if scope == StructureScope::WholeFamily && logical.scope != scope {
            return Err(ShapeError::StructureScope);
        }
        super::assembly::check_axes(&free, &free, &logical.declarations)?;
        let work = rows.iter().try_fold(extent.entries(), |total, row| {
            total
                .checked_add(row.terms.len())
                .ok_or(ShapeError::ExtentOverflow)
        })?;
        limits.check_work(work)?;
        let mut entries = initial_entries(logical, &free)?;
        let mut offsets = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            check_row(logical, extent, &mut entries, index, row)?;
            let quantity = logical_quantity(logical, index)?;
            let offset = row
                .offset
                .clone()
                .map_or_else(|| ExactCoefficient::zero(quantity), Ok)?;
            quantity.difference().compatible_kind(offset.descriptor())?;
            check_structural_zero(logical, extent, &entries, index, &offset)?;
            offsets.push(offset);
        }
        Ok(Self {
            logical: logical.clone(),
            free,
            extent,
            entries,
            offsets,
            scope,
        })
    }

    /// Original logical shape; covariance still has its full logical entry count.
    #[must_use]
    pub const fn logical(&self) -> &CheckedShape {
        &self.logical
    }
    /// Ordered free-coordinate identities; equality never implies source independence.
    #[must_use]
    pub const fn free_axis(&self) -> &Axis {
        &self.free
    }
    /// Full expansion extent, before any storage packing.
    #[must_use]
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// All expansion coefficients in logical column order.
    #[must_use]
    pub fn entries(&self) -> &[ExpansionEntry] {
        &self.entries
    }
    /// Exact affine offsets in logical entry order.
    #[must_use]
    pub fn offsets(&self) -> &[ExactCoefficient] {
        &self.offsets
    }
    /// Scope of this exact structural declaration.
    #[must_use]
    pub const fn scope(&self) -> StructureScope {
        self.scope
    }

    /// Authorize the same expansion of perturbations and both covariance axes.
    ///
    /// # Errors
    /// Refuses center-only structure; it cannot compress uncertain coordinates.
    pub fn companion_expansion(&self) -> Result<&Self, ShapeError> {
        if self.scope == StructureScope::WholeFamily {
            Ok(self)
        } else {
            Err(ShapeError::StructureScope)
        }
    }

    /// Derive the exact metric for `delta vec(A) = T delta z`.
    ///
    /// The affine offset cancels only for differences. A center norm must also
    /// evaluate `d`. Heterogeneous physical norms require explicit normalization
    /// and are refused by this homogeneous metric constructor.
    /// # Errors
    /// Refuses unlike physical quantities, an unproved packed isometry or work exhaustion.
    pub fn metric(
        &self,
        norm: ExpansionNorm,
        limits: ShapeLimits,
    ) -> Result<ExpansionMetric, ShapeError> {
        let quantity = metric_quantity(self)?;
        let extent = limits.extent(self.free.entries.len(), self.free.entries.len())?;
        limits.check_work(
            extent
                .entries()
                .checked_mul(self.extent.rows())
                .ok_or(ShapeError::ExtentOverflow)?,
        )?;
        let mut entries = Vec::with_capacity(extent.entries());
        for column in 0..extent.columns() {
            for row in 0..extent.rows() {
                entries.push(self.gram_entry(row, column)?);
            }
        }
        if norm == ExpansionNorm::PackedEuclidean {
            check_identity_metric(extent, &entries)?;
        }
        Ok(ExpansionMetric {
            extent,
            entries,
            quantity,
            norm,
        })
    }

    /// Exact sum of products of descriptor constants, with no numerical data involved.
    fn gram_entry(&self, row: usize, column: usize) -> Result<Conversion, ShapeError> {
        let mut sum = zero_map();
        for logical in 0..self.extent.rows() {
            let first = &self
                .entries
                .get(self.extent.index(logical, row)?)
                .ok_or(ShapeError::EntryIndex)?
                .value;
            let second = &self
                .entries
                .get(self.extent.index(logical, column)?)
                .ok_or(ShapeError::EntryIndex)?
                .value;
            let product = first.apply(&second.scale)?;
            sum.scale = Conversion {
                scale: Measure::coherent().scale().clone(),
                shift: sum.scale,
            }
            .apply(&product)?;
        }
        Ok(sum)
    }
}

/// Omitted coefficients are formal typed zeros, not untyped scalar literal shortcuts.
fn initial_entries(logical: &CheckedShape, free: &Axis) -> Result<Vec<ExpansionEntry>, ShapeError> {
    let mut entries = Vec::new();
    for input in &free.entries {
        for index in 0..logical.entries.len() {
            entries.push(ExpansionEntry {
                pairing: PairingWitness::sensitivity(
                    &logical_quantity(logical, index)?.coherent(),
                    input.scalar()?,
                )?,
                value: zero_map(),
                definition: None,
            });
        }
    }
    Ok(entries)
}

/// Check explicit coefficients once per row; duplicate declarations cannot erase originals.
fn check_row(
    logical: &CheckedShape,
    extent: Extent,
    entries: &mut [ExpansionEntry],
    row: usize,
    draft: &ExpansionRow,
) -> Result<(), ShapeError> {
    for term in &draft.terms {
        let entry = entries
            .get_mut(extent.index(row, term.coordinate)?)
            .ok_or(ShapeError::EntryIndex)?;
        if entry.definition.is_some() {
            return Err(ShapeError::Partition);
        }
        let conversion = entry.pairing.check_entry(
            logical.declarations.registry(),
            term.coefficient.descriptor(),
        )?;
        entry.value.scale = conversion.apply(&term.coefficient.value.scale)?;
        entry.definition = Some(term.coefficient.clone());
    }
    Ok(())
}

/// A scalar schema is needed to type both offsets and free-coordinate contractions.
fn logical_quantity(logical: &CheckedShape, index: usize) -> Result<&Descriptor, ShapeError> {
    logical
        .entries
        .get(index)
        .and_then(super::EntryContract::scalar_descriptor)
        .ok_or(ShapeError::Unsupported(
            "structured expansion requires concrete scalar entry descriptors",
        ))
}

/// An expansion cannot contradict a previously declared structural zero.
fn check_structural_zero(
    logical: &CheckedShape,
    extent: Extent,
    entries: &[ExpansionEntry],
    row: usize,
    offset: &ExactCoefficient,
) -> Result<(), ShapeError> {
    if logical
        .entries
        .get(row)
        .is_some_and(super::EntryContract::is_zero)
    {
        if !offset.value.scale.is_zero() {
            return Err(ShapeError::Partition);
        }
        for column in 0..extent.columns() {
            if !entries
                .get(extent.index(row, column)?)
                .ok_or(ShapeError::EntryIndex)?
                .value
                .scale
                .is_zero()
            {
                return Err(ShapeError::Partition);
            }
        }
    }
    Ok(())
}

/// A physical Frobenius norm requires one compatible difference quantity and unit factors.
fn metric_quantity(expansion: &StructuredExpansion) -> Result<Descriptor, ShapeError> {
    let quantity = logical_quantity(&expansion.logical, 0)?.coherent();
    for index in 0..expansion.logical.entries.len() {
        quantity
            .compatible_kind(logical_quantity(&expansion.logical, index)?)
            .map_err(|_| ShapeError::UnsupportedMetric)?;
    }
    for entry in &expansion.free.entries {
        let coordinate = entry.scalar()?;
        quantity
            .conversion_to(coordinate)
            .map_err(|_| ShapeError::UnsupportedMetric)?;
        if coordinate.measure().scale() != Measure::coherent().scale() {
            return Err(ShapeError::UnsupportedMetric);
        }
    }
    Ok(quantity)
}

/// Check the exact isometry identity rather than inferring it from packed storage.
fn check_identity_metric(extent: Extent, entries: &[Conversion]) -> Result<(), ShapeError> {
    let coherent = Measure::coherent();
    for (index, entry) in entries.iter().enumerate() {
        let expected = if index % extent.rows() == index / extent.rows() {
            coherent.scale()
        } else {
            coherent.offset()
        };
        if &entry.scale != expected {
            return Err(ShapeError::UnsupportedMetric);
        }
    }
    Ok(())
}

/// The exact zero map is metadata; no floating or heap coordinate enters this algebra.
fn zero_map() -> Conversion {
    let zero = Measure::coherent().offset().clone();
    Conversion {
        scale: zero.clone(),
        shift: zero,
    }
}
