// SPDX-License-Identifier: AGPL-3.0-only
//! Exact coordinate schemas; numerical lowering and evaluation belong to later kernels.

use crate::{Axis, AxisAlignment, CheckedShape, ShapeError, ShapeLimits};
use nrz_units::{
    Conversion, Descriptor, Kind, Measure, NamedReference, PairingWitness, Role, Scale,
};
use std::rc::Rc;

/// Definition identity is retained separately from equal names and exact constants.
#[derive(Clone, Debug)]
struct DefinitionId(Rc<()>);

impl PartialEq for DefinitionId {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for DefinitionId {}

/// Positive typed exact source-free scale, defined as one unit of its difference measure.
///
/// This constructor accepts an exact schema definition, never a value handle or an
/// uncertainty claim. An uncertain calibration scale must be a numerical operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceScale {
    /// Scoped declaration identity; cloning preserves it.
    identity: DefinitionId,
    /// Units-checked nonzero difference reference and its declared name.
    reference: NamedReference,
}

impl ReferenceScale {
    /// Define the positive coherent magnitude given by this exact difference measure's scale.
    ///
    /// `Measure` has already refused symbolic, zero and negative scales. The reference
    /// is one unit of that measure, so no positivity assumption about a heap value enters.
    /// # Errors
    /// Refuses points, counts, relative kinds, empty names or unsupported pairing kinds.
    pub fn new(name: impl Into<String>, definition: &Descriptor) -> Result<Self, ShapeError> {
        if definition.role() != Role::Difference
            || definition.kind().count_domain().is_some()
            || definition.kind().is_relative()
        {
            return Err(ShapeError::Reference);
        }
        PairingWitness::sensitivity(definition, definition)?;
        Ok(Self {
            identity: DefinitionId(Rc::new(())),
            reference: NamedReference::new(name, definition, true)?,
        })
    }

    /// Named typed exact definition; equal spellings do not merge definition identities.
    #[must_use]
    pub const fn reference(&self) -> &NamedReference {
        &self.reference
    }

    /// The positive exact scale is this descriptor's difference-measure scale.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        self.reference.descriptor()
    }
}

/// A typed exact point origin, defined as the zero coordinate of its declared measure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointOrigin {
    /// Distinct origin declarations stay distinct even at equal coordinates.
    identity: DefinitionId,
    /// Human-readable definition label.
    name: String,
    /// Point kind and coherent origin in the measure offset.
    descriptor: Descriptor,
}

impl PointOrigin {
    /// Define an exact source-free point origin from an existing point measure.
    ///
    /// # Errors
    /// Refuses non-point definitions or empty origin labels.
    pub fn new(name: impl Into<String>, definition: &Descriptor) -> Result<Self, ShapeError> {
        let name = name.into();
        if name.trim().is_empty() || definition.role() != Role::Point {
            return Err(ShapeError::Origin);
        }
        Ok(Self {
            identity: DefinitionId(Rc::new(())),
            name,
            descriptor: definition.clone(),
        })
    }

    /// Origin declaration label.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact point definition; its measure offset is the coherent origin.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}

/// An ordered physical axis with retained exact reference and origin definitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateMap {
    /// Original point or difference coordinate interpretations.
    axis: Axis,
    /// One positive exact reference per coordinate.
    references: Vec<ReferenceScale>,
    /// An exact origin exactly where the axis has a point role.
    origins: Vec<Option<PointOrigin>>,
    /// Coherent physical centers to normalized coordinates, subtracting origins first.
    normalize: Vec<Conversion>,
    /// Normalized centers to coherent physical coordinates, restoring point origins.
    restore: Vec<Conversion>,
}

