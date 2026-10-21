// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh expected claims and independent traversal retain the original symbolic source.

use crate::graph::{Cas, Handle, Vertex};
use crate::limits::{Budget, Limits};
use crate::proof_model::{Judgment, Statement};
use crate::solving::{
    SolveAnswer, SolveForm, SolveStatement, StatementCoverage, StatementPresentation,
};
use crate::testing::common::TestResult;
use petgraph::visit::EdgeRef;
use std::collections::BTreeSet;

type Solved = (super::source::Fixture, crate::request::Request, SolveAnswer);

/// Search receives only the original symbolic source and its fixed acceptance limits.
pub(crate) fn solved(keys: &[&str]) -> TestResult<Solved> {
    let (mut fixture, request) = super::source::symbolic(keys)?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("symbolic acceptance: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    assert_eq!(answer.unresolved_regions, [] as [Handle; 0]);
    assert!(
        answer
            .coverage
            .as_ref()
            .is_some_and(crate::proof::VerifiedWitness::is_discharged)
    );
    Ok((fixture, request, answer))
}

/// Expected source problem and region are built before importing either claimed theorem.
pub(crate) fn fresh(cas: &Cas, answer: &SolveAnswer, keys: &[&str]) -> TestResult {
    let (mut fixture, request) = super::source::symbolic(keys)?;
    let crate::request::Task::Solve(original) = &request.task else {
        return Err("fresh source".into());
    };
    let coverage = answer.coverage.as_ref().ok_or("fresh coverage")?;
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let imported = fixture
        .cas
        .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    let Statement::Solve(statement) = &imported.judgment().statement else {
        return Err("fresh solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("fresh coverage claim".into());
    };
    let set = claim.set.clone();
    let expected = judgment(
        &fixture,
        SolveStatement::Coverage(StatementCoverage {
            problem: original.problem.clone(),
            set: set.clone(),
            region: fixture.region.clone(),
        }),
    );
    replay(&mut fixture.cas, &bytes, &expected)?;
    let presentation = answer.facts.last().ok_or("fresh presentation")?;
    let bytes = cas.export_witness(presentation, &mut Budget::new(Limits::blind_cubic())?)?;
    let expected = judgment(
        &fixture,
        SolveStatement::Presentation(StatementPresentation {
            set,
            form: SolveForm::Radicals,
            region: fixture.region.clone(),
        }),
    );
    replay(&mut fixture.cas, &bytes, &expected)
}

fn judgment(fixture: &super::source::Fixture, claim: SolveStatement) -> Judgment {
    Judgment {
        context: fixture.context.clone(),
        layer: crate::domain::Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(claim)),
    }
}

fn replay(cas: &mut Cas, bytes: &[u8], expected: &Judgment) -> TestResult {
    let checked = cas.replay(bytes, expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(checked.is_discharged());
    assert_eq!(
        checked.hypotheses(),
        [] as [crate::domain::HypothesisRef; 0]
    );
    assert_eq!(
        cas.export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Walk every dependency edge, including literal attachments and original occurrence receipts.
fn free_keys(cas: &Cas, root: &Handle) -> TestResult<BTreeSet<String>> {
    let mut pending = vec![
        *cas.state
            .indices
            .get(&root.logical_id())
            .ok_or("source index")?,
    ];
    let mut seen = BTreeSet::new();
    let mut keys = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        if let Some(Vertex::Expression(expression)) = cas.state.graph.node_weight(index)
            && let crate::model::Operator::Free(key) = &expression.operator
        {
            keys.insert(key.clone());
        }
        for edge in cas.state.graph.edges(index) {
            pending.push(edge.target());
        }
    }
    Ok(keys)
}

/// Rebuild a genuinely different source by changing one original coefficient occurrence.
fn changed(
    fixture: &mut super::source::Fixture,
    keys: &[&str],
    position: usize,
) -> TestResult<Handle> {
    use crate::model::Operator;
    let unknown = fixture.free("x")?;
    let mut expression = fixture.integer(0, false)?;
    for (index, key) in keys.iter().enumerate().rev() {
        let product = fixture.expression(Operator::Mul, &[expression, unknown.clone()], false)?;
        let mut coefficient = fixture.free(key)?;
        if index == position {
            assert_eq!(
                fixture.cas.expression(&coefficient)?.0,
                &Operator::Free((*key).into())
            );
            let one = fixture.integer(1, false)?;
            let replacement =
                fixture.expression(Operator::Add, &[coefficient.clone(), one], false)?;
            assert_ne!(replacement, coefficient);
            coefficient = replacement;
        }
        expression = fixture.expression(Operator::Add, &[product, coefficient], false)?;
    }
    let zero = fixture.integer(0, false)?;
    let predicate = fixture.equation(&expression, &zero)?;
    fixture.problem("x", &predicate, &[])
}

/// A theorem for the old equation must reject each distinct expected equation, even on zero cells.
fn reject_changed(cas: &Cas, answer: &SolveAnswer, keys: &[&str]) -> TestResult {
    let coverage = answer.coverage.as_ref().ok_or("source coverage")?;
    let bytes = cas.export_witness(coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    for position in 0..keys.len() {
        let (mut fresh, request) = super::source::symbolic(keys)?;
        let crate::request::Task::Solve(original) = &request.task else {
            return Err("source expected problem".into());
        };
        let replacement = changed(&mut fresh, keys, position)?;
        assert_ne!(replacement, original.problem);
        let imported = fresh
            .cas
            .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
        let Statement::Solve(statement) = &imported.judgment().statement else {
            return Err("source imported claim".into());
        };
        let SolveStatement::Coverage(claim) = statement.as_ref() else {
            return Err("source coverage claim".into());
        };
        assert_eq!(claim.problem, original.problem);
        let expected = judgment(
            &fresh,
            SolveStatement::Coverage(StatementCoverage {
                problem: replacement,
                set: claim.set.clone(),
                region: fresh.region.clone(),
            }),
        );
        assert!(
            fresh
                .cas
                .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)
                .is_err()
        );
    }
    Ok(())
}

/// All original parameter occurrences survive in both source and returned closure under either naming.
pub(crate) fn source() -> TestResult {
    for keys in [["d", "c", "b", "a"], ["delta", "gamma", "beta", "alpha"]] {
        let (fixture, request, answer) = solved(&keys)?;
        let expected: BTreeSet<_> = keys
            .iter()
            .copied()
            .chain(std::iter::once("x"))
            .map(str::to_owned)
            .collect();
        assert_eq!(
            free_keys(
                &fixture.cas,
                &request.inputs.first().ok_or("source receipt")?.root
            )?,
            expected
        );
        assert_eq!(
            free_keys(
                &fixture.cas,
                answer.coverage.as_ref().ok_or("source result")?.proof()
            )?,
            expected
        );
        fresh(&fixture.cas, &answer, &keys)?;
        reject_changed(&fixture.cas, &answer, &keys)?;
    }
    Ok(())
}
