// SPDX-License-Identifier: AGPL-3.0-only
//! Selected-root relation closure with independently specified nested arithmetic.

mod properties;

use crate::Rational;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::graph::{Cas, Handle};
use crate::proof_model::{CheckedApplication, Statement, VerifiedWitness};
use crate::solving::coefficients::{arithmetic::Engine, forms::Form};
use crate::solving::elementary::{evaluate::Evaluation, selected, value::Value};
use crate::solving::{SolveForm, SolveStatement};
use crate::testing::common::TestResult;

struct Fixture {
    cas: Cas,
    region: Handle,
    domain: Handle,
    roots: Vec<Handle>,
    premises: Vec<CheckedApplication>,
}

/// Copy checked existence conclusions only; no explicit scalar equality enters the evaluator.
fn existences(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Vec<CheckedApplication>> {
    let mut pending = vec![witness.proof.logical_id()];
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    let mut budget = Budget::new(Limits::blind_cubic())?;
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let application = crate::proof_view::application(cas, id, &mut budget)?;
        pending.extend(&application.premises);
        if let Statement::Solve(statement) = &application.judgment.statement
            && matches!(statement.as_ref(), SolveStatement::RootExistence(_))
        {
            result.push(application);
        }
    }
    Ok(result)
}

/// Use the original Real cubic, then discard every nonexistence fact, including the s²=D lemma.
fn fixture() -> TestResult<Fixture> {
    let (mut source, request) =
        crate::testing::solve_blind::real_polynomial(&[1, 1, 0, 1], SolveForm::AnyExact)?;
    let crate::request::Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("nested Real cubic was not solved".into());
    };
    let premises = existences(&source.cas, answer.coverage.as_ref().ok_or("coverage")?)?;
    let mut ordered = Vec::new();
    for premise in &premises {
        let Statement::Solve(statement) = &premise.judgment.statement else {
            return Err("existence statement".into());
        };
        let SolveStatement::RootExistence(existence) = statement.as_ref() else {
            return Err("existence conclusion".into());
        };
        let selection = selected::selection(&source.cas, &existence.relation)?;
        assert_eq!(selection.relations.len(), 1);
        let relation = selected::relation(&source.cas, &selection.relations[0])?;
        assert_eq!(relation.region, source.region);
        let polynomial = crate::solving::coefficients::storage::polynomial(
            &source.cas.state,
            &relation.polynomial,
        )?;
        let degree = polynomial.terms.iter().map(|term| term.exponents[0]).max();
        let root = crate::testing::solve_graph::admit(
            &mut source.cas,
            &crate::syntax::Draft::RootValue {
                header: crate::Reference::Existing(source.header.clone()),
                selection: crate::Reference::Existing(existence.relation.clone()),
                slot: 0,
            },
        )?;
        ordered.push((degree, root));
    }
    ordered.sort_by_key(|(degree, _)| *degree);
    assert_eq!(
        ordered
            .iter()
            .map(|(degree, _)| *degree)
            .collect::<Vec<_>>(),
        [Some(2), Some(3), Some(3)]
    );
    Ok(Fixture {
        cas: source.cas,
        region: source.region,
        domain: source.algebra,
        roots: ordered.into_iter().map(|(_, root)| root).collect(),
        premises,
    })
}

impl Fixture {
    fn evaluation(&self) -> Evaluation<'_> {
        Evaluation {
            cas: &self.cas,
            engine: Engine {
                state: &self.cas.state,
                domain: &self.domain,
            },
            region: &self.region,
            premises: &self.premises,
            free: &[],
            bound: &[],
        }
    }

    fn values(&self, budget: &mut Budget) -> TestResult<Vec<Value>> {
        self.roots
            .iter()
            .map(|root| {
                Ok(self
                    .evaluation()
                    .expression(root.logical_id(), true, budget)?)
            })
            .collect()
    }

    fn target(&self, budget: &mut Budget) -> TestResult<Value> {
        let values = self.values(budget)?;
        let engine = self.evaluation().engine;
        let product = values[1].pow(3, &engine, budget)?.mul(
            &values[2].pow(3, &engine, budget)?,
            &engine,
            budget,
        )?;
        Ok(product.add(&constant(&engine, 1, 27, budget)?, &engine, budget)?)
    }
}