impl CoordinateMap {
    /// Validate each reference and origin against its ordered axis coordinate.
    ///
    /// # Errors
    /// Refuses missing or extra definitions, wrong kinds or roles, formal duals and limits.
    pub fn check(
        axis: &Axis,
        references: Vec<ReferenceScale>,
        origins: Vec<Option<PointOrigin>>,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        limits.check_work(axis.entries().len())?;
        if axis.entries().len() != references.len() || references.len() != origins.len() {
            return Err(ShapeError::LengthMismatch);
        }
        let mut normalize = Vec::with_capacity(references.len());
        let mut restore = Vec::with_capacity(references.len());
        for ((entry, reference), origin) in axis.entries().iter().zip(&references).zip(&origins) {
            let quantity = entry.quantity().ok_or(ShapeError::LinearAxis)?;
            quantity
                .difference()
                .conversion_to(reference.descriptor())?;
            check_origin(quantity, origin.as_ref())?;
            let target = coordinate_measure(reference.descriptor().measure(), origin.as_ref())?;
            normalize.push(Measure::coherent().conversion_to(&target, quantity.role())?);
            restore.push(target.conversion_to(&Measure::coherent(), quantity.role())?);
        }
        Ok(Self {
            axis: axis.clone(),
            references,
            origins,
            normalize,
            restore,
        })
    }

    /// Original ordered axis; point roles and origins are retained.
    #[must_use]
    pub const fn axis(&self) -> &Axis {
        &self.axis
    }
    /// Positive exact reference definitions in axis order.
    #[must_use]
    pub fn references(&self) -> &[ReferenceScale] {
        &self.references
    }
    /// Declared origins, present only for point coordinates.
    #[must_use]
    pub fn origins(&self) -> &[Option<PointOrigin>] {
        &self.origins
    }
    /// Exact center maps from coherent physical coordinates, including point subtraction.
    #[must_use]
    pub fn normalization(&self) -> &[Conversion] {
        &self.normalize
    }
    /// Exact center restoration maps, including point origins.
    #[must_use]
    pub fn restoration(&self) -> &[Conversion] {
        &self.restore
    }

    /// Check identity and measure compatibility after explicit origin removal.
    fn align_to(&self, axis: &Axis, limits: ShapeLimits) -> Result<AxisAlignment, ShapeError> {
        AxisAlignment::identity(axis, &self.axis.differences(), limits)
    }
}

/// Two-sided exact normalization and restoration for one physical matrix entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryNormalization {
    /// Typed row-reference over column-reference quotient.
    quotient: PairingWitness,
    /// Coherent physical center to normalized center.
    normalize: Conversion,
    /// Normalized center to coherent physical center.
    restore: Conversion,
    /// Same normalization for differences, bounds and perturbations; shift is zero.
    difference_normalize: Conversion,
    /// Same restoration for differences; point origins never enter.
    difference_restore: Conversion,
}

impl EntryNormalization {
    /// Typed reference quotient defining this entry's physical scale.
    #[must_use]
    pub const fn quotient(&self) -> &PairingWitness {
        &self.quotient
    }
    /// Exact center normalization, including any declared point origin.
    #[must_use]
    pub const fn normalization(&self) -> &Conversion {
        &self.normalize
    }
    /// Exact center restoration, including any declared point origin.
    #[must_use]
    pub const fn restoration(&self) -> &Conversion {
        &self.restore
    }
    /// Zero-shift map for differences and numerical error, never statistical assurance.
    #[must_use]
    pub const fn difference_normalization(&self) -> &Conversion {
        &self.difference_normalize
    }
    /// Zero-shift restoration for perturbations; magnitude bounds use its positive scale.
    #[must_use]
    pub const fn difference_restoration(&self) -> &Conversion {
        &self.difference_restore
    }
}

/// Ordered affine RHS correction after point origins have first been subtracted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginCorrection {
    /// Add back this RHS origin before scaling the correction into normalized RHS coordinates.
    RightOrigin {
        /// Logical equation row.
        row: usize,
        /// Retained exact physical origin.
        origin: PointOrigin,
    },
    /// Subtract the physical operator entry times this solution origin.
    OperatorOrigin {
        /// Logical equation row.
        row: usize,
        /// Logical variable column in the physical operator.
        column: usize,
        /// Retained exact physical solution origin.
        origin: PointOrigin,
    },
}

