// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC coverage and independently composed state/time transport use public APIs only.

#[path = "spectral/real.rs"]
mod real;

use super::lc_roots::{self, Fixture, TestResult, expression};
use nrz_bindings::{
    Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale,
    readings::{
        CoordinateNormalization, EntryMeaning, LcSpectralLaw, PositiveReciprocalMap,
        ReciprocalMapDefinition, RootCoverage, RootProblem,
    },
};
use nrz_cas::{
    ExactConstant, Handle, Rational,
    limits::{Budget, Limits},
    readings::{AffineMap, ExactAffineMap},
    syntax::{Field, Operator},
};

/// Unchanged production policy applies independently to each measured operation.
pub(crate) fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::default())?)
}

/// Original mathematical fixture and the exact objects needed for adversarial transports.
pub(crate) struct SpectralFixture {
    pub(crate) source: Fixture,
    pub(crate) coverage: RootCoverage,
    pub(crate) parameters: [Handle; 2],
    pub(crate) state: CoordinateNormalization,
    pub(crate) time: EntryMeaning,
    pub(crate) recipe: PositiveReciprocalMap,
    pub(crate) construction_work: u64,
}

impl SpectralFixture {
    pub(crate) fn check(&mut self, budget: &mut Budget) -> TestResult<LcSpectralLaw> {
        Ok(LcSpectralLaw::check(
            &mut self.source.cas,
            self.coverage.clone(),
            self.parameters.clone(),
            self.recipe.clone(),
            self.state.clone(),
            self.time.clone(),
            budget,
        )
        .map_err(|error| format!("closed spectral law: {error:?}"))?)
    }
}

/// Complete original replay precedes every new physical interpretation.
pub(crate) fn prepared(references: [&str; 3]) -> TestResult<SpectralFixture> {
    prepared_with_policy(references, Limits::default())
}

/// The construction honors the caller policy, including deliberately narrower bit boundaries.
pub(crate) fn prepared_with_policy(
    references: [&str; 3],
    limits: Limits,
) -> TestResult<SpectralFixture> {
    let theorem = lc_roots::proof()?;
    let (mut source, request) = lc_roots::original_with_policy(limits.clone(), &mut ledger()?)?;
    let set = lc_roots::proposal(&mut source, &theorem)?;
    let parameters = [
        source.real.clone(),
        expression(
            &mut source,
            Field::Real,
            Operator::Free("C".into()),
            &[],
            &mut ledger()?,
        )?,
    ];
    let original = RootProblem::check(&mut source.cas, request, &mut ledger()?)?;
    let coverage = RootCoverage::replay(
        &mut source.cas,
        &original,
        set,
        &theorem.bytes,
        &mut ledger()?,
    )
    .map_err(|error| format!("original root replay: {error:?}"))?;
    let mut construction = Budget::new(limits)?;
    let (state, time, recipe) = construct(&mut source, references, &mut construction)?;
    Ok(SpectralFixture {
        source,
        coverage,
        parameters,
        state,
        time,
        recipe,
        construction_work: construction.usage().work,
    })
}

/// One caller ledger measures the entire independent physical transport construction.
fn construct(
    source: &mut Fixture,
    references: [&str; 3],
    budget: &mut Budget,
) -> TestResult<(CoordinateNormalization, EntryMeaning, PositiveReciprocalMap)> {
    let context = real::context(source, budget)?;
    let entries = [
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0], references[0], budget)?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0], references[1], budget)?,
    ];
    let state = CoordinateNormalization::new(&mut source.cas, &context, &entries, budget)
        .map_err(|error| format!("state reference inversion: {error:?}"))?;
    let time = meaning("duration", [0, 0, 1, 0, 0, 0, 0], references[2], budget)?;
    let recipe = recipe(source, &context, &state, &time, budget)?;
    Ok((state, time, recipe))
}

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
fn recipe(
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