fn constant(
    engine: &Engine<'_>,
    numerator: i64,
    denominator: u64,
    budget: &mut Budget,
) -> TestResult<Value> {
    let rational = Rational::from_ratio(numerator, denominator, budget)?;
    Ok(Value::polynomial(
        Form::constant(engine.rational(&rational, budget)?, engine, budget)?,
        engine,
        budget,
    )?)
}

/// The two cube roots introduce the shared square root, whose relation closes the measured remainder.
/// ´test:crate:nested-root-remainder-without-lemma´
#[test]
fn nested_root_remainder_without_lemma() -> TestResult {
    let fixture = fixture()?;
    let evaluation = fixture.evaluation();
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let target = fixture.target(&mut budget)?;
    let relations = evaluation.root_relations(&target.numerator, &mut budget)?;
    assert_eq!(relations.len(), 3);
    evaluation.zero(&target, &mut budget)?;
    let values = fixture.values(&mut budget)?;
    let center = constant(&evaluation.engine, -1, 2, &mut budget)?;
    let substituted = center
        .add(&values[0], &evaluation.engine, &mut budget)?
        .mul(
            &center.sub(&values[0], &evaluation.engine, &mut budget)?,
            &evaluation.engine,
            &mut budget,
        )?
        .add(
            &constant(&evaluation.engine, 1, 27, &mut budget)?,
            &evaluation.engine,
            &mut budget,
        )?;
    let remainder = constant(&evaluation.engine, 31, 108, &mut budget)?.sub(
        &values[0].pow(2, &evaluation.engine, &mut budget)?,
        &evaluation.engine,
        &mut budget,
    )?;
    assert!(substituted.equals(&remainder, &evaluation.engine, &mut budget)?);
    evaluation.zero(&remainder, &mut budget)?;
    Ok(())
}

/// Shared parent and self occurrences are expanded once; exact traversal Work is cumulative.
/// ´test:crate:root-closure-work-boundary´
#[test]
fn root_closure_work_boundary() -> TestResult {
    let fixture = fixture()?;
    let evaluation = fixture.evaluation();
    let target = fixture.target(&mut Budget::new(Limits::blind_cubic())?)?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    assert_eq!(
        evaluation
            .root_relations(&target.numerator, &mut measured)?
            .len(),
        3
    );
    let required = measured.usage().work;
    assert!(required > 1);
    for cap in [required, required - 1] {
        let mut budget = Budget::new(Limits {
            work: cap,
            ..Limits::blind_cubic()
        })?;
        let result = evaluation.root_relations(&target.numerator, &mut budget);
        if cap == required {
            assert_eq!(result?.len(), 3);
            assert_eq!(budget.usage().work, required);
        } else {
            let fault = result
                .err()
                .ok_or("closure accepted below its exact Work bound")?;
            assert!(matches!(fault.kind, crate::FaultKind::Exhausted {
                budget: BudgetKind::Work, limit, needed, ..
            } if limit == cap && needed == required));
        }
    }
    Ok(())
}

/// Inventory the exact Real theorem surface square equation separately from the evaluator's existence-only premises.
/// ´test:crate:single-root-square-lemma-inventory´
#[test]
fn single_root_square_lemma_inventory() -> TestResult {
    let fixture = fixture()?;
    let mut pending: Vec<_> = fixture
        .premises
        .iter()
        .flat_map(|premise| &premise.premises)
        .copied()
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut count = 0;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let application = crate::proof_view::application(&fixture.cas, id, &mut budget)?;
        pending.extend(&application.premises);
        if let Statement::Equality { left, right, .. } = &application.judgment.statement {
            let expression = fixture.cas.state.expression(left.logical_id())?;
            let squared = expression.operator == crate::model::Operator::IntegerPower(2);
            let exact_root = expression.operands == [fixture.roots[0].logical_id()];
            if squared && exact_root {
                let evaluation = fixture.evaluation();
                let actual = evaluation.expression(right.logical_id(), true, &mut budget)?;
                let expected = constant(&evaluation.engine, 31, 108, &mut budget)?;
                assert!(actual.equals(&expected, &evaluation.engine, &mut budget)?);
                count += 1;
            }
        }
    }
    assert_eq!(count, 0);
    Ok(())
}
