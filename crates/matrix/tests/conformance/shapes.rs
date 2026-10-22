// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered axes and physical quotient refusal twins at the public boundary.

use crate::shape_testing::{ResultOf, axis, dense, owner, quantity, ratio_shape, ratios};
use nrz_bindings::{
    BinaryOp, Context, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry, Role,
};
use nrz_matrix::{
    Axis, AxisAlignment, AxisEntry, BlockDraft, EntryDraft, EntryMap, Extent, Partition,
    ShapeDraft, ShapeError, ShapeLimits, ShapeRecipe, StructureScope,
};
use std::rc::Rc;

/// Equal dimensions and equal display labels cannot manufacture coordinate identity.
/// ´test:integration:ordered-axis-identity-refusals´
#[test]
fn ordered_axis_identity_refusals() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let alien = axis(&["a", "b"], &Descriptor::ratio())?;
    let operator = ratio_shape(&snapshot, &coordinates, &coordinates)?;
    let wrong = ratio_shape(&snapshot, &alien, &alien)?;
    assert!(matches!(
        operator.product(&wrong, ShapeLimits::default()),
        Err(ShapeError::AxisMismatch)
    ));
    let reversed = Axis::new(
        coordinates.entries().iter().rev().cloned().collect(),
        coordinates.partition().clone(),
        ShapeLimits::default(),
    )?;
    let reordered = ratio_shape(&snapshot, &reversed, &reversed)?;
    assert!(matches!(
        operator.product(&reordered, ShapeLimits::default()),
        Err(ShapeError::AxisMismatch)
    ));
    assert!(operator.product(&operator, ShapeLimits::default()).is_ok());
    let alignment =
        AxisAlignment::check(&coordinates, &reversed, vec![1, 0], ShapeLimits::default())?;
    let sum = operator.sum(&reordered, &alignment, &alignment, ShapeLimits::default())?;
    assert_eq!(
        sum.entry_maps().first().ok_or("sum map")?.indices(),
        [3, 2, 1, 0]
    );
    assert!(
        AxisAlignment::check(&coordinates, &reversed, vec![0, 0], ShapeLimits::default()).is_err()
    );
    Ok(())
}

