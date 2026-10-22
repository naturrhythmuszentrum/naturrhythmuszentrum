// SPDX-License-Identifier: AGPL-3.0-only
//! Native descriptor contracts retain operands, maps and formal axis reversals.

use super::{
    Axis, AxisAlignment, CheckedShape, EntryContract, EntryMap, ShapeError, ShapeLimits,
    ShapeRecipe, StructureScope,
};
use nrz_bindings::{BinaryOp, PairingWitness};
use std::rc::Rc;

impl CheckedShape {
    /// Derive a typed identity, including off-diagonal quotient zeros on unlike axes.
    ///
    /// # Errors
    /// Refuses non-difference coordinates, unsupported pairings or exhausted limits.
    pub fn identity(
        axis: &Axis,
        declarations: Rc<nrz_bindings::CheckedDeclarations>,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let extent = limits.extent(axis.entries.len(), axis.entries.len())?;
        limits.partitions(axis, axis)?;
        super::assembly::check_axes(axis, axis, &declarations)?;
        let mut entries = Vec::with_capacity(extent.entries());
        for (column, input) in axis.entries.iter().enumerate() {
            for (row, output) in axis.entries.iter().enumerate() {
                let mut entry = EntryContract::formal(output, input)?;
                entry.zero = row != column;
                if row == column {
                    let ratio = nrz_bindings::Descriptor::ratio();
                    entry.conversion =
                        Some(entry.pairing.check_entry(declarations.registry(), &ratio)?);
                    entry.scalar = Some(ratio);
                }
                entries.push(entry);
            }
        }
        Ok(Self {
            domain: axis.clone(),
            codomain: axis.clone(),
            extent,
            entries,
            declarations,
            scope: StructureScope::WholeFamily,
            recipe: ShapeRecipe::Identity,
            parents: Vec::new(),
            maps: Vec::new(),
            alignments: Vec::new(),
        })
    }

    /// Check an explicitly aligned sum and preserve the common partition refinement.
    ///
    /// # Errors
    /// Refuses foreign snapshots, wrong alignment endpoints, scalar meanings or limits.
    pub fn sum(
        &self,
        right: &Self,
        domain: &AxisAlignment,
        codomain: &AxisAlignment,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        self.same_snapshot(right)?;
        domain.validate(&self.domain, &right.domain)?;
        codomain.validate(&self.codomain, &right.codomain)?;
        limits.extent(self.extent.rows(), self.extent.columns())?;
        let mut indices = Vec::with_capacity(self.entries.len());
        let mut entries = Vec::with_capacity(self.entries.len());
        for column in domain.indices() {
            for row in codomain.indices() {
                indices.push(right.extent.index(*row, *column)?);
            }
        }
        for (left, index) in self.entries.iter().zip(&indices) {
            let other = right.entries.get(*index).ok_or(ShapeError::EntryIndex)?;
            check_sum_entry(left, other, self.declarations.registry())?;
            let mut entry = left.clone();
            entry.zero = left.zero && other.zero;
            entry.original = None;
            entry.evidence = None;
            if other.scalar.is_none() {
                entry.scalar = None;
                entry.conversion = None;
            }
            entries.push(entry);
        }
        let mut result = self.derived(
            self.domain.clone(),
            self.codomain.clone(),
            entries,
            ShapeRecipe::Sum,
            limits,
        )?;
        result.domain.partition = domain.refinement().clone();
        result.codomain.partition = codomain.refinement().clone();
        limits.partitions(&result.domain, &result.codomain)?;
        result.parents.push(Rc::new(right.clone()));
        result
            .maps
            .push(EntryMap::new(right.extent, self.extent, indices, limits)?);
        result.alignments = vec![domain.clone(), codomain.clone()];
        result.scope = common_scope(self.scope, right.scope);
        Ok(result)
    }

