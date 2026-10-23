// SPDX-License-Identifier: AGPL-3.0-only
//! Public LC recipe fixtures; original similarity and numerical oracle are constructed independently.

use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, real};
use nrz_bindings::readings::algebra::{
    ExactConstant, Handle, Rational, Reference,
    domain::{AssumptionDraft, ContextDraft},
    readings::ExactAffineMap,
    syntax::{Draft, Operator, PredicateKind},
};
use nrz_bindings::readings::{
    CoordinateNormalization, EntryMeaning, PositiveReciprocalMap, ReciprocalMapDefinition,
};
use nrz_bindings::{Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale};
use nrz_matrix::{CheckedShape, OperationDraft};
use nrz_scalar::LiteralLimits;

/// Exact graph, physical input references and independently written original LC similarity.
pub(crate) struct FixturePlan {
    /// Original graph and all mathematical declarations.
    pub(crate) case: Fixture,
    /// Formal L/C references derived from the fixed state and time references.
    pub(crate) frame: CoordinateNormalization,
    /// Entire proposed numerical coefficient recipe, with the independently composed original.
    pub(crate) definition: ReciprocalMapDefinition,
}

/// Original positive L/C stratum and the fixed state/time references precede numerical values.
pub(crate) fn plan() -> TestResult<FixturePlan> {
    let mut case = Fixture::new(&[
        ("L", real()),
        ("C", real()),
        ("b", matrix(2, 2)),
        ("B", matrix(2, 2)),
    ])?;
    positive(&mut case)?;
    let state = [
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0], "5")?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0], "1/10")?,
    ];
    let frame = CoordinateNormalization::new(&mut case.cas, &case.context, &state, &mut ledger()?)?;
    let time = meaning("duration", [0, 0, 1, 0, 0, 0, 0], "1/2000")?;
    let original = original(&mut case, &frame, time.magnitude())?;
    let entries = [
        meaning("inductance", [2, 1, -2, -2, 0, 0, 0], "1/40")?,
        meaning("capacitance", [-2, -1, 4, 2, 0, 0, 0], "1/100000")?,
    ];
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, &entries, &mut ledger()?)?;
    let inputs = vec![case.symbol("L")?, case.symbol("C")?];
    let definition = ReciprocalMapDefinition {
        inputs,
        shape: [2, 2],
        map: map(2, 4, &[0, 0, -1, 0, 0, 1, 0, 0], &[0; 4])?,
        original,
    };
    Ok(FixturePlan {
        case,
        frame,
        definition,
    })
}

/// Explicit model positivity is retained by CAS; runtime still has to prove its whole-box counterpart.
fn positive(case: &mut Fixture) -> TestResult {
    let header = case.cas.context_header(&case.context)?;
    let domain = case.cas.admissible_region(&case.context)?;
    let mut assumptions = Vec::new();
    for key in ["L", "C"] {
        let term = case.symbol(key)?;
        let condition = case
            .cas
            .admit(
                &[Draft::Condition {
                    kind: PredicateKind::Positive,
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    arguments: vec![Reference::Existing(term)],
                }],
                &mut ledger()?,
            )?
            .pop()
            .ok_or("condition")?;
        assumptions.push(AssumptionDraft {
            key: key.into(),
            condition: Reference::Existing(condition),
        });
    }
    case.context = case
        .cas
        .admit(
            &[Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header),
                admissible: Reference::Existing(domain),
                assumptions,
            })],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("context")?;
    Ok(())
}

/// Native exact composition follows `t_ref*S^(-1)*A*S`, independently of flattened recipe rows.
fn original(
    case: &mut Fixture,
    frame: &CoordinateNormalization,
    time: &ExactConstant,
) -> TestResult<ExactAffineMap> {
    let zero = term(case, Operator::Rational(Rational::zero()), &[])?;
    let one = term(case, Operator::Rational(Rational::one()), &[])?;
    let capacitance = case.symbol("C")?;
    let inductance = case.symbol("L")?;
    let upper = term(case, Operator::Div, &[one.clone(), capacitance])?;
    let inverse = term(case, Operator::Div, &[one, inductance])?;
    let lower = term(case, Operator::Neg, &[inverse])?;
    let physical = case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &[zero.clone(), upper, lower, zero.clone()],
        &[zero.clone(), zero],
        &mut ledger()?,
    )?;
    let right = case
        .cas
        .compose_reading_maps(&physical, frame.restoration(), &mut ledger()?)?;
    let similarity =
        case.cas
            .compose_reading_maps(frame.inverse().map(), &right, &mut ledger()?)?;
    let scaled_time = case.cas.constant_reading_map(
        &case.context,
        (2, 2),
        &[
            time.clone(),
            ExactConstant::zero(),
            ExactConstant::zero(),
            time.clone(),
        ],
        &[ExactConstant::zero(), ExactConstant::zero()],
        &mut ledger()?,
    )?;
    Ok(case
        .cas
        .compose_reading_maps(&scaled_time, &similarity, &mut ledger()?)?)
}