/// Typed zeros obey quotient dimensions and named scalar multiplication rules.
/// ´test:integration:typed-zero-and-kind-refusals´
#[test]
fn typed_zero_and_kind_refusals() -> ResultOf<()> {
    let displacement = quantity("displacement", [1, 0, 0, 0, 0, 0, 0])?;
    let force = quantity("force", [1, 1, -2, 0, 0, 0, 0])?;
    let stiffness = quantity("stiffness", [0, 1, -2, 0, 0, 0, 0])?;
    let impostor = quantity("impostor", [0, 1, -2, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        stiffness.kind().clone(),
        displacement.kind().clone(),
        force.kind().clone(),
    )?;
    let snapshot = owner(registry)?;
    let domain = axis(&["u"], &displacement)?;
    let codomain = axis(&["f"], &force)?;
    let valid = dense(
        &snapshot,
        &domain,
        &codomain,
        vec![EntryDraft::Zero(stiffness)],
    )?;
    assert!(valid.entry(0, 0)?.is_zero());
    assert_eq!(
        valid.entry(0, 0)?.dimension(),
        &Dimension::integers([0, 1, -2, 0, 0, 0, 0])?
    );
    assert!(
        dense(
            &snapshot,
            &domain,
            &codomain,
            vec![EntryDraft::Zero(Descriptor::ratio())]
        )
        .is_err()
    );
    assert!(
        dense(
            &snapshot,
            &domain,
            &codomain,
            vec![EntryDraft::Zero(impostor)]
        )
        .is_err()
    );
    let point = Descriptor::new(force.kind().clone(), Measure::coherent(), Role::Point)?;
    assert!(
        dense(
            &snapshot,
            &domain,
            &axis(&["point"], &point)?,
            vec![EntryDraft::Declared(point)]
        )
        .is_err()
    );
    Ok(())
}

/// Coverage is complete and nonoverlapping, with boundaries respected by every block.
/// ´test:integration:shape-partition-and-coverage-refusals´
#[test]
fn shape_partition_and_coverage_refusals() -> ResultOf<()> {
    let snapshot = ratios()?;
    let limits = ShapeLimits::default();
    let entries = axis(&["a", "b"], &Descriptor::ratio())?;
    let split = Axis::new(
        entries.entries().to_vec(),
        Partition::new(2, vec![0, 1, 2], limits)?,
        limits,
    )?;
    let block = || {
        BlockDraft::new(
            0..1,
            0..2,
            vec![EntryDraft::Declared(Descriptor::ratio()); 2],
        )
    };
    let draft = |blocks| {
        ShapeDraft::new(
            split.clone(),
            split.clone(),
            blocks,
            StructureScope::CenterOnly,
        )
    };
    assert!(matches!(
        draft(vec![block()]).check(Rc::clone(&snapshot), limits),
        Err(ShapeError::Partition)
    ));
    assert!(matches!(
        draft(vec![block(), block()]).check(Rc::clone(&snapshot), limits),
        Err(ShapeError::Partition)
    ));
    let bottom = BlockDraft::new(1..2, 0..2, vec![EntryDraft::Zero(Descriptor::ratio()); 2]);
    assert!(
        draft(vec![block(), bottom])
            .check(Rc::clone(&snapshot), limits)
            .is_ok()
    );
    assert!(Partition::new(2, vec![0, 1, 1, 2], limits).is_err());
    assert!(Partition::new(2, vec![0, 1], limits).is_err());
    let first = Partition::new(4, vec![0, 1, 4], limits)?;
    let second = Partition::new(4, vec![0, 2, 4], limits)?;
    assert_eq!(first.refine(&second, limits)?.boundaries(), [0, 1, 2, 4]);
    assert!(
        first
            .refine(&second, ShapeLimits::new(4, 16, 2, 16))
            .is_err()
    );
    Ok(())
}

/// Empty contraction yields typed zero output; empty solve and rectangular inverse refuse.
/// ´test:integration:empty-shape-operation-contracts´
#[test]
fn empty_shape_operation_contracts() -> ResultOf<()> {
    let snapshot = ratios()?;
    let limits = ShapeLimits::default();
    let empty = axis(&[], &Descriptor::ratio())?;
    let row = axis(&["r"], &Descriptor::ratio())?;
    let columns = axis(&["c", "d"], &Descriptor::ratio())?;
    let left = ratio_shape(&snapshot, &empty, &row)?;
    let right = ratio_shape(&snapshot, &columns, &empty)?;
    let product = left.product(&right, limits)?;
    assert_eq!(product.extent(), Extent::new(1, 2)?);
    assert!(
        product
            .entries()
            .iter()
            .all(nrz_matrix::EntryContract::is_zero)
    );
    assert!(matches!(left.inverse(limits), Err(ShapeError::SolveExtent)));
    let vacant = ratio_shape(&snapshot, &empty, &empty)?;
    assert!(matches!(
        vacant.solve(&right, limits),
        Err(ShapeError::SolveExtent)
    ));
    let square = ratio_shape(&snapshot, &row, &row)?;
    assert!(matches!(
        square.solve(&left, limits),
        Err(ShapeError::SolveExtent)
    ));
    assert!(matches!(
        square.product(&square, ShapeLimits::new(1, 1, 1, 0)),
        Err(ShapeError::Limit)
    ));
    Ok(())
}

/// Original evidence is immutable, owned, and cannot cross a declaration snapshot.
/// ´test:integration:original-shape-evidence-retains-owner´
#[expect(
    clippy::default_trait_access,
    reason = "the bindings context takes the CAS limits and environment, and nrz-matrix names no CAS type"
)]
#[test]
fn original_shape_evidence_retains_owner() -> ResultOf<()> {
    let context = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Default::default(),
        Default::default(),
    )?;
    let original = context.parse("coefficient", "0 * (1 + 1)")?;
    let checked = context.check(&original)?;
    assert!(
        context
            .check(&context.parse("invalid", "0 * sin(1)")?)
            .is_err()
    );
    let snapshot = Rc::new(context.freeze()?);
    let coordinate = axis(&["x"], &Descriptor::ratio())?;
    let shape = dense(
        &snapshot,
        &coordinate,
        &coordinate,
        vec![EntryDraft::Checked(Box::new(checked.clone()))],
    )?;
    drop(snapshot);
    let evidence = shape
        .entry(0, 0)?
        .checked_expression()
        .ok_or("original evidence")?;
    shape.declarations().validate_checked(evidence)?;
    assert!(evidence.nodes().len() > 1);
    assert!(
        dense(
            &ratios()?,
            &coordinate,
            &coordinate,
            vec![EntryDraft::Checked(Box::new(checked))]
        )
        .is_err()
    );
    Ok(())
}

