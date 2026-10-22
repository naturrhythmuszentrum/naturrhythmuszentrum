// SPDX-License-Identifier: AGPL-3.0-only
//! Physical restoration uses separate homogeneous voltage and current regions.

use super::{Case, ResultOf, shapes};
use nrz_bindings::{CheckedDeclarations, Descriptor};
use nrz_matrix::{
    CheckedShape, FixedCoefficient, Handle, Heap, LinearMap, OperationDraft, ShapeLimits, Value,
};
use nrz_scalar::LiteralLimits;
use std::rc::Rc;

/// Restore each admitted physical block with exact unit coefficients and retained source lineage.
pub(crate) fn blocks(
    heap: &mut Heap,
    contract: (&Rc<CheckedDeclarations>, &CheckedShape),
    input: Handle<Value, f64>,
    gain: u8,
    result: &mut Case,
) -> ResultOf<[Handle<Value, f64>; 2]> {
    let voltage = shapes::quantity("voltage", [2, 1, -3, -1, 0, 0, 0])?;
    let current = shapes::quantity("current", [0, 0, 0, 1, 0, 0, 0])?;
    Ok([
        block(
            heap,
            contract,
            input,
            &voltage,
            &[0, 1, 2, 3],
            result,
            &format!("voltage-gain-{gain}"),
        )?,
        block(
            heap,
            contract,
            input,
            &current,
            &[4],
            result,
            &format!("current-gain-{gain}"),
        )?,
    ])
}

/// Every zero and unit coefficient has the physical output-over-ratio descriptor.
fn block(
    heap: &mut Heap,
    contract: (&Rc<CheckedDeclarations>, &CheckedShape),
    input: Handle<Value, f64>,
    quantity: &Descriptor,
    selection: &[usize],
    result: &mut Case,
    name: &str,
) -> ResultOf<Handle<Value, f64>> {
    let (declarations, shape) = contract;
    let axis = shapes::axis(selection.len(), quantity)?;
    let operator = shapes::dense(declarations, shape.codomain(), &axis, quantity)?;
    let mut coefficients = vec![
        FixedCoefficient::represented(quantity.clone(), 0.0)?;
        selection.len() * shape.extent().rows()
    ];
    for (row, &column) in selection.iter().enumerate() {
        coefficients[row + selection.len() * column] =
            FixedCoefficient::represented(quantity.clone(), 1.0)?;
    }
    let map = LinearMap::new(
        operator,
        shape.clone(),
        coefficients,
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    super::resistor::evaluate(
        heap,
        OperationDraft::linear_map(map),
        vec![input],
        result,
        name,
    )
}
