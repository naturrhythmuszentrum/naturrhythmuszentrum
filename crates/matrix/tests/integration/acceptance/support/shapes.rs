// SPDX-License-Identifier: AGPL-3.0-only
//! Checked physical and normalized coordinates, without a direct CAS dependency.

use super::ResultOf;
use nrz_matrix::{
    Axis, AxisEntry, BlockDraft, CheckedShape, EntryDraft, Partition, ShapeDraft, ShapeLimits,
    StructureScope,
};
use nrz_units::{
    CheckedDeclarations, Context, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry,
    Role,
};
use std::rc::Rc;

/// Freeze a descriptor owner before supplying any numerical coordinates.
#[expect(
    clippy::default_trait_access,
    reason = "matrix uses the units facade without naming its CAS implementation types"
)]
pub(crate) fn owner(registry: Registry) -> ResultOf<Rc<CheckedDeclarations>> {
    Ok(Rc::new(
        Context::new(registry, Default::default(), Default::default())?.freeze()?,
    ))
}

/// A coherent physical difference with a separately declared semantic kind.
pub(crate) fn quantity(name: &str, dimension: [i64; 7]) -> ResultOf<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(dimension)?)),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Coordinate identity follows construction order, never matching dimension strings.
pub(crate) fn axis(length: usize, quantity: &Descriptor) -> ResultOf<Axis> {
    let limits = ShapeLimits::default();
    let entries = (0..length)
        .map(|i| AxisEntry::new(format!("entry-{i}"), quantity.clone()))
        .collect::<Result<_, _>>()?;
    Ok(Axis::new(
        entries,
        Partition::whole(length, limits)?,
        limits,
    )?)
}

/// Check every original entry in a complete homogeneous rectangle.
pub(crate) fn dense(
    declarations: &Rc<CheckedDeclarations>,
    domain: &Axis,
    codomain: &Axis,
    quantity: &Descriptor,
) -> ResultOf<CheckedShape> {
    let rows = codomain.entries().len();
    let columns = domain.entries().len();
    Ok(ShapeDraft::new(
        domain.clone(),
        codomain.clone(),
        vec![BlockDraft::new(
            0..rows,
            0..columns,
            vec![EntryDraft::Declared(quantity.clone()); rows * columns],
        )],
        StructureScope::WholeFamily,
    )
    .check(Rc::clone(declarations), ShapeLimits::default())?)
}

/// Independent ratio coordinates for an explicitly normalized problem.
pub(crate) fn normalized(
    order: usize,
    columns: usize,
) -> ResultOf<(Rc<CheckedDeclarations>, [CheckedShape; 3])> {
    let declarations = owner(Registry::new(ExponentPolicy::Rational))?;
    let ratio = Descriptor::ratio();
    let variables = axis(order, &ratio)?;
    let equations = axis(order, &ratio)?;
    let loads = axis(columns, &ratio)?;
    let shapes = [
        dense(&declarations, &variables, &equations, &ratio)?,
        dense(&declarations, &loads, &equations, &ratio)?,
        dense(&declarations, &loads, &variables, &ratio)?,
    ];
    Ok((declarations, shapes))
}
