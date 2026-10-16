// SPDX-License-Identifier: AGPL-3.0-only
//! Three common-source ladder solves in the explicitly normalized coordinates of the design.

use super::{Case, ResultOf, capture, model, shapes};
use nrz_accuracy::Budget;
use nrz_matrix::{
    Binding, CheckedShape, ExecutionLimits, Extent, FactorRef, FixedCoefficient, Handle, Heap,
    HeapLimits, JointLimits, LinearMap, OperationDraft, ShapeLimits, SolveBudget, SolvePolicy,
    SolveRecordKeys, Value,
};
use nrz_scalar::LiteralLimits;
use nrz_units::CheckedDeclarations;
use nrz_units::{Conversion, Descriptor, Measure, Role, Scale};
use std::{rc::Rc, time::Instant};

/// Exact nodal elimination of V1=E and I=V2-E, ordered (V4,V3,V2).
const OPERATOR: [f64; 9] = [2.0, -1.0, 0.0, -1.0, 3.0, -1.0, 0.0, -1.0, 3.0];

/// Retain three independent result records, every cross block, and the dependent mean.
pub(crate) fn resistor() -> ResultOf<Case> {
    let started = Instant::now();
    let mut result = Case::new("resistor");
    let mut heap = Heap::new(HeapLimits::default())?;
    let (declarations, [operator_shape, rhs_shape, solution_shape]) = shapes::normalized(3, 1)?;
    let operator = heap.import_represented(Extent::new(3, 3)?, &Descriptor::ratio(), &OPERATOR)?;
    let source =
        heap.import_represented(Extent::new(3, 1)?, &Descriptor::ratio(), &[0.0, 0.0, 1.0])?;
    model::attach(
        &mut heap,
        [operator.value(), source.value()],
        &model::Law::Resistor,
    )?;
    result.inputs(&heap, &[operator, source])?;
    let whole = shapes::dense(
        &declarations,
        solution_shape.domain(),
        &shapes::axis(5, &Descriptor::ratio())?,
        &Descriptor::ratio(),
    )?;
    let proposal = OperationDraft::solve(
        operator_shape,
        rhs_shape.clone(),
        solution_shape.clone(),
        ShapeLimits::default(),
    )?;
    result
        .timings
        .push(("input-and-shape", started.elapsed().as_secs_f64()));
    let mut factor = None;
    let mut outputs = Vec::new();
    let mut physical = Vec::new();
    for gain in [1_u8, 2, 3] {
        let rhs = mapped(
            &mut heap,
            (&declarations, &rhs_shape),
            source.value(),
            &FixedCoefficient::represented(Descriptor::ratio(), f64::from(gain))?,
            &mut result,
            &format!("rhs-gain-{gain}"),
        )?;
        let output = solve(
            &mut heap,
            &proposal,
            [operator.value(), rhs],
            gain,
            &mut factor,
            &mut result,
        )?;
        let output = expand(
            &mut heap,
            (&declarations, &solution_shape),
            &whole,
            output,
            gain,
            &mut result,
        )?;
        physical.push(super::restore::blocks(
            &mut heap,
            (&declarations, &whole),
            output,
            gain,
            &mut result,
        )?);
        outputs.push(output);
    }
    cross_blocks(&mut heap, &outputs, &physical, &mut result)?;
    aggregate(&mut heap, &outputs, (&declarations, &whole), &mut result)?;
    result.finish(&heap);
    result
        .timings
        .push(("total", started.elapsed().as_secs_f64()));
    Ok(result)
}

/// Keep all ordered point pairs and the physical voltage/current cross block.
fn cross_blocks(
    heap: &mut Heap,
    outputs: &[Handle<Value, f64>],
    physical: &[[Handle<Value, f64>; 2]],
    result: &mut Case,
) -> ResultOf {
    let phase = Instant::now();
    for &left in outputs {
        for &right in outputs {
            let cross = heap.materialize_cross_covariance(left, right, JointLimits::default())?;
            result
                .cross
                .push(capture::parameter(heap.moments(cross)?.covariance())?);
        }
    }
    let cross =
        heap.materialize_cross_covariance(physical[0][0], physical[2][1], JointLimits::default())?;
    result
        .cross
        .push(capture::parameter(heap.moments(cross)?.covariance())?);
    result
        .timings
        .push(("ten-cross-blocks", phase.elapsed().as_secs_f64()));
    Ok(())
}

