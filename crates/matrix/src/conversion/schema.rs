// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered checked scalar and formal quotient maps from one retained normalization schema.

use super::ConversionContract;
use super::definitions::{AxisDefinitions, DefinitionPool};
use crate::{CoordinateMap, EntryNormalization, NormalizationSchema, OperationError};
use nrz_bindings::{CheckedDeclarations, CheckedMap, LimitsOverride};

/// Separate center and difference maps for one formal physical entry.
#[derive(Clone, Debug)]
pub struct NormalizedEntryMaps {
    /// Affine center map, including a row origin where the schema declares one.
    centers: CheckedMap,
    /// Zero-shift map for perturbations; it cannot restore a point origin.
    differences: CheckedMap,
}

impl NormalizedEntryMaps {
    /// Exact center map; its inverse restores the same quotient and point origin.
    #[must_use]
    pub const fn centers(&self) -> &CheckedMap {
        &self.centers
    }
    /// Exact difference map, independent of the center origin.
    #[must_use]
    pub const fn differences(&self) -> &CheckedMap {
        &self.differences
    }
}

/// Exact checked normalization maps, retaining formal coordinates rather than inventing scalar kinds.
///
/// Scalar coordinate maps can enter `ConversionContract::new`. Formal entry maps
/// are descriptor evidence for block kernels, not permission to import a formal
/// quotient into homogeneous scalar storage. The retained schema records RHS
/// correction placement and products; exporting maps does not execute those products.
#[derive(Clone, Debug)]
pub struct NormalizationMaps {
    /// Original shape, definition identities and ordered affine correction recipe.
    schema: NormalizationSchema,
    /// Scalar variable coordinates X, in original axis order.
    domain: Vec<CheckedMap>,
    /// Scalar equation coordinates Y, in original axis order.
    codomain: Vec<CheckedMap>,
    /// Scalar RHS-column coordinates W, in original axis order.
    columns: Vec<CheckedMap>,
    /// Formal operator maps in logical column order.
    operator: Vec<NormalizedEntryMaps>,
    /// Formal RHS maps before operator-origin correction.
    rhs: Vec<NormalizedEntryMaps>,
    /// Formal solution maps, including point restoration through their inverses.
    solution: Vec<NormalizedEntryMaps>,
}

impl NormalizationMaps {
    /// Original checked schema, including unexecuted affine RHS correction terms.
    #[must_use]
    pub const fn schema(&self) -> &NormalizationSchema {
        &self.schema
    }
    /// Scalar variable-coordinate maps in X order.
    #[must_use]
    pub fn domain(&self) -> &[CheckedMap] {
        &self.domain
    }
    /// Scalar equation-coordinate maps in Y order.
    #[must_use]
    pub fn codomain(&self) -> &[CheckedMap] {
        &self.codomain
    }
    /// Scalar RHS-column maps in W order.
    #[must_use]
    pub fn columns(&self) -> &[CheckedMap] {
        &self.columns
    }
    /// Operator entry maps in logical column order.
    #[must_use]
    pub fn operator(&self) -> &[NormalizedEntryMaps] {
        &self.operator
    }
    /// RHS entry maps before the retained operator-origin correction.
    #[must_use]
    pub fn rhs(&self) -> &[NormalizedEntryMaps] {
        &self.rhs
    }
    /// Solution entry maps; inverse center maps restore declared point origins.
    #[must_use]
    pub fn solution(&self) -> &[NormalizedEntryMaps] {
        &self.solution
    }
}

impl ConversionContract {
    /// Export one schema's scalar axes and formal row/column entry maps through bindings.
    ///
    /// Definitions retain their H identity within this bundle and are independent of
    /// equal labels in frozen declarations. No stored coordinate enters bindings algebra.
    /// # Errors
    /// Preserves each bindings `MapError` cause, including exact arithmetic limits.
    pub fn from_normalization_schema(
        schema: &NormalizationSchema,
        limits: &LimitsOverride,
    ) -> Result<NormalizationMaps, OperationError> {
        let owner = &schema.operator().declarations;
        let mut pool = DefinitionPool::default();
        let domain = pool.axis(schema.domain())?;
        let codomain = pool.axis(schema.codomain())?;
        let columns = pool.axis(schema.columns())?;
        Ok(NormalizationMaps {
            schema: schema.clone(),
            domain: scalar_maps(owner, schema.domain(), &domain, limits)?,
            codomain: scalar_maps(owner, schema.codomain(), &codomain, limits)?,
            columns: scalar_maps(owner, schema.columns(), &columns, limits)?,
            operator: entry_maps(
                owner,
                schema.operator_maps(),
                &codomain,
                &domain,
                false,
                limits,
            )?,
            rhs: entry_maps(owner, schema.rhs_maps(), &codomain, &columns, true, limits)?,
            solution: entry_maps(
                owner,
                schema.solution_maps(),
                &domain,
                &columns,
                true,
                limits,
            )?,
        })
    }
}

/// Coherent physical scalar centers use the declared point origin and exact difference reference.
fn scalar_maps(
    owner: &CheckedDeclarations,
    coordinates: &CoordinateMap,
    definitions: &AxisDefinitions,
    limits: &LimitsOverride,
) -> Result<Vec<CheckedMap>, OperationError> {
    let mut maps = crate::binding::buffer(coordinates.axis().entries().len())?;
    for ((entry, reference), origin) in coordinates
        .axis()
        .entries()
        .iter()
        .zip(&definitions.references)
        .zip(&definitions.origins)
    {
        let descriptor = entry
            .quantity()
            .ok_or(OperationError::UnsupportedShape)?
            .coherent();
        maps.push(owner.schema_normalization_map(
            &descriptor,
            reference,
            origin.as_ref(),
            limits,
        )?);
    }
    Ok(maps)
}

/// Exact formal quotients preserve both ordered reference descriptors and separate point shifts.
fn entry_maps(
    owner: &CheckedDeclarations,
    entries: &[EntryNormalization],
    rows: &AxisDefinitions,
    columns: &AxisDefinitions,
    points: bool,
    limits: &LimitsOverride,
) -> Result<Vec<NormalizedEntryMaps>, OperationError> {
    let mut maps = crate::binding::buffer(entries.len())?;
    for (column, reference) in columns.references.iter().enumerate() {
        for (row, (numerator, origin)) in rows.references.iter().zip(&rows.origins).enumerate() {
            let index = column
                .checked_mul(rows.references.len())
                .and_then(|offset| offset.checked_add(row))
                .ok_or(OperationError::ResourceLimit)?;
            let entry = entries.get(index).ok_or(OperationError::InputCount)?;
            let differences = owner.quotient_normalization_map(
                entry.quotient(),
                numerator,
                reference,
                None,
                limits,
            )?;
            let centers = if points && origin.is_some() {
                owner.quotient_normalization_map(
                    entry.quotient(),
                    numerator,
                    reference,
                    origin.as_ref(),
                    limits,
                )?
            } else {
                differences.clone()
            };
            maps.push(NormalizedEntryMaps {
                centers,
                differences,
            });
        }
    }
    Ok(maps)
}
