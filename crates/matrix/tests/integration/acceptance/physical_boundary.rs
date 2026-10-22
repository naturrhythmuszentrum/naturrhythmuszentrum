// SPDX-License-Identifier: AGPL-3.0-only
//! The heterogeneous shape contract is retained even where direct numerical storage refuses.

use crate::shape_testing::{ResultOf, axis, dense, owner, quantity};
use nrz_bindings::{BinaryOp, Descriptor, ExponentPolicy, Registry};
use nrz_matrix::{
    Axis, AxisEntry, BlockDraft, CheckedShape, CoordinateMap, EntryDraft, NormalizationSchema,
    OperationDraft, OperationError, Partition, ReferenceScale, ShapeDraft, ShapeLimits,
    StructureScope,
};
use std::rc::Rc;

/// The full ladder's physical blocks check; the current homogeneous operation boundary is explicit.
/// ´test:integration:integrated-mixed-resistor-retains-checked-normalization-boundary´
#[test]
fn integrated_mixed_resistor_retains_checked_normalization_boundary() -> ResultOf<()> {
    let [operator, rhs, solution] = shapes()?;
    let limits = ShapeLimits::default();
    let schema = NormalizationSchema::check(
        &operator,
        &rhs,
        coordinates(operator.domain())?,
        coordinates(operator.codomain())?,
        coordinates(rhs.domain())?,
        limits,
    )?;
    assert_eq!(schema.solution_maps().len(), 5);
    for (entry, map) in schema.solution_maps().iter().enumerate() {
        assert_eq!(
            map.quotient().dimension(),
            solution.entry(entry, 0)?.dimension()
        );
    }
    assert!(operator.entry(4, 4)?.is_zero());
    assert!(matches!(
        OperationDraft::solve(operator, rhs, solution, limits),
        Err(OperationError::UnsupportedShape)
    ));
    Ok(())
}

/// One exact source-free SI reference per physical coordinate.
fn coordinates(axis: &Axis) -> ResultOf<CoordinateMap> {
    let references = axis
        .entries()
        .iter()
        .map(|entry| {
            Ok(ReferenceScale::new(
                entry.name(),
                entry.quantity().ok_or("physical coordinate")?,
            )?)
        })
        .collect::<ResultOf<Vec<_>>>()?;
    Ok(CoordinateMap::check(
        axis,
        references,
        vec![None; axis.entries().len()],
        ShapeLimits::default(),
    )?)
}

/// Ordered voltage/current axes keep quotient dimensions and the typed resistance zero.
fn shapes() -> ResultOf<[CheckedShape; 3]> {
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
    let declarations = owner(registry)?;
    let domain = mixed_axis(&voltage, &current)?;
    let codomain = mixed_axis(&current, &voltage)?;
    let blocks = vec![
        BlockDraft::new(0..4, 0..4, vec![EntryDraft::Declared(conductance); 16]),
        BlockDraft::new(
            0..4,
            4..5,
            vec![EntryDraft::Declared(Descriptor::ratio()); 4],
        ),
        BlockDraft::new(
            4..5,
            0..4,
            vec![EntryDraft::Declared(Descriptor::ratio()); 4],
        ),
        BlockDraft::new(4..5, 4..5, vec![EntryDraft::Zero(resistance)]),
    ];
    let operator = ShapeDraft::new(
        domain.clone(),
        codomain.clone(),
        blocks,
        StructureScope::WholeFamily,
    )
    .check(Rc::clone(&declarations), ShapeLimits::default())?;
    let load = axis(&["source"], &Descriptor::ratio())?;
    let mut inputs = vec![EntryDraft::Declared(current.clone()); 4];
    inputs.push(EntryDraft::Declared(voltage.clone()));
    let mut outputs = vec![EntryDraft::Declared(voltage); 4];
    outputs.push(EntryDraft::Declared(current));
    Ok([
        operator,
        dense(&declarations, &load, &codomain, inputs)?,
        dense(&declarations, &load, &domain, outputs)?,
    ])
}

/// Four like coordinates followed by the source's unlike fifth coordinate.
fn mixed_axis(first: &Descriptor, last: &Descriptor) -> ResultOf<Axis> {
    let mut entries = axis(&["0", "1", "2", "3"], first)?.entries().to_vec();
    entries.push(AxisEntry::new("4", last.clone())?);
    Ok(Axis::new(
        entries,
        Partition::new(5, vec![0, 4, 5], ShapeLimits::default())?,
        ShapeLimits::default(),
    )?)
}
