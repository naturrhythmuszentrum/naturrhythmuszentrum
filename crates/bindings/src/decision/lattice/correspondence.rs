// SPDX-License-Identifier: AGPL-3.0-only
//! Original target, offset and basis entries restore through the exact physical frame.

use super::{DecisionError, LatticeDecisionSpec, preparation::Preparations};
use crate::{DecisionCoordinate, Role, decision::storage, readings::EntryMeaning};
use nrz_cas::{
    Cas, Handle,
    lattice::LatticeConvention,
    limits::Budget,
    source::{SyntaxNode, SyntaxRole},
    syntax::{AtomicSort, Extent, Field, Operator, Sort},
};

/// Validate every original row, including zero products and exact affine point origins.
pub(super) fn check(
    cas: &Cas,
    value: &LatticeDecisionSpec,
    budget: &mut Budget,
) -> Result<Preparations, DecisionError> {
    let mut prepared = Preparations::default();
    let basis = value.basis.question();
    cas.validate_lattice_basis(&value.index.question().basis, &value.basis, budget)?;
    if value.frame.context() != &basis.context {
        return Err(DecisionError::LatticeMeaning);
    }
    match (value.convention, value.metric) {
        (LatticeConvention::Floor, None) | (LatticeConvention::Nearest, Some(_)) => {}
        _ => return Err(DecisionError::LatticeMetric),
    }
    let dimension = value.basis.dimension();
    let header = cas.context_header(&basis.context)?;
    let targets = entries(cas, &value.target, &header, [dimension, 1], budget)?;
    let offsets = entries(cas, &basis.offset, &header, [dimension, 1], budget)?;
    let coefficients = entries(cas, &basis.basis, &header, [dimension, dimension], budget)?;
    let width = value.axes.len();
    for (row, (axis, meaning)) in value.axes.iter().zip(value.frame.entries()).enumerate() {
        coordinate(
            cas,
            &targets[row],
            &axis.target,
            meaning,
            false,
            &mut prepared,
            budget,
        )?;
        coordinate(
            cas,
            &offsets[row],
            &axis.offset,
            meaning,
            false,
            &mut prepared,
            budget,
        )?;
        for (column, physical) in axis.basis.iter().enumerate() {
            coordinate(
                cas,
                &coefficients[row * width + column],
                physical,
                meaning,
                true,
                &mut prepared,
                budget,
            )?;
        }
    }
    Ok(prepared)
}

/// Typed syntax exposes original operands without importing CAS-private matrix machinery.
fn entries(
    cas: &Cas,
    node: &Handle,
    header: &Handle,
    shape: [u32; 2],
    budget: &mut Budget,
) -> Result<Vec<Handle>, DecisionError> {
    let syntax = cas.describe_syntax(node, budget)?;
    let SyntaxNode::Expression(expression) = syntax.node else {
        return Err(DecisionError::LatticeShape);
    };
    let [rows, columns] = shape;
    let expected = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    });
    let count = usize::try_from(u64::from(rows) * u64::from(columns))
        .map_err(|_| DecisionError::Capacity)?;
    if syntax.header != *header
        || !syntax.scope.is_empty()
        || syntax.binding.is_some()
        || expression.operator != (Operator::Matrix { rows, columns })
        || expression.sort != expected
        || syntax.dependencies.len() != count
    {
        return Err(DecisionError::LatticeShape);
    }
    let bytes = u64::try_from(count)
        .map_err(|_| DecisionError::Capacity)?
        .saturating_mul(64);
    storage::retained(bytes, budget)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| DecisionError::Capacity)?;
    for (position, dependency) in syntax.dependencies.into_iter().enumerate() {
        budget.work(1, storage::AT)?;
        if dependency.role
            != SyntaxRole::Operand(u32::try_from(position).map_err(|_| DecisionError::Capacity)?)
        {
            return Err(DecisionError::LatticeShape);
        }
        result.push(dependency.target);
    }
    Ok(result)
}

/// Compare coherent physical values after applying the retained exact reference and origin.
fn coordinate(
    cas: &Cas,
    node: &Handle,
    physical: &DecisionCoordinate,
    meaning: &EntryMeaning,
    difference: bool,
    prepared: &mut Preparations,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    let descriptor = physical.checked()?.descriptor();
    descriptor.compatible_kind(meaning.descriptor())?;
    let role = if difference {
        Role::Difference
    } else {
        meaning.descriptor().role()
    };
    if descriptor.role() != role {
        return Err(DecisionError::LatticeMeaning);
    }
    let restored = prepared.restore(cas, node, meaning, difference, budget)?;
    budget.work(
        storage::constant_bytes(&restored).max(storage::constant_bytes(physical.exact())),
        storage::AT,
    )?;
    if restored.as_ref() != physical.exact() {
        return Err(DecisionError::LatticeMeaning);
    }
    Ok(())
}
