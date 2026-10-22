// SPDX-License-Identifier: AGPL-3.0-only
//! Shape fixtures use only the installed public bindings and matrix facades.

use nrz_bindings::{
    CheckedDeclarations, Context, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry,
    Role,
};
use nrz_matrix::{
    Axis, AxisEntry, BlockDraft, CheckedShape, EntryDraft, Partition, ShapeDraft, ShapeLimits,
    StructureScope,
};
use std::rc::Rc;

/// Fallible shape fixture result.
pub(crate) type ResultOf<T> = Result<T, Box<dyn std::error::Error>>;

/// Freeze a complete semantic registry without a direct CAS dependency.
#[expect(
    clippy::default_trait_access,
    reason = "the bindings context takes the CAS limits and environment, and nrz-matrix names no CAS type"
)]
pub(crate) fn owner(registry: Registry) -> ResultOf<Rc<CheckedDeclarations>> {
    Ok(Rc::new(
        Context::new(registry, Default::default(), Default::default())?.freeze()?,
    ))
}

/// Standard ratio-only declaration owner.
pub(crate) fn ratios() -> ResultOf<Rc<CheckedDeclarations>> {
    owner(Registry::new(ExponentPolicy::Rational))
}

/// Literal physical dimensions, independent of a derived quotient's expected result.
pub(crate) fn quantity(name: &str, dimension: [i64; 7]) -> ResultOf<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(dimension)?)),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Fresh ordered coordinates with one complete partition.
pub(crate) fn axis(names: &[&str], quantity: &Descriptor) -> ResultOf<Axis> {
    let entries = names
        .iter()
        .map(|name| AxisEntry::new(*name, quantity.clone()))
        .collect::<Result<_, _>>()?;
    Ok(Axis::new(
        entries,
        Partition::whole(names.len(), ShapeLimits::default())?,
        ShapeLimits::default(),
    )?)
}

/// Assemble a complete rectangle, with no blocks for empty extents.
pub(crate) fn dense(
    owner: &Rc<CheckedDeclarations>,
    domain: &Axis,
    codomain: &Axis,
    entries: Vec<EntryDraft>,
) -> ResultOf<CheckedShape> {
    let rows = codomain.entries().len();
    let columns = domain.entries().len();
    let blocks = if rows == 0 || columns == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(0..rows, 0..columns, entries)]
    };
    Ok(ShapeDraft::new(
        domain.clone(),
        codomain.clone(),
        blocks,
        StructureScope::WholeFamily,
    )
    .check(Rc::clone(owner), ShapeLimits::default())?)
}

/// A ratio-valued rectangle on the supplied coordinate identities.
pub(crate) fn ratio_shape(
    owner: &Rc<CheckedDeclarations>,
    domain: &Axis,
    codomain: &Axis,
) -> ResultOf<CheckedShape> {
    dense(
        owner,
        domain,
        codomain,
        vec![
            EntryDraft::Declared(Descriptor::ratio());
            domain.entries().len() * codomain.entries().len()
        ],
    )
}
