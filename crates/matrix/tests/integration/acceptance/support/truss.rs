// SPDX-License-Identifier: AGPL-3.0-only
//! Three-member static truss: physical stiffness, load, displacement and member-force regions.

use super::{Case, ResultOf, capture, model, shapes};
use nrz_accuracy::Budget;
use nrz_matrix::{
    Binding, CheckedShape, ExecutionLimits, Extent, FixedCoefficient, Heap, HeapLimits, LinearMap,
    OperationDraft, PublishedBundle, ShapeLimits, SolveBudget, SolvePolicy, SolveRecordKeys,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral};
use nrz_units::{BinaryOp, Descriptor, ExponentPolicy, Registry};
use std::time::Instant;

/// Execute the committed truss problem, with a separately declared finite-moment load scenario.
pub(crate) fn truss() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("truss");
    let [operator_shape, load_shape, solution_shape] = contracts()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let [operator, rhs] = inputs(&mut heap, &operator_shape, &load_shape)?;
    result.inputs(&heap, &[operator, rhs])?;
    let length = solution_shape
        .entry(0, 0)?
        .scalar_descriptor()
        .ok_or("length")?;
    let policy = policy(length)?;
    let draft = OperationDraft::solve(
        operator_shape.clone(),
        load_shape,
        solution_shape,
        ShapeLimits::default(),
    )?
    .with_solve_policy(policy)?;
    result
        .timings
        .push(("input-and-shape", started.elapsed().as_secs_f64()));
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let statistical_work = bound.statistical_work();
    let phase = Instant::now();
    let factor = bound.factor()?;
    result.factors += 1;
    result
        .timings
        .push(("factor-and-inverse-witness", phase.elapsed().as_secs_f64()));
    let phase = Instant::now();
    let candidate = bound.evaluate_factored(&bound.normalized_inputs()?, &factor)?;
    let outcome = bound.publish(candidate)?;
    result.timings.push((
        "solve-certificate-moments-publication",
        phase.elapsed().as_secs_f64(),
    ));
    result.samples.push(capture::sample(
        &heap,
        &outcome,
        "displacement",
        statistical_work,
    )?);
    member_forces(&mut heap, &outcome, operator_shape, &mut result)?;
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}

/// Import exact stiffness entries and independently decoded numerical load text.
fn inputs(
    heap: &mut Heap,
    operator_shape: &CheckedShape,
    load_shape: &CheckedShape,
) -> ResultOf<[PublishedBundle<f64>; 2]> {
    let stiffness = operator_shape
        .entry(0, 0)?
        .scalar_descriptor()
        .ok_or("stiffness")?;
    let force = load_shape.entry(0, 0)?.scalar_descriptor().ok_or("force")?;
    let operator = heap.import_declared_bounds(
        Extent::new(2, 2)?,
        stiffness,
        &[1000.5, 0.5, 0.5, 1.5],
        &[0.0; 4],
    )?;
    let literal = NumericLiteral::new("1/1000", LiteralLimits::default())?
        .decode::<f64>(&ArithmeticContext::new()?)?;
    let rhs = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        force,
        &[1.0, literal.center().get()],
        &[0.0, literal.displacement().get()],
    )?;
    model::attach(
        heap,
        [operator.value(), rhs.value()],
        &model::Law::Factor(&[0.0, 0.0, 0.0, 0.0, 1.0, 1.0], 0.0625),
    )?;
    Ok([operator, rhs])
}

/// A positive characteristic displacement supplies the physical relative budget scale.
fn policy(length: &Descriptor) -> ResultOf<SolvePolicy> {
    let budget = SolveBudget::frobenius(length.clone(), Budget::new(0.0, 0.000_001, 0.001)?)?;
    let keys = SolveRecordKeys::new(
        Some("truss-static".into()),
        Some("base".into()),
        Some("load-1".into()),
        Some("displacement".into()),
    )?;
    Ok(SolvePolicy::with_budget(budget).with_record_keys(keys))
}

/// Hooke's law maps the two orthogonal member extensions into physical forces.
fn member_forces(
    heap: &mut Heap,
    displacement: &nrz_matrix::Outcome<Vec<nrz_matrix::PublishedBundle<f64>>>,
    operator: CheckedShape,
    result: &mut Case,
) -> ResultOf {
    let started = Instant::now();
    let shape = heap
        .operation_record(displacement.record())?
        .operation()
        .outputs()
        .first()
        .ok_or("displacement shape")?
        .shape()
        .clone();
    let quantity = operator
        .entry(0, 0)?
        .scalar_descriptor()
        .ok_or("stiffness")?;
    let coefficients = [1000.0, 0.0, 0.0, 1.0]
        .into_iter()
        .map(|value| FixedCoefficient::represented(quantity.clone(), value))
        .collect::<Result<_, _>>()?;
    let map = LinearMap::new(
        operator,
        shape,
        coefficients,
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    let limits = ExecutionLimits::default();
    let input = displacement.output().first().ok_or("displacement")?.value();
    let bound = Binding::new(
        OperationDraft::linear_map(map).check(limits)?,
        vec![input],
        limits,
    )
    .validate(heap)?;
    let work = bound.statistical_work();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let forces = bound.publish(candidate)?;
    result.samples.push(capture::sample(
        heap,
        &forces,
        "orthogonal-member-forces",
        work,
    )?);
    result
        .timings
        .push(("member-force-map", started.elapsed().as_secs_f64()));
    Ok(())
}

/// The reduced truss stiffness remains a physical N/m operator throughout the lifecycle.
fn contracts() -> ResultOf<[CheckedShape; 3]> {
    let length = shapes::quantity("length", [1, 0, 0, 0, 0, 0, 0])?;
    let force = shapes::quantity("force", [1, 1, -2, 0, 0, 0, 0])?;
    let stiffness = shapes::quantity("stiffness", [0, 1, -2, 0, 0, 0, 0])?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        stiffness.kind().clone(),
        length.kind().clone(),
        force.kind().clone(),
    )?;
    let declarations = shapes::owner(registry)?;
    let variables = shapes::axis(2, &length)?;
    let equations = shapes::axis(2, &force)?;
    let load = shapes::axis(1, &Descriptor::ratio())?;
    Ok([
        shapes::dense(&declarations, &variables, &equations, &stiffness)?,
        shapes::dense(&declarations, &load, &equations, &force)?,
        shapes::dense(&declarations, &load, &variables, &length)?,
    ])
}