/// Rectangular logical selection records column vectorization and repeated identities.
/// ´test:integration:logical-shape-maps-preserve-order´
#[test]
fn logical_shape_maps_preserve_order() -> ResultOf<()> {
    let limits = ShapeLimits::default();
    let snapshot = ratios()?;
    let domain = axis(&["a", "b", "c"], &Descriptor::ratio())?;
    let codomain = axis(&["r", "s"], &Descriptor::ratio())?;
    let shape = ratio_shape(&snapshot, &domain, &codomain)?;
    let transposed = shape.dual_transpose(limits)?;
    assert_eq!(
        transposed
            .entry_maps()
            .first()
            .ok_or("transpose map")?
            .indices(),
        [0, 2, 4, 1, 3, 5]
    );
    let restored = transposed.dual_transpose(limits)?;
    assert_eq!(restored.domain(), &domain);
    assert_eq!(restored.codomain(), &codomain);
    let column = axis(&["one"], &Descriptor::ratio())?;
    let map = EntryMap::new(shape.extent(), Extent::new(2, 1)?, vec![4, 4], limits)?;
    let selected = shape.select(column, codomain, map, limits)?;
    assert_eq!(selected.recipe(), ShapeRecipe::Selection);
    assert_eq!(
        selected.entry_maps().first().ok_or("selection")?.indices(),
        [4, 4]
    );
    assert!(EntryMap::new(shape.extent(), Extent::new(1, 1)?, vec![6], limits).is_err());
    assert!(EntryMap::reshape(shape.extent(), Extent::new(3, 2)?, limits).is_ok());
    Ok(())
}

/// A role-correct axis cannot accept a point coefficient under multiplication.
/// ´test:integration:point-coefficient-is-not-linear´
#[test]
fn point_coefficient_is_not_linear() -> ResultOf<()> {
    let value = quantity("temperature", [0, 0, 0, 0, 1, 0, 0])?;
    let point = Descriptor::new(value.kind().clone(), Measure::coherent(), Role::Point)?;
    let snapshot = ratios()?;
    let input = axis(&["ratio"], &Descriptor::ratio())?;
    let output = axis(&["temperature"], &value)?;
    assert!(
        dense(
            &snapshot,
            &input,
            &output,
            vec![EntryDraft::Declared(point)]
        )
        .is_err()
    );
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let alien = AxisEntry::new("temperature", angle)?;
    assert_ne!(output.entries().first(), Some(&alien));
    Ok(())
}

/// Heterogeneous identities retain typed off-diagonal zeros without inventing scalar kinds.
/// ´test:integration:heterogeneous-identity-has-typed-quotient-zeros´
#[test]
fn heterogeneous_identity_has_typed_quotient_zeros() -> ResultOf<()> {
    let length = quantity("length", [1, 0, 0, 0, 0, 0, 0])?;
    let time = quantity("time", [0, 0, 1, 0, 0, 0, 0])?;
    let limits = ShapeLimits::default();
    let coordinates = Axis::new(
        vec![AxisEntry::new("x", length)?, AxisEntry::new("t", time)?],
        Partition::whole(2, limits)?,
        limits,
    )?;
    let identity = nrz_matrix::CheckedShape::identity(&coordinates, ratios()?, limits)?;
    assert!(!identity.entry(0, 0)?.is_zero());
    assert!(identity.entry(0, 1)?.is_zero());
    assert_eq!(
        identity.entry(0, 1)?.dimension(),
        &Dimension::integers([1, 0, -1, 0, 0, 0, 0])?
    );
    assert!(identity.entry(0, 1)?.scalar_descriptor().is_none());
    Ok(())
}

/// A sum involving a formal result keeps a formal result and limits the combined refinement.
/// ´test:integration:formal-sums-and-refinement-capacity´
#[test]
fn formal_sums_and_refinement_capacity() -> ResultOf<()> {
    let snapshot = ratios()?;
    let limits = ShapeLimits::default();
    let coordinates = axis(&["a", "b", "c", "d"], &Descriptor::ratio())?;
    let first = Axis::new(
        coordinates.entries().to_vec(),
        Partition::new(4, vec![0, 1, 4], limits)?,
        limits,
    )?;
    let second = Axis::new(
        coordinates.entries().to_vec(),
        Partition::new(4, vec![0, 2, 4], limits)?,
        limits,
    )?;
    let left = ratio_shape(&snapshot, &first, &first)?;
    let right = ratio_shape(&snapshot, &second, &second)?;
    let alignment = AxisAlignment::identity(&first, &second, limits)?;
    assert!(matches!(
        left.sum(
            &right,
            &alignment,
            &alignment,
            ShapeLimits::new(4, 16, 5, 16)
        ),
        Err(ShapeError::Limit)
    ));
    let inverse = left.inverse(limits)?;
    let aligned = AxisAlignment::identity(&first, &first, limits)?;
    let sum = left.sum(&inverse, &aligned, &aligned, limits)?;
    assert!(
        sum.entries()
            .iter()
            .all(|entry| entry.scalar_descriptor().is_none())
    );
    Ok(())
}