/// Typed source-free references never depend on an input's numerical center.
pub(crate) fn meaning(name: &str, dimensions: [i64; 7], value: &str) -> TestResult<EntryMeaning> {
    let descriptor = Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(dimensions)?)),
        Measure::coherent(),
        Role::Difference,
    )?;
    let reference = ReferenceDefinition::new(
        name,
        &descriptor,
        Scale::Exact(Rational::parse(value, &mut ledger()?)?),
    )?;
    Ok(EntryMeaning::new(
        descriptor,
        reference,
        None,
        &mut ledger()?,
    )?)
}

/// Exact terms use the same original header and never inspect heap data.
fn term(case: &mut Fixture, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
    use nrz_bindings::readings::algebra::syntax::ExpressionDraft;
    let header = case.cas.context_header(&case.context)?;
    Ok(case
        .cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                operator,
                sort: real(),
                header: Reference::Existing(header),
                scope: Vec::new(),
                binding: None,
                space: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            })],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("term")?)
}

/// The exact correspondence is independently checked before any matrix execution contract exists.
pub(crate) fn proved() -> TestResult<(FixturePlan, PositiveReciprocalMap)> {
    let mut prepared = plan()?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition.clone(),
        &mut ledger()?,
    )?;
    Ok((prepared, recipe))
}

/// Native inputs keep L/C meanings separately; the complete normalized output is dimensionless.
pub(crate) fn shapes(
    recipe: &PositiveReciprocalMap,
) -> TestResult<(Vec<CheckedShape>, CheckedShape)> {
    let owner = crate::conversion_testing::owner()?;
    let inputs = recipe
        .frame()
        .entries()
        .iter()
        .map(|entry| crate::conversion_testing::shape(&owner, entry.descriptor(), 1))
        .collect::<TestResult<Vec<_>>>()?;
    let [rows, columns] = recipe.shape();
    let domain = crate::kernel_testing::axis(usize::try_from(columns)?)?;
    let codomain = crate::kernel_testing::axis(usize::try_from(rows)?)?;
    let output = crate::kernel_testing::shape(&owner, &domain, &codomain)?;
    Ok((inputs, output))
}

/// Build the closed public operation without supplying numerical input values to exact algebra.
pub(crate) fn draft(
    prepared: &FixturePlan,
    recipe: &PositiveReciprocalMap,
) -> TestResult<OperationDraft> {
    let (inputs, output) = shapes(recipe)?;
    Ok(OperationDraft::positive_reciprocal_map(
        &prepared.case.cas,
        recipe.clone(),
        inputs,
        output,
        LiteralLimits::default(),
        &mut ledger()?,
    )?)
}

/// A homogeneous two-entry block exercises dense mixing, signs and an offset-only row.
pub(crate) fn block_plan() -> TestResult<FixturePlan> {
    let mut case = Fixture::new(&[("L", real()), ("C", real())])?;
    positive(&mut case)?;
    let mut entries = Vec::new();
    for text in ["2", "3"] {
        let descriptor = Descriptor::ratio();
        let reference = ReferenceDefinition::new(
            text,
            &descriptor,
            Scale::Exact(Rational::parse(text, &mut ledger()?)?),
        )?;
        entries.push(EntryMeaning::new(
            descriptor,
            reference,
            None,
            &mut ledger()?,
        )?);
    }
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, &entries, &mut ledger()?)?;
    let original = block_original(&mut case)?;
    let inputs = vec![case.symbol("L")?, case.symbol("C")?];
    let definition = ReciprocalMapDefinition {
        inputs,
        shape: [2, 2],
        map: map(2, 4, &[1, 2, -1, 1, 0, 0, 3, -2], &[0, 0, 5, 0])?,
        original,
    };
    Ok(FixturePlan {
        case,
        frame,
        definition,
    })
}

/// Independent matrix entries never inspect the proposed affine coefficient array.
fn block_original(case: &mut Fixture) -> TestResult<ExactAffineMap> {
    let first = case.symbol("L")?;
    let second = case.symbol("C")?;
    let mut values = Vec::new();
    for (left, right, offset) in [(2, 6, 0), (0, 0, 5), (-2, 3, 0), (6, -6, 0)] {
        let mut terms = Vec::new();
        for (numerator, denominator) in [(left, &first), (right, &second)] {
            let value = term(
                case,
                Operator::Rational(Rational::from_ratio(numerator, 1, &mut ledger()?)?),
                &[],
            )?;
            terms.push(term(case, Operator::Div, &[value, denominator.clone()])?);
        }
        let sum = term(case, Operator::Add, &terms)?;
        let shift = term(
            case,
            Operator::Rational(Rational::from_ratio(offset, 1, &mut ledger()?)?),
            &[],
        )?;
        values.push(term(case, Operator::Add, &[sum, shift])?);
    }
    let zero = term(case, Operator::Rational(Rational::zero()), &[])?;
    Ok(case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &values,
        &[zero.clone(), zero],
        &mut ledger()?,
    )?)
}

