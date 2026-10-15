// SPDX-License-Identifier: AGPL-3.0-only
//! Heterogeneous normalization and logical structure stay independent of physical layout.

use crate::shape_testing::{ResultOf, axis, dense, owner, quantity, ratio_shape, ratios};
use nrz_matrix::{
    Axis, AxisEntry, CoordinateMap, EntryDraft, ExactCoefficient, ExpansionNorm, ExpansionRow,
    ExpansionTerm, Extent, Heap, HeapLimits, Layout, NormalizationSchema, Partition,
    ReferenceScale, ShapeError, ShapeLimits, StructureScope, StructuredExpansion,
};
use nrz_units::{BinaryOp, Descriptor, ExponentPolicy, Registry};

/// Build one exact source-free reference per ordered axis coordinate.
fn coordinates(axis: &Axis) -> ResultOf<CoordinateMap> {
    let references = axis
        .entries()
        .iter()
        .map(|entry| {
            Ok(ReferenceScale::new(
                entry.name(),
                entry.quantity().ok_or("scalar axis")?,
            )?)
        })
        .collect::<ResultOf<_>>()?;
    Ok(CoordinateMap::check(
        axis,
        references,
        vec![None; axis.entries().len()],
        ShapeLimits::default(),
    )?)
}

/// Mixed voltage/current schemas normalize entrywise and refuse a fictitious common physical norm.
/// ´test:integration:mixed-physical-normalization-has-entrywise-restoration´
#[test]
fn mixed_physical_normalization_has_entrywise_restoration() -> ResultOf<()> {
    let voltage = quantity("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let current = quantity("current", [0, 0, 0, 1, 0, 0, 0])?;
    let conductance = quantity("conductance", [-2, -1, 3, 2, 0, 0, 0])?;
    let resistance = quantity("resistance", [2, 1, -3, -2, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        conductance.kind().clone(),
        voltage.kind().clone(),
        current.kind().clone(),
    )?;
    registry.register_binary(
        BinaryOp::Multiply,
        resistance.kind().clone(),
        current.kind().clone(),
        voltage.kind().clone(),
    )?;
    let snapshot = owner(registry)?;
    let limits = ShapeLimits::default();
    let partition = Partition::new(2, vec![0, 1, 2], limits)?;
    let domain = Axis::new(
        vec![
            AxisEntry::new("v", voltage.clone())?,
            AxisEntry::new("i", current.clone())?,
        ],
        partition.clone(),
        limits,
    )?;
    let codomain = Axis::new(
        vec![
            AxisEntry::new("kcl", current.clone())?,
            AxisEntry::new("source", voltage.clone())?,
        ],
        partition,
        limits,
    )?;
    let operator = dense(
        &snapshot,
        &domain,
        &codomain,
        vec![
            EntryDraft::Declared(conductance),
            EntryDraft::Declared(Descriptor::ratio()),
            EntryDraft::Declared(Descriptor::ratio()),
            EntryDraft::Zero(resistance),
        ],
    )?;
    let column = axis(&["one"], &Descriptor::ratio())?;
    let rhs = dense(
        &snapshot,
        &column,
        &codomain,
        vec![EntryDraft::Declared(current), EntryDraft::Declared(voltage)],
    )?;
    let schema = NormalizationSchema::check(
        &operator,
        &rhs,
        coordinates(&domain)?,
        coordinates(&codomain)?,
        coordinates(&column)?,
        limits,
    )?;
    assert_ne!(
        schema
            .solution_maps()
            .first()
            .ok_or("voltage")?
            .quotient()
            .dimension(),
        schema
            .solution_maps()
            .get(1)
            .ok_or("current")?
            .quotient()
            .dimension()
    );
    let rows = [
        ExpansionRow::new(
            vec![ExpansionTerm::new(
                1,
                ExactCoefficient::unit(&Descriptor::ratio())?,
            )],
            None,
        ),
        ExpansionRow::new(
            vec![ExpansionTerm::new(
                0,
                ExactCoefficient::unit(&Descriptor::ratio())?,
            )],
            None,
        ),
    ];
    let expansion =
        StructuredExpansion::check(&rhs, domain, &rows, StructureScope::WholeFamily, limits)?;
    assert!(matches!(
        expansion.metric(ExpansionNorm::LogicalFrobenius, limits),
        Err(ShapeError::UnsupportedMetric)
    ));
    Ok(())
}

/// A logical transpose map agrees with the checked storage view without evaluating any normalization.
/// ´test:integration:shape-entry-map-agrees-with-strided-layout´
#[test]
fn shape_entry_map_agrees_with_strided_layout() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let shape = ratio_shape(&snapshot, &coordinates, &coordinates)?;
    let transposed = shape.dual_transpose(ShapeLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let value = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 3.0, 4.0],
    )?;
    let layout = Layout::strided(Extent::new(2, 2)?, 4, 0, 2, 1)?;
    let alias = heap.alias(value.value(), &layout)?;
    let view = heap.value(alias)?;
    let original = heap.value(value.value())?.region().export()?;
    let expected = transposed
        .entry_maps()
        .first()
        .ok_or("entry map")?
        .indices()
        .iter()
        .map(|index| original.get(*index).copied().ok_or("source index"))
        .collect::<Result<Vec<_>, _>>()?;
    let actual = view.region().export()?;
    assert_eq!(
        actual
            .iter()
            .map(|entry| entry.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|entry| entry.to_bits())
            .collect::<Vec<_>>()
    );
    Ok(())
}
