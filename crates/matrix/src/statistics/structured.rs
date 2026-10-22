// SPDX-License-Identifier: AGPL-3.0-only
//! Checked whole-family expansions reuse the existing deterministic and statistical affine kernel.
//!
//! # Whole-family structured propagation
//!
//! `StructuredExpansion::linear_map` requires `WholeFamily` scope and a homogeneous free-coordinate column with H's exact axis identities. It exports signed and zero source-free coefficients through bindings and reuses the existing certified affine kernel, yielding the full logical entry vector and covariance `T U Tᵀ`. A checked entry transformation can reshape that vector into the original rectangle. Duplicate off-diagonal entries remain dependent. Center-only symmetry imposes no covariance reduction; existing full logical or proven-metric norm contracts remain unchanged. Whole-family structural zeros additionally require certified zero mean and variation.

use super::StatisticsError;
use crate::{
    Axis, AxisEntry, BlockDraft, CheckedShape, EntryDraft, EntryMap, Extent, FixedCoefficient,
    LinearMap, OperationError, Partition, RegionContract, ShapeDraft, ShapeLimits,
    StructuredExpansion,
};
use nrz_bindings::{BinaryOp, Conversion, Descriptor};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, Scalar};

impl StructuredExpansion {
    /// Build the certified affine map from free coordinates to the full logical entry vector.
    ///
    /// The output is column-vectorized; a checked entry transformation can restore the
    /// logical matrix rectangle. Free-axis identities, both covariance axes and exact
    /// offsets are retained. A center-only declaration cannot constrain random entries.
    /// # Errors
    /// Refuses center-only scope, foreign snapshots/axes, nonhomogeneous scalar schemas and limits.
    pub fn linear_map(
        &self,
        input: CheckedShape,
        literals: LiteralLimits,
        limits: ShapeLimits,
    ) -> Result<LinearMap, OperationError> {
        self.companion_expansion()?;
        self.free_axis().same_coordinates(input.codomain())?;
        if input.extent().columns() != 1
            || input.declarations().id() != self.logical().declarations().id()
        {
            return Err(StatisticsError::Contract.into());
        }
        let operand = RegionContract::check(input.clone())?;
        for coordinate in self.free_axis().entries() {
            if coordinate.quantity() != Some(operand.descriptor()) {
                return Err(StatisticsError::Contract.into());
            }
        }
        let flattened = flatten(self, &input, limits)?;
        let output = RegionContract::check(flattened.clone())?;
        let coefficient = self
            .logical()
            .declarations()
            .registry()
            .binary(BinaryOp::Divide, output.descriptor(), operand.descriptor())
            .map_err(crate::ShapeError::from)?;
        let operator = operator(self, &flattened, &coefficient, limits)?;
        let coefficients = self
            .entries()
            .iter()
            .map(|entry| FixedCoefficient {
                quantity: coefficient.clone(),
                value: crate::kernels::linear::CoefficientValue::Expansion(Box::new(
                    entry.exact_map().clone(),
                )),
            })
            .collect();
        let offsets = self
            .offsets()
            .iter()
            .map(|entry| FixedCoefficient {
                quantity: entry.descriptor().clone(),
                value: crate::kernels::linear::CoefficientValue::Expansion(Box::new(
                    entry.exact_map().clone(),
                )),
            })
            .collect();
        let mut map = LinearMap::new(operator, input, coefficients, offsets, literals, limits)?;
        map.output = output;
        Ok(map)
    }
}
/// Flatten through the original logical shape, retaining its parent and exact entry permutation.
fn flatten(
    expansion: &StructuredExpansion,
    input: &CheckedShape,
    limits: ShapeLimits,
) -> Result<CheckedShape, OperationError> {
    let count = expansion.logical().extent().entries();
    let mut entries = Vec::new();
    for (index, entry) in expansion.logical().entries().iter().enumerate() {
        let quantity = entry.scalar_descriptor().ok_or(StatisticsError::Contract)?;
        entries.push(AxisEntry::new(
            format!("logical-entry-{index}"),
            quantity.clone(),
        )?);
    }
    let codomain = Axis::new(entries, Partition::whole(count, limits)?, limits)?;
    let map = EntryMap::reshape(expansion.logical().extent(), Extent::new(count, 1)?, limits)?;
    Ok(expansion
        .logical()
        .select(input.domain().clone(), codomain, map, limits)?)
}
/// Recheck every exact coefficient against the retained physical quotient before kernel construction.
fn operator(
    expansion: &StructuredExpansion,
    flattened: &CheckedShape,
    coefficient: &Descriptor,
    limits: ShapeLimits,
) -> Result<CheckedShape, OperationError> {
    let mut entries = Vec::new();
    for entry in expansion.entries() {
        entry
            .pairing()
            .check_entry(expansion.logical().declarations().registry(), coefficient)?;
        entries.push(if entry.exact_map().scale.is_zero() {
            EntryDraft::Zero(coefficient.clone())
        } else {
            EntryDraft::Declared(coefficient.clone())
        });
    }
    let extent = expansion.extent();
    let blocks = if extent.entries() == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(
            0..extent.rows(),
            0..extent.columns(),
            entries,
        )]
    };
    Ok(ShapeDraft::new(
        expansion.free_axis().clone(),
        flattened.codomain().clone(),
        blocks,
        crate::StructureScope::WholeFamily,
    )
    .check(
        std::rc::Rc::clone(&expansion.logical().declarations),
        limits,
    )?)
}
/// Decode only exact structural metadata through bindings' bounded export and scalar's outward parser.
pub(crate) fn lower_definition<P: Scalar>(
    definition: &Conversion,
    limits: LiteralLimits,
    context: &ArithmeticContext,
) -> Result<crate::kernels::lowering::Coefficient<P>, OperationError> {
    let spelling = definition.export_scale(&nrz_bindings::LimitsOverride::default())?;
    let decoded = NumericLiteral::new(&spelling, limits)?.decode(context)?;
    Ok(crate::kernels::lowering::Coefficient {
        center: decoded.center().get(),
        enclosure: decoded.enclosure(),
    })
}