/// One coherent solve coordinate system with exact operator, RHS and solution maps.
///
/// The schema establishes `A_hat = S_Y^-1 A S_X`, `B_hat = S_Y^-1 B S_W`
/// and `X_hat = S_X^-1 X S_W`, plus explicit affine origin corrections.
/// It owns no numerical coefficients, conversion-error bounds or solve certificate.
#[derive(Clone, Debug)]
pub struct NormalizationSchema {
    /// Original operator contract and its frozen evidence.
    operator: CheckedShape,
    /// Original right-hand-side contract.
    rhs: CheckedShape,
    /// Formal solution contract with reversed axes.
    solution: CheckedShape,
    /// Variable coordinates and exact restoration origins.
    domain: CoordinateMap,
    /// Equation coordinates and original RHS origins.
    codomain: CoordinateMap,
    /// Shared RHS-column reference coordinates.
    columns: CoordinateMap,
    /// All exact maps in operator logical column order.
    operator_maps: Vec<EntryNormalization>,
    /// All exact maps in RHS logical column order.
    rhs_maps: Vec<EntryNormalization>,
    /// All exact maps in solution logical column order.
    solution_maps: Vec<EntryNormalization>,
    /// Ordered equation adjustments following origin subtraction.
    corrections: Vec<OriginCorrection>,
    /// Validated coordinate identity and difference-measure alignments.
    alignments: Vec<AxisAlignment>,
}

impl NormalizationSchema {
    /// Check one shared X/Y/W reference system and retain every exact restoration map.
    ///
    /// Point origins are shared by each row across ratio RHS columns. Non-ratio
    /// column origins need an entry-origin schema and explicitly refuse here.
    /// # Errors
    /// Refuses incompatible axes, unsupported affine columns, invalid solves or limits.
    pub fn check(
        operator: &CheckedShape,
        rhs: &CheckedShape,
        domain: CoordinateMap,
        codomain: CoordinateMap,
        columns: CoordinateMap,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        let solution = operator.solve(rhs, limits)?;
        let alignments = vec![
            domain.align_to(operator.domain(), limits)?,
            codomain.align_to(operator.codomain(), limits)?,
            columns.align_to(rhs.domain(), limits)?,
        ];
        check_affine_columns(&domain, &codomain, &columns)?;
        let work = operator
            .extent()
            .entries()
            .checked_add(rhs.extent().entries())
            .and_then(|total| total.checked_add(solution.extent().entries()))
            .ok_or(ShapeError::ExtentOverflow)?;
        limits.check_work(work)?;
        let operator_maps = entry_maps(&codomain, &domain, false)?;
        let rhs_maps = entry_maps(&codomain, &columns, true)?;
        let solution_maps = entry_maps(&domain, &columns, true)?;
        let corrections = origin_corrections(&domain, &codomain, limits)?;
        Ok(Self {
            operator: operator.clone(),
            rhs: rhs.clone(),
            solution,
            domain,
            codomain,
            columns,
            operator_maps,
            rhs_maps,
            solution_maps,
            corrections,
            alignments,
        })
    }

    /// Checked physical operator, before normalization.
    #[must_use]
    pub const fn operator(&self) -> &CheckedShape {
        &self.operator
    }
    /// Checked physical right-hand side.
    #[must_use]
    pub const fn rhs(&self) -> &CheckedShape {
        &self.rhs
    }
    /// Descriptor-only solution envelope; it proves no numerical existence.
    #[must_use]
    pub const fn solution(&self) -> &CheckedShape {
        &self.solution
    }
    /// Retained variable references and point origins.
    #[must_use]
    pub const fn domain(&self) -> &CoordinateMap {
        &self.domain
    }
    /// Retained equation references and point origins.
    #[must_use]
    pub const fn codomain(&self) -> &CoordinateMap {
        &self.codomain
    }
    /// Retained RHS-column references.
    #[must_use]
    pub const fn columns(&self) -> &CoordinateMap {
        &self.columns
    }
    /// `S_Y^-1 A S_X` maps in logical column order.
    #[must_use]
    pub fn operator_maps(&self) -> &[EntryNormalization] {
        &self.operator_maps
    }
    /// `S_Y^-1 (B - O_Y) S_W` center maps, preceding the declared RHS correction.
    #[must_use]
    pub fn rhs_maps(&self) -> &[EntryNormalization] {
        &self.rhs_maps
    }
    /// Solution normalization and restoration; origins affect only centers.
    #[must_use]
    pub fn solution_maps(&self) -> &[EntryNormalization] {
        &self.solution_maps
    }
    /// Add `S_Y^-1 (O_Y - A O_X) S_W` after RHS-origin subtraction in each column.
    #[must_use]
    pub fn origin_corrections(&self) -> &[OriginCorrection] {
        &self.corrections
    }
    /// Axis identity and difference-measure evidence in X/Y/W order.
    #[must_use]
    pub fn alignments(&self) -> &[AxisAlignment] {
        &self.alignments
    }
}