    /// Compose `self: X -> Y` with `right: W -> X` in the declared order.
    ///
    /// # Errors
    /// Refuses reordered contractions, unregistered concrete scalar products or limits.
    pub fn product(&self, right: &Self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        self.same_snapshot(right)?;
        self.domain.same_coordinates(&right.codomain)?;
        let extent = limits.extent(self.extent.rows(), right.extent.columns())?;
        let work = extent
            .entries()
            .checked_mul(self.extent.columns())
            .ok_or(ShapeError::ExtentOverflow)?;
        limits.check_work(work)?;
        let mut entries = Vec::with_capacity(extent.entries());
        for input in &right.domain.entries {
            for output in &self.codomain.entries {
                entries.push(EntryContract::formal(output, input)?);
            }
        }
        for column in 0..extent.columns() {
            for row in 0..extent.rows() {
                let result = entries
                    .get_mut(extent.index(row, column)?)
                    .ok_or(ShapeError::EntryIndex)?;
                self.contract_entry(right, row, column, result)?;
            }
        }
        let mut result = self.derived(
            right.domain.clone(),
            self.codomain.clone(),
            entries,
            ShapeRecipe::Product,
            limits,
        )?;
        result.parents.push(Rc::new(right.clone()));
        result.scope = common_scope(self.scope, right.scope);
        Ok(result)
    }

    /// Permute entries and derive reciprocal dual axes: `A^T: Y* -> X*`.
    ///
    /// This is not an inverse and claims neither a physical adjoint nor invertibility.
    /// # Errors
    /// Refuses a failed reciprocal pairing or work exhaustion.
    pub fn dual_transpose(&self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        let map = EntryMap::transpose(self.extent, limits)?;
        let entries = map
            .indices()
            .iter()
            .map(|index| {
                self.entries
                    .get(*index)
                    .cloned()
                    .ok_or(ShapeError::EntryIndex)
            })
            .collect::<Result<_, _>>()?;
        let mut result = self.derived(
            self.codomain.dual()?,
            self.domain.dual()?,
            entries,
            ShapeRecipe::DualTranspose,
            limits,
        )?;
        result.maps.push(map);
        Ok(result)
    }

    /// Check only the nonempty square solve axes and derive `Z: W -> X`.
    ///
    /// Invertibility and numerical certification remain separate obligations.
    /// # Errors
    /// Refuses empty, rectangular or incompatible right-hand-side contracts.
    pub fn solve(&self, rhs: &Self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        self.same_snapshot(rhs)?;
        self.square()?;
        if rhs.extent.columns() == 0 {
            return Err(ShapeError::SolveExtent);
        }
        self.codomain.same_coordinates(&rhs.codomain)?;
        let mut result = self.reversed(rhs.domain.clone(), ShapeRecipe::Solve, limits)?;
        result.parents.push(Rc::new(rhs.clone()));
        Ok(result)
    }

    /// Check the square inverse envelope `A^-1: Y -> X`, without a numerical inverse.
    ///
    /// # Errors
    /// Refuses empty or rectangular operators and unsupported formal scalar axes.
    pub fn inverse(&self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        self.square()?;
        self.reversed(self.codomain.clone(), ShapeRecipe::Inverse, limits)
    }

    /// Select or reshape logical entries, checking every supplied new axis quotient.
    ///
    /// Repeated indices retain the same parent entry; no independence is inferred.
    /// # Errors
    /// Refuses mismatched maps, incompatible new axes or unsupported formal entries.
    pub fn select(
        &self,
        domain: Axis,
        codomain: Axis,
        map: EntryMap,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let extent = limits.extent(codomain.entries.len(), domain.entries.len())?;
        limits.partitions(&domain, &codomain)?;
        super::assembly::check_axes(&domain, &codomain, &self.declarations)?;
        if map.source() != self.extent || map.target() != extent {
            return Err(ShapeError::LengthMismatch);
        }
        let mut entries = Vec::with_capacity(extent.entries());
        for (position, index) in map.indices().iter().enumerate() {
            let mut entry = self
                .entries
                .get(*index)
                .cloned()
                .ok_or(ShapeError::EntryIndex)?;
            let output = codomain
                .entries
                .get(position % extent.rows())
                .ok_or(ShapeError::EntryIndex)?;
            let input = domain
                .entries
                .get(position / extent.rows())
                .ok_or(ShapeError::EntryIndex)?;
            let pairing = PairingWitness::sensitivity(output.scalar()?, input.scalar()?)?;
            let scalar = entry
                .scalar
                .as_ref()
                .ok_or(ShapeError::Unsupported("selection of a formal quotient"))?;
            entry.conversion = Some(pairing.check_entry(self.declarations.registry(), scalar)?);
            entry.pairing = pairing;
            entries.push(entry);
        }
        let mut result = self.derived(domain, codomain, entries, ShapeRecipe::Selection, limits)?;
        result.maps.push(map);
        Ok(result)
    }

