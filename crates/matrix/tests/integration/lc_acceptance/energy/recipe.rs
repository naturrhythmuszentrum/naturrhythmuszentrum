// SPDX-License-Identifier: AGPL-3.0-only
//! Native energy shapes retain the original state and element kinds on shared ordered axes.

use crate::reading_testing::{TestResult, ledger};
use crate::shape_testing::{axis, dense, owner, quantity};
use crate::spectral_testing::Fixture;
use nrz_bindings::{BinaryOp, Descriptor, ExponentPolicy, Registry};
use nrz_matrix::{
    CheckedOperation, Elementwise, EntryDraft, ExecutionLimits, FixedCoefficient, LinearMap,
    OperationDraft, ShapeLimits,
};

/// Square both state components, multiply original L/C, add, and apply the exact fixed half.
pub(super) fn operations(fixture: &Fixture) -> TestResult<Vec<CheckedOperation>> {
    let voltage = fixture.law.state().entries()[0].descriptor();
    let current = fixture.law.state().entries()[1].descriptor();
    let inductance = fixture.law.operator().frame().entries()[0].descriptor();
    let capacitance = fixture.law.operator().frame().entries()[1].descriptor();
    let squared = [
        quantity("squared-voltage", [4, 2, -6, -2, 0, 0, 0])?,
        quantity("squared-current", [0, 0, 0, 2, 0, 0, 0])?,
    ];
    let energy = quantity("energy", [2, 1, -2, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    let exponent = nrz_bindings::readings::algebra::Rational::from_ratio(2, 1, &mut ledger()?)?;
    for ((input, square), parameter) in [voltage, current]
        .into_iter()
        .zip(&squared)
        .zip([capacitance, inductance])
    {
        registry.register_power(
            input.kind().clone(),
            exponent.clone(),
            square.kind().clone(),
        )?;
        registry.register_binary(
            BinaryOp::Multiply,
            parameter.kind().clone(),
            square.kind().clone(),
            energy.kind().clone(),
        )?;
    }
    let declarations = owner(registry)?;
    let unit = axis(&["one"], &Descriptor::ratio())?;
    let output = axis(&["energy"], &energy)?;
    let mut operations = Vec::new();
    let mut products = Vec::new();
    for ((input, square), parameter) in [voltage, current]
        .into_iter()
        .zip(&squared)
        .zip([capacitance, inductance])
    {
        let [power, product] = term(&declarations, [&unit, &output], [input, square, parameter])?;
        operations.push(power);
        products.push(product);
    }
    let suffix = finish(&declarations, &output, &products)?;
    operations.extend(products);
    operations.extend(suffix);
    Ok(operations)
}

/// Physical squaring and element multiplication keep their common contraction axis.
fn term(
    declarations: &std::rc::Rc<nrz_bindings::CheckedDeclarations>,
    axes: [&nrz_matrix::Axis; 2],
    quantities: [&Descriptor; 3],
) -> TestResult<[CheckedOperation; 2]> {
    let [unit, output] = axes;
    let [input, square, parameter] = quantities;
    let physical = axis(&["state"], input)?;
    let powered = axis(&["squared-state"], square)?;
    let input = dense(
        declarations,
        unit,
        &physical,
        vec![EntryDraft::Declared(input.clone())],
    )?;
    let square = dense(
        declarations,
        unit,
        &powered,
        vec![EntryDraft::Declared(square.clone())],
    )?;
    let power = OperationDraft::map_elementwise(input, square.clone(), Elementwise::Square)?
        .check(ExecutionLimits::default())?;
    let coefficient = dense(
        declarations,
        &powered,
        output,
        vec![EntryDraft::Declared(parameter.clone())],
    )?;
    let product = OperationDraft::matmul(coefficient, square, ShapeLimits::default())?
        .check(ExecutionLimits::default())?;
    Ok([power, product])
}

/// Both contributions meet on one energy axis before the fixed rational half.
fn finish(
    declarations: &std::rc::Rc<nrz_bindings::CheckedDeclarations>,
    output: &nrz_matrix::Axis,
    products: &[CheckedOperation],
) -> TestResult<[CheckedOperation; 2]> {
    let sum = OperationDraft::add(
        products[0].outputs()[0].shape().clone(),
        products[1].outputs()[0].shape().clone(),
        ShapeLimits::default(),
    )?
    .check(ExecutionLimits::default())?;
    let half = dense(
        declarations,
        output,
        output,
        vec![EntryDraft::Declared(Descriptor::ratio())],
    )?;
    let half = LinearMap::new(
        half,
        sum.outputs()[0].shape().clone(),
        vec![FixedCoefficient::represented(Descriptor::ratio(), 0.5)?],
        Vec::new(),
        nrz_scalar::LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    Ok([
        sum,
        OperationDraft::linear_map(half).check(ExecutionLimits::default())?,
    ])
}