/// Reconstruct (V1,V2,V3,V4,I) exactly from the three nodal unknowns before physical restoration.
fn expand(
    heap: &mut Heap,
    contract: (&Rc<CheckedDeclarations>, &CheckedShape),
    whole: &CheckedShape,
    input: Handle<Value, f64>,
    gain: u8,
    result: &mut Case,
) -> ResultOf<Handle<Value, f64>> {
    let (declarations, shape) = contract;
    let operator = shapes::dense(
        declarations,
        shape.codomain(),
        whole.codomain(),
        &Descriptor::ratio(),
    )?;
    let coefficients = [
        0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 0.0, -2.0,
    ]
    .into_iter()
    .map(|value| FixedCoefficient::represented(Descriptor::ratio(), value))
    .collect::<Result<_, _>>()?;
    let map = LinearMap::new(
        operator,
        shape.clone(),
        coefficients,
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    evaluate(
        heap,
        OperationDraft::linear_map(map),
        vec![input],
        result,
        &format!("solution-gain-{gain}"),
    )
}

/// One factor serves three RHS problems; every result recomputes its complete certificate.
fn solve(
    heap: &mut Heap,
    proposal: &OperationDraft,
    inputs: [Handle<Value, f64>; 2],
    gain: u8,
    factor: &mut Option<FactorRef<f64>>,
    result: &mut Case,
) -> ResultOf<Handle<Value, f64>> {
    let budget =
        SolveBudget::frobenius(Descriptor::ratio(), Budget::new(0.000_000_001, 0.0, 1.0)?)?;
    let keys = SolveRecordKeys::new(
        Some("resistor-common-source".into()),
        Some(format!("gain-{gain}")),
        Some(gain.to_string()),
        Some("solve".into()),
    )?;
    let policy = SolvePolicy::with_budget(budget).with_record_keys(keys);
    let limits = ExecutionLimits::default();
    let operation = proposal.clone().with_solve_policy(policy)?.check(limits)?;
    let bound = Binding::new(operation, inputs.to_vec(), limits).validate(heap)?;
    let work = bound.statistical_work();
    if factor.is_none() {
        let phase = Instant::now();
        *factor = Some(bound.factor()?);
        result.factors += 1;
        result
            .timings
            .push(("factor-and-inverse-witness", phase.elapsed().as_secs_f64()));
    }
    let phase = Instant::now();
    let candidate = bound.evaluate_factored(
        &bound.normalized_inputs()?,
        factor.as_ref().ok_or("factor")?,
    )?;
    let outcome = bound.publish(candidate)?;
    result.timings.push((
        "solve-certificate-moments-publication",
        phase.elapsed().as_secs_f64(),
    ));
    result.samples.push(capture::sample(
        heap,
        &outcome,
        &format!("reduced-solution-gain-{gain}"),
        work,
    )?);
    Ok(outcome.output().first().ok_or("solution")?.value())
}

/// A fixed diagonal affine map preserves the original source model and all cross-point dependence.
fn mapped(
    heap: &mut Heap,
    contract: (&Rc<CheckedDeclarations>, &CheckedShape),
    input: Handle<Value, f64>,
    scale: &FixedCoefficient,
    result: &mut Case,
    name: &str,
) -> ResultOf<Handle<Value, f64>> {
    let (declarations, shape) = contract;
    let order = shape.extent().rows();
    let operator = shapes::dense(
        declarations,
        shape.codomain(),
        shape.codomain(),
        &Descriptor::ratio(),
    )?;
    let mut coefficients =
        vec![FixedCoefficient::represented(Descriptor::ratio(), 0.0)?; order * order];
    for entry in 0..order {
        coefficients[entry + order * entry].clone_from(scale);
    }
    let map = LinearMap::new(
        operator,
        shape.clone(),
        coefficients,
        Vec::new(),
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    evaluate(
        heap,
        OperationDraft::linear_map(map),
        vec![input],
        result,
        name,
    )
}

/// Evaluate a public affine transaction, exposing its exact charged allowance.
pub(crate) fn evaluate(
    heap: &mut Heap,
    draft: OperationDraft,
    inputs: Vec<Handle<Value, f64>>,
    result: &mut Case,
    name: &str,
) -> ResultOf<Handle<Value, f64>> {
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft.check(limits)?, inputs, limits).validate(heap)?;
    let work = bound.statistical_work();
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    result
        .samples
        .push(capture::sample(heap, &outcome, name, work)?);
    Ok(outcome.output().first().ok_or("affine output")?.value())
}

/// Mean membership and summation order are explicit: ((gain 1 + gain 2) + gain 3) / 3.
fn aggregate(
    heap: &mut Heap,
    outputs: &[Handle<Value, f64>],
    contract: (&Rc<CheckedDeclarations>, &CheckedShape),
    result: &mut Case,
) -> ResultOf {
    let started = Instant::now();
    let shape = contract.1;
    let handles: [Handle<Value, f64>; 3] = outputs.try_into().map_err(|_| "three gains")?;
    let sum = evaluate(
        heap,
        OperationDraft::add(shape.clone(), shape.clone(), ShapeLimits::default())?,
        vec![handles[0], handles[1]],
        result,
        "sum-1-2",
    )?;
    let sum = evaluate(
        heap,
        OperationDraft::add(shape.clone(), shape.clone(), ShapeLimits::default())?,
        vec![sum, handles[2]],
        result,
        "sum-1-2-3",
    )?;
    mapped(heap, contract, sum, &third(shape)?, result, "mean")?;
    result
        .timings
        .push(("ordered-mean", started.elapsed().as_secs_f64()));
    Ok(())
}

/// Exact descriptor algebra defines one third; no stored data enters these operations.
fn third(shape: &CheckedShape) -> ResultOf<FixedCoefficient> {
    let unit = Measure::coherent();
    let increment = Conversion {
        scale: unit.scale().clone(),
        shift: unit.scale().clone(),
    };
    let three = increment.apply(&increment.apply(unit.scale())?)?;
    let triple = Measure::new("three", Scale::Exact(three), unit.offset().clone())?;
    let conversion = unit.conversion_to(&triple, Role::Difference)?;
    let ratio = Descriptor::ratio();
    let descriptor = Descriptor::new(
        ratio.kind().clone(),
        Measure::new(
            "third",
            Scale::Exact(conversion.scale),
            unit.offset().clone(),
        )?,
        Role::Difference,
    )?;
    let exact = shape.declarations().conversion_map(&descriptor, &ratio)?;
    Ok(FixedCoefficient::descriptor(ratio, exact.scale().clone())?)
}