    /// Keep every scalar summand's multiplication before zero elimination.
    fn contract_entry(
        &self,
        right: &Self,
        row: usize,
        column: usize,
        result: &mut EntryContract,
    ) -> Result<(), ShapeError> {
        result.zero = true;
        let mut formal = false;
        for contracted in 0..self.extent.columns() {
            let left = self.entry(row, contracted)?;
            let other = right.entry(contracted, column)?;
            if let (Some(first), Some(second)) = (&left.scalar, &other.scalar) {
                let term =
                    self.declarations
                        .registry()
                        .binary(BinaryOp::Multiply, first, second)?;
                result
                    .pairing
                    .check_entry(self.declarations.registry(), &term)?;
                if let Some(previous) = &result.scalar {
                    self.declarations
                        .registry()
                        .binary(BinaryOp::Add, previous, &term)?;
                }
                result.scalar = Some(term);
            } else {
                formal = true;
            }
            result.zero &= left.zero || other.zero;
        }
        if formal {
            result.scalar = None;
        }
        if let Some(quantity) = &result.scalar {
            result.conversion = Some(
                result
                    .pairing
                    .check_entry(self.declarations.registry(), quantity)?,
            );
        }
        Ok(())
    }

    /// Derive solution or inverse entries without inventing a named quotient kind.
    fn reversed(
        &self,
        domain: Axis,
        recipe: ShapeRecipe,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let extent = limits.extent(self.domain.entries.len(), domain.entries.len())?;
        let mut entries = Vec::with_capacity(extent.entries());
        for input in &domain.entries {
            for output in &self.domain.entries {
                entries.push(EntryContract::formal(output, input)?);
            }
        }
        let mut result = self.derived(domain, self.domain.clone(), entries, recipe, limits)?;
        result.scope = StructureScope::CenterOnly;
        Ok(result)
    }

    /// Refuse unsupported numerical domain shapes even if there are no entries to check.
    const fn square(&self) -> Result<(), ShapeError> {
        if self.extent.rows() == 0 || self.extent.rows() != self.extent.columns() {
            Err(ShapeError::SolveExtent)
        } else {
            Ok(())
        }
    }

    /// Descriptor rules and original declarations must have one immutable meaning.
    pub(crate) fn same_snapshot(&self, other: &Self) -> Result<(), ShapeError> {
        if self.declarations.id() == other.declarations.id() {
            Ok(())
        } else {
            Err(ShapeError::SnapshotMismatch)
        }
    }

    /// Construct only after the operation has established every output entry's relation.
    fn derived(
        &self,
        domain: Axis,
        codomain: Axis,
        entries: Vec<EntryContract>,
        recipe: ShapeRecipe,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let extent = limits.extent(codomain.entries.len(), domain.entries.len())?;
        limits.partitions(&domain, &codomain)?;
        if entries.len() != extent.entries() {
            return Err(ShapeError::LengthMismatch);
        }
        Ok(Self {
            domain,
            codomain,
            extent,
            entries,
            declarations: Rc::clone(&self.declarations),
            scope: self.scope,
            recipe,
            parents: vec![Rc::new(self.clone())],
            maps: Vec::new(),
            alignments: Vec::new(),
        })
    }
}

/// Formal quotients use their pairing sides; concrete kinds must additionally admit addition.
fn check_sum_entry(
    left: &EntryContract,
    right: &EntryContract,
    registry: &nrz_bindings::Registry,
) -> Result<(), ShapeError> {
    if left.dimension() != right.dimension() {
        return Err(nrz_bindings::UnitError::DimensionMismatch.into());
    }
    if let (Some(first), Some(second)) = (&left.scalar, &right.scalar) {
        registry.binary(BinaryOp::Add, first, second)?;
    }
    Ok(())
}

/// A whole-family claim survives only if both operands supplied that scope.
const fn common_scope(left: StructureScope, right: StructureScope) -> StructureScope {
    if matches!(
        (left, right),
        (StructureScope::WholeFamily, StructureScope::WholeFamily)
    ) {
        StructureScope::WholeFamily
    } else {
        StructureScope::CenterOnly
    }
}
