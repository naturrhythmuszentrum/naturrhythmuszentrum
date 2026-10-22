// SPDX-License-Identifier: AGPL-3.0-only
//! Physical block assemblies keep coordinate meaning through descriptor operations.

use crate::shape_testing::{ResultOf, axis, dense, owner, quantity, ratio_shape, ratios};
use nrz_bindings::{BinaryOp, Descriptor, Dimension, ExponentPolicy, Registry};
use nrz_matrix::{
    Axis, AxisAlignment, AxisEntry, BlockDraft, EntryDraft, Partition, ShapeDraft, ShapeLimits,
    ShapeRecipe, StructureScope,
};
use std::rc::Rc;

/// A truss stiffness transpose keeps stiffness dimensions; its inverse reverses them.
/// ´test:integration:truss-dual-and-solve-axis-contracts´
#[test]
fn truss_dual_and_solve_axis_contracts() -> ResultOf<()> {
    let displacement = quantity("displacement", [1, 0, 0, 0, 0, 0, 0])?;
    let force = quantity("force", [1, 1, -2, 0, 0, 0, 0])?;
    let stiffness = quantity("stiffness", [0, 1, -2, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        stiffness.kind().clone(),
        displacement.kind().clone(),
        force.kind().clone(),
    )?;
    let snapshot = owner(registry)?;
    let domain = axis(&["u0", "u1"], &displacement)?;
    let codomain = axis(&["f0", "f1"], &force)?;
    let operator = dense(
        &snapshot,
        &domain,
        &codomain,
        vec![EntryDraft::Declared(stiffness); 4],
    )?;
    let limits = ShapeLimits::default();
    let transposed = operator.dual_transpose(limits)?;
    assert_eq!(
        transposed.entry(1, 0)?.dimension(),
        &Dimension::integers([0, 1, -2, 0, 0, 0, 0])?
    );
    assert_eq!(
        transposed
            .domain()
            .entries()
            .first()
            .ok_or("dual")?
            .dimension(),
        &Dimension::integers([-1, -1, 2, 0, 0, 0, 0])?
    );
    assert!(
        transposed
            .domain()
            .entries()
            .first()
            .ok_or("dual")?
            .quantity()
            .is_none()
    );
    let inverse = operator.inverse(limits)?;
    assert_eq!(
        inverse.entry(0, 0)?.dimension(),
        &Dimension::integers([0, -1, 2, 0, 0, 0, 0])?
    );
    assert_eq!(inverse.domain(), &codomain);
    assert_eq!(inverse.codomain(), &domain);
    let loads = axis(&["load-a", "load-b", "load-c"], &Descriptor::ratio())?;
    let rhs = dense(
        &snapshot,
        &loads,
        &codomain,
        vec![EntryDraft::Declared(force); 6],
    )?;
    let solution = operator.solve(&rhs, limits)?;
    assert_eq!(solution.domain(), &loads);
    assert_eq!(solution.codomain(), &domain);
    assert_eq!(solution.entry(1, 2)?.dimension(), displacement.dimension());
    assert_eq!(solution.recipe(), ShapeRecipe::Solve);
    assert!(
        solution
            .entries()
            .iter()
            .all(|entry| entry.scalar_descriptor().is_none())
    );
    Ok(())
}

/// The mixed resistor table has conductance, ratio, ratio and typed resistance blocks.
/// ´test:integration:resistor-block-shape-assembly´
#[test]
fn resistor_block_shape_assembly() -> ResultOf<()> {
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
    let mut unknowns = axis(&["v0", "v1", "v2", "v3"], &voltage)?
        .entries()
        .to_vec();
    unknowns.push(AxisEntry::new("source-current", current.clone())?);
    let mut equations = axis(&["kcl0", "kcl1", "kcl2", "kcl3"], &current)?
        .entries()
        .to_vec();
    equations.push(AxisEntry::new("source-voltage", voltage)?);
    let partition = Partition::new(5, vec![0, 4, 5], limits)?;
    let domain = Axis::new(unknowns, partition.clone(), limits)?;
    let codomain = Axis::new(equations, partition, limits)?;
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
    let shape = ShapeDraft::new(domain, codomain, blocks, StructureScope::WholeFamily)
        .check(Rc::clone(&snapshot), limits)?;
    assert!(shape.entry(4, 4)?.is_zero());
    assert_eq!(
        shape.entry(0, 0)?.dimension(),
        &Dimension::integers([-2, -1, 3, 2, 0, 0, 0])?
    );
    assert_eq!(
        shape.entry(4, 4)?.dimension(),
        &Dimension::integers([2, 1, -3, -2, 0, 0, 0])?
    );
    assert_eq!(shape.entry(4, 0)?.dimension(), &Dimension::zero());
    Ok(())
}

/// Addition refines both original partitions in aligned coordinate order.
/// ´test:integration:aligned-sum-retains-partition-refinement´
#[test]
fn aligned_sum_retains_partition_refinement() -> ResultOf<()> {
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
    let sum = left.sum(&right, &alignment, &alignment, limits)?;
    assert_eq!(sum.domain().partition().boundaries(), [0, 1, 2, 4]);
    assert_eq!(sum.codomain().partition().boundaries(), [0, 1, 2, 4]);
    assert_eq!(sum.parents().len(), 2);
    assert_eq!(sum.alignments().len(), 2);
    Ok(())
}
