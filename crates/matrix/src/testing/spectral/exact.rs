// SPDX-License-Identifier: AGPL-3.0-only
//! Matrix's independently composed original similarity uses only public bindings/CAS APIs.

use super::{
    TestResult,
    source::{Fixture, expression},
};
use nrz_bindings::{
    Budget, Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale,
    readings::algebra::{
        ExactConstant, Handle, Rational,
        readings::{AffineMap, ExactAffineMap},
        syntax::{Field, Operator},
    },
    readings::{
        CoordinateNormalization, EntryMeaning, PositiveReciprocalMap, ReciprocalMapDefinition,
    },
};

/// Explicit fixed source-free reference definitions never inspect numeric input centers.
pub(crate) fn meaning(
    name: &str,
    dimensions: [i64; 7],
    text: &str,
    budget: &mut Budget,
) -> TestResult<EntryMeaning> {
    let descriptor = Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(dimensions)?)),
        Measure::coherent(),
        Role::Difference,
    )?;
    let reference = ReferenceDefinition::new(
        name,
        &descriptor,
        Scale::Exact(Rational::parse(text, budget)?),
    )?;
    Ok(EntryMeaning::new(descriptor, reference, None, budget)?)
}

/// The independent original composition and the proposed reciprocal rows meet only in the checker.
pub(super) fn recipe(
    source: &mut Fixture,
    context: &Handle,
    state: &CoordinateNormalization,
    time: &EntryMeaning,
    budget: &mut Budget,
) -> TestResult<PositiveReciprocalMap> {
    let entries = [
        meaning("inductance", [2, 1, -2, -2, 0, 0, 0], "1/40", budget)?,
        meaning("capacitance", [-2, -1, 4, 2, 0, 0, 0], "1/100000", budget)?,
    ];
    let frame = CoordinateNormalization::new(&mut source.cas, context, &entries, budget)
        .map_err(|error| format!("input reference inversion: {error:?}"))?;
    let inductance = source.real.clone();
    let capacitance = expression(source, Field::Real, Operator::Free("C".into()), &[], budget)?;
    let original = original(
        source,
        context,
        state,
        time.magnitude(),
        [&inductance, &capacitance],
        budget,
    )
    .map_err(|error| format!("original similarity composition: {error:?}"))?;
    let upper = time
        .magnitude()
        .mul(state.entries()[1].magnitude(), budget)?
        .div(state.entries()[0].magnitude(), budget)?
        .div(entries[1].magnitude(), budget)?;
    let lower = time
        .magnitude()
        .mul(state.entries()[0].magnitude(), budget)?
        .div(state.entries()[1].magnitude(), budget)?
        .div(entries[0].magnitude(), budget)?;
    let upper = upper.as_rational().ok_or("rational upper")?.clone();
    let lower = lower
        .as_rational()
        .ok_or("rational lower")?
        .negated(budget)?;
    let zero = Rational::zero();
    let map = AffineMap::new(
        2,
        4,
        vec![
            zero.clone(),
            zero.clone(),
            lower,
            zero.clone(),
            zero.clone(),
            upper,
            zero.clone(),
            zero.clone(),
        ],
        vec![zero; 4],
        budget,
    )?;
    let definition = ReciprocalMapDefinition {
        inputs: vec![inductance, capacitance],
        shape: [2, 2],
        map,
        original,
    };
    Ok(
        PositiveReciprocalMap::prove(&mut source.cas, &frame, definition, budget)
            .map_err(|error| format!("reciprocal correspondence: {error:?}"))?,
    )
}

/// Exact `t*S^(-1)*A*S` uses native map composition independently of flattened recipe rows.
fn original(
    source: &mut Fixture,
    context: &Handle,
    state: &CoordinateNormalization,
    time: &ExactConstant,
    inputs: [&Handle; 2],
    budget: &mut Budget,
) -> TestResult<ExactAffineMap> {
    let zero = expression(
        source,
        Field::Real,
        Operator::Rational(Rational::zero()),
        &[],
        budget,
    )?;
    let one = expression(
        source,
        Field::Real,
        Operator::Rational(Rational::one()),
        &[],
        budget,
    )?;
    let upper = expression(
        source,
        Field::Real,
        Operator::Div,
        &[one.clone(), inputs[1].clone()],
        budget,
    )?;
    let reciprocal = expression(
        source,
        Field::Real,
        Operator::Div,
        &[one, inputs[0].clone()],
        budget,
    )?;
    let lower = expression(source, Field::Real, Operator::Neg, &[reciprocal], budget)?;
    let physical = source.cas.exact_reading_map(
        context,
        (2, 2),
        &[zero.clone(), upper, lower, zero.clone()],
        &[zero.clone(), zero],
        budget,
    )?;
    let right = source
        .cas
        .compose_reading_maps(&physical, state.restoration(), budget)?;
    let similarity = source
        .cas
        .compose_reading_maps(state.inverse().map(), &right, budget)?;
    let scale = source.cas.constant_reading_map(
        context,
        (2, 2),
        &[
            time.clone(),
            ExactConstant::zero(),
            ExactConstant::zero(),
            time.clone(),
        ],
        &[ExactConstant::zero(), ExactConstant::zero()],
        budget,
    )?;
    Ok(source
        .cas
        .compose_reading_maps(&scale, &similarity, budget)?)
}