/// Origins apply exactly to point coordinates, with matching physical kinds.
fn check_origin(quantity: &Descriptor, origin: Option<&PointOrigin>) -> Result<(), ShapeError> {
    match (quantity.role(), origin) {
        (Role::Difference, None) => Ok(()),
        (Role::Point, Some(origin)) => {
            quantity.conversion_to(origin.descriptor())?;
            Ok(())
        }
        _ => Err(ShapeError::Origin),
    }
}

/// Use an exact reference magnitude and a separately retained coherent origin.
fn coordinate_measure(
    reference: &Measure,
    origin: Option<&PointOrigin>,
) -> Result<Measure, ShapeError> {
    let offset = origin.map_or_else(
        || Measure::coherent().offset().clone(),
        |value| value.descriptor().measure().offset().clone(),
    );
    Ok(Measure::new(
        "normalized-reference",
        Scale::Exact(reference.scale().clone()),
        offset,
    )?)
}

/// Non-ratio affine columns need quotient-entry origins, rather than axis origins.
fn check_affine_columns(
    domain: &CoordinateMap,
    codomain: &CoordinateMap,
    columns: &CoordinateMap,
) -> Result<(), ShapeError> {
    if columns.origins.iter().any(Option::is_some) {
        return Err(ShapeError::Origin);
    }
    if domain
        .origins
        .iter()
        .chain(&codomain.origins)
        .any(Option::is_some)
        && columns.axis.entries().iter().any(|entry| {
            entry
                .quantity()
                .is_none_or(|quantity| quantity.kind() != &Kind::Ratio)
        })
    {
        return Err(ShapeError::Unsupported(
            "axis point origins require ratio RHS columns",
        ));
    }
    Ok(())
}

/// Exact row/column scale quotients are derived through units, never by heap arithmetic.
fn entry_maps(
    rows: &CoordinateMap,
    columns: &CoordinateMap,
    points: bool,
) -> Result<Vec<EntryNormalization>, ShapeError> {
    let mut maps = Vec::new();
    for input in &columns.references {
        for (output, origin) in rows.references.iter().zip(&rows.origins) {
            let quotient = PairingWitness::sensitivity(output.descriptor(), input.descriptor())?;
            let coherent = Measure::coherent();
            let difference_normalize =
                coherent.conversion_to(quotient.measure(), Role::Difference)?;
            let difference_restore = quotient
                .measure()
                .conversion_to(&coherent, Role::Difference)?;
            let origin = if points { origin.as_ref() } else { None };
            let target = coordinate_measure(quotient.measure(), origin)?;
            let role = if origin.is_some() {
                Role::Point
            } else {
                Role::Difference
            };
            maps.push(EntryNormalization {
                normalize: coherent.conversion_to(&target, role)?,
                restore: target.conversion_to(&coherent, role)?,
                quotient,
                difference_normalize,
                difference_restore,
            });
        }
    }
    Ok(maps)
}

/// Retain the correction broadcast to each RHS column; products await numerical kernels.
fn origin_corrections(
    domain: &CoordinateMap,
    codomain: &CoordinateMap,
    limits: ShapeLimits,
) -> Result<Vec<OriginCorrection>, ShapeError> {
    let mut corrections = Vec::new();
    for (row, origin) in codomain.origins.iter().enumerate() {
        if let Some(origin) = origin {
            corrections.push(OriginCorrection::RightOrigin {
                row,
                origin: origin.clone(),
            });
        }
        for (column, origin) in domain.origins.iter().enumerate() {
            if let Some(origin) = origin {
                corrections.push(OriginCorrection::OperatorOrigin {
                    row,
                    column,
                    origin: origin.clone(),
                });
            }
        }
    }
    limits.check_work(corrections.len())?;
    Ok(corrections)
}
