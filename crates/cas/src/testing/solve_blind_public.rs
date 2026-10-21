// SPDX-License-Identifier: AGPL-3.0-only
//! Source-only blind service fixtures; no expected radical formula is loaded during setup.

use crate::testing::common::TestResult;
#[path = "solve_blind/source.rs"]
mod input;
use crate::test_api::Reference;
use crate::test_api::limits::{Budget, Limits, LimitsOverride};
use crate::test_api::request::{CoveredInput, Request, Task};
use crate::test_api::solving::{ExclusionPolicy, SolveForm, SolveGoal, SolveSpec};
use crate::test_api::syntax::{
    AtomicSort, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort,
};
pub(crate) use input::Fixture;
use input::admit;

/// The request pins exactly the documented acceptance overrides, with ordinary defaults elsewhere.
fn profile(limits: &Limits) -> LimitsOverride {
    LimitsOverride {
        arithmetic_steps: Some(limits.arithmetic_steps),
        work: Some(limits.work),
        replay_work: Some(limits.replay_work),
        component_bits: Some(limits.component_bits),
        intermediate_bits: Some(limits.intermediate_bits),
        coefficient_ops: Some(limits.coefficient_ops),
        gcd_steps: Some(limits.gcd_steps),
        tower_reductions: Some(limits.tower_reductions),
        orbit_permutations: Some(limits.orbit_permutations),
        orbit_terms: Some(limits.orbit_terms),
        symmetric_steps: Some(limits.symmetric_steps),
        tower_levels: Some(limits.tower_levels),
        tower_degree: Some(limits.tower_degree),
        orbit_members: Some(limits.orbit_members),
        resolvent_depth: Some(limits.resolvent_depth),
        resolvent_degree: Some(limits.resolvent_degree),
        selection_slots: Some(limits.selection_slots),
        solution_branches: Some(limits.solution_branches),
        witness_bytes: Some(limits.witness_bytes),
        output_bytes: Some(limits.output_bytes),
        condition_atoms: Some(limits.condition_atoms),
        condition_steps: Some(limits.condition_steps),
        ..LimitsOverride::default()
    }
}

/// Horner construction uses only the supplied original integer coefficients and no root candidates.
pub(crate) fn polynomial(coefficients: &[i64]) -> TestResult<(Fixture, Request)> {
    let limits = degree_limits(coefficients.len());
    let mut fixture =
        Fixture::with_parameters(&Field::Complex, &["a", "b", "c", "d"], limits.clone())?;
    let unknown = fixture.free("x")?;
    let mut expression = fixture.integer(0, false)?;
    for coefficient in coefficients.iter().rev() {
        let product = fixture.expression(Operator::Mul, &[expression, unknown.clone()], false)?;
        let constant = fixture.integer(*coefficient, false)?;
        expression = fixture.expression(Operator::Add, &[product, constant], false)?;
    }
    request(fixture, expression, format!("{coefficients:?}"), &limits)
}

/// Only noncubic tests use a separate finite tower product ceiling.
fn degree_limits(count: usize) -> Limits {
    Limits {
        tower_degree: if count > 5 {
            32
        } else {
            Limits::blind_cubic().tower_degree
        },
        ..Limits::blind_cubic()
    }
}

/// Independent coefficient names enter the source exactly in ascending power order.
pub(crate) fn symbolic(keys: &[&str]) -> TestResult<(Fixture, Request)> {
    symbolic_in_field(keys, &Field::Complex)
}

/// Real and complex acceptance requests have separate explicit semantic headers.
pub(crate) fn symbolic_in_field(keys: &[&str], field: &Field) -> TestResult<(Fixture, Request)> {
    let limits = degree_limits(keys.len());
    let mut fixture = Fixture::with_parameters(field, keys, limits.clone())?;
    fixture.algebra = admit(
        &mut fixture.cas,
        &Draft::Solve(crate::test_api::solving::SolveDraft::CoefficientDomain(
            crate::test_api::solving::CoefficientDomainData::Fraction {
                base: Reference::Existing(fixture.algebra.clone()),
                coordinates: Reference::Existing(fixture.coefficients.clone()),
            },
        )),
    )?;
    let unknown = fixture.free("x")?;
    let mut expression = fixture.integer(0, false)?;
    for key in keys.iter().rev() {
        let product = fixture.expression(Operator::Mul, &[expression, unknown.clone()], false)?;
        let coefficient = fixture.free(key)?;
        expression = fixture.expression(Operator::Add, &[product, coefficient], false)?;
    }
    request(fixture, expression, format!("{keys:?}"), &limits)
}

/// Match the original Boolean occurrence and the problem predicate before requesting any search.
fn request(
    mut fixture: Fixture,
    expression: crate::test_api::Handle,
    text: String,
    limits: &Limits,
) -> TestResult<(Fixture, Request)> {
    let zero = fixture.integer(0, false)?;
    let predicate = fixture.equation(&expression, &zero)?;
    let problem = fixture.problem("x", &predicate, &[])?;
    let boolean = admit(
        &mut fixture.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(fixture.header.clone()),
            operator: Operator::Predicate(PredicateKind::Equal),
            sort: Sort::Value(AtomicSort::Boolean),
            operands: vec![Reference::Existing(expression), Reference::Existing(zero)],
            scope: Vec::new(),
            space: None,
            binding: None,
        }),
    )?;
    let source = admit(
        &mut fixture.cas,
        &Draft::Source {
            key: "blind-original".into(),
            text,
        },
    )?;
    let occurrence = fixture.cas.materialize_original(
        &boolean,
        &source,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let receipt = fixture
        .cas
        .walk_original(&occurrence, &mut Budget::new(Limits::blind_cubic())?)?
        .receipt;
    let request = Request {
        task: Task::Solve(Box::new(SolveSpec {
            problem,
            goal: SolveGoal::Complete,
            form: SolveForm::Radicals,
            exclusions: ExclusionPolicy::PreserveOriginal,
        })),
        context: fixture.context.clone(),
        inputs: vec![CoveredInput {
            root: occurrence,
            receipt,
        }],
        premises: Vec::new(),
        limits: profile(limits),
    };
    Ok((fixture, request))
}

#[path = "solve_blind/regional.rs"]
mod regional;
pub(crate) use regional::unit as regional_unit;