#[path = "reciprocal/readings.rs"]
mod readings;
pub(crate) use readings::coverage;

/// An original zero term keeps an undeclared denominator even though coefficient equality cancels it.
pub(crate) fn hidden_denominator() -> TestResult<(FixturePlan, ExactAffineMap)> {
    let mut prepared = plan()?;
    let entries = [prepared.frame.entries()[0].clone()];
    prepared.frame = CoordinateNormalization::new(
        &mut prepared.case.cas,
        &prepared.case.context,
        &entries,
        &mut ledger()?,
    )?;
    let input = prepared.case.symbol("L")?;
    let extra = prepared.case.symbol("C")?;
    let numerator = term(
        &mut prepared.case,
        Operator::Rational(Rational::parse("1/40", &mut ledger()?)?),
        &[],
    )?;
    let value = term(
        &mut prepared.case,
        Operator::Div,
        &[numerator, input.clone()],
    )?;
    let zero = term(
        &mut prepared.case,
        Operator::Rational(Rational::zero()),
        &[],
    )?;
    let cancelled = term(&mut prepared.case, Operator::Div, &[zero.clone(), extra])?;
    let original = term(
        &mut prepared.case,
        Operator::Add,
        &[value.clone(), cancelled],
    )?;
    let context = &prepared.case.context;
    let target = prepared.case.cas.exact_reading_map(
        context,
        (1, 1),
        &[value],
        std::slice::from_ref(&zero),
        &mut ledger()?,
    )?;
    let original = prepared.case.cas.exact_reading_map(
        context,
        (1, 1),
        &[original],
        &[zero],
        &mut ledger()?,
    )?;
    prepared.definition = ReciprocalMapDefinition {
        inputs: vec![input],
        shape: [1, 1],
        map: map(1, 1, &[1], &[0])?,
        original,
    };
    Ok((prepared, target))
}

/// A source-free degree reference remains exact until the separate numerical export boundary.
pub(crate) fn irrational_plan() -> TestResult<FixturePlan> {
    let mut prepared = plan()?;
    let registry =
        nrz_bindings::Registry::with_angle_frequency(nrz_bindings::ExponentPolicy::Rational)?;
    let degree = registry.measure("degree").ok_or("degree")?;
    let reference = ReferenceDefinition::unit("degree-reference", degree)?;
    let entry = EntryMeaning::new(degree.coherent(), reference, None, &mut ledger()?)?;
    let magnitude = entry.magnitude().clone();
    prepared.frame = CoordinateNormalization::new(
        &mut prepared.case.cas,
        &prepared.case.context,
        &[entry],
        &mut ledger()?,
    )?;
    let input = prepared.case.symbol("L")?;
    let one = term(&mut prepared.case, Operator::Rational(Rational::one()), &[])?;
    let zero = term(
        &mut prepared.case,
        Operator::Rational(Rational::zero()),
        &[],
    )?;
    let inverse = term(&mut prepared.case, Operator::Div, &[one, input.clone()])?;
    let base = prepared.case.cas.exact_reading_map(
        &prepared.case.context,
        (1, 1),
        &[inverse],
        &[zero],
        &mut ledger()?,
    )?;
    let scale = prepared.case.cas.constant_reading_map(
        &prepared.case.context,
        (1, 1),
        &[magnitude],
        &[ExactConstant::zero()],
        &mut ledger()?,
    )?;
    let original = prepared
        .case
        .cas
        .compose_reading_maps(&scale, &base, &mut ledger()?)?;
    prepared.definition = ReciprocalMapDefinition {
        inputs: vec![input],
        shape: [1, 1],
        map: map(1, 1, &[1], &[0])?,
        original,
    };
    Ok(prepared)
}

/// A zero coefficient column still binds and guards the second original positive input.
pub(crate) fn unused_input_plan() -> TestResult<FixturePlan> {
    let mut prepared = plan()?;
    let first = prepared.case.symbol("L")?;
    let numerator = term(
        &mut prepared.case,
        Operator::Rational(Rational::parse("1/40", &mut ledger()?)?),
        &[],
    )?;
    let value = term(&mut prepared.case, Operator::Div, &[numerator, first])?;
    let zero = term(
        &mut prepared.case,
        Operator::Rational(Rational::zero()),
        &[],
    )?;
    prepared.definition.original = prepared.case.cas.exact_reading_map(
        &prepared.case.context,
        (1, 1),
        &[value],
        &[zero],
        &mut ledger()?,
    )?;
    prepared.definition.shape = [1, 1];
    prepared.definition.map = map(2, 1, &[1, 0], &[0])?;
    Ok(prepared)
}

#[path = "reciprocal/oracle.rs"]
pub(crate) mod oracle;
