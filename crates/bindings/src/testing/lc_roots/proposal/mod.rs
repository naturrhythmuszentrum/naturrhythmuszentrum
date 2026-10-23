// SPDX-License-Identifier: AGPL-3.0-only
//! Copy only untrusted candidate syntax; no proof application is inspected or admitted.

#[path = "coefficients.rs"]
mod coefficients;
#[path = "expressions.rs"]
mod expressions;
#[path = "solving.rs"]
mod solving;

use super::{Fixture, TestResult, Theorem, admit, mutations, reference};
use nrz_bindings::readings::algebra::{
    Cas, Handle, Reference,
    limits::Budget,
    request::{Request, Task},
    source::SyntaxRole,
    syntax::Draft,
};

/// All original endpoints come from an independently reconstructed positive LC request.
pub(super) fn copy(
    fixture: &mut Fixture,
    theorem: &Theorem,
    budget: &mut Budget,
) -> TestResult<Handle> {
    let frame = super::family::build(fixture, [1, 1], budget)?;
    let original = super::request::build(fixture, &frame, [1, 1], false, budget)?;
    let pairs = anchors(
        &theorem.fixture.cas,
        &theorem.request,
        &fixture.cas,
        &original,
        budget,
    )?;
    let mut importer = Importer {
        source: &theorem.fixture.cas,
        receiver: &mut fixture.cas,
        pairs,
        budget,
    };
    importer.copy(&theorem.set)
}

fn anchors(
    source: &Cas,
    request: &Request,
    receiver: &Cas,
    original: &Request,
    budget: &mut Budget,
) -> TestResult<Vec<(Handle, Handle)>> {
    let left = mutations::problem_data(source, request)?;
    let right = mutations::problem_data(receiver, original)?;
    let (Task::Solve(first), Task::Solve(second)) = (&request.task, &original.task) else {
        return Err("LC originals".into());
    };
    let mut pairs = vec![
        (first.problem.clone(), second.problem.clone()),
        (left.header, right.header),
        (left.coefficient_domain, right.coefficient_domain),
        (left.coefficients, right.coefficients),
        (left.unknowns, right.unknowns),
        (left.region.clone(), right.region.clone()),
        (left.domain, right.domain),
    ];
    let first = source.describe_syntax(&left.region, budget)?;
    let second = receiver.describe_syntax(&right.region, budget)?;
    for role in [SyntaxRole::Family, SyntaxRole::BaseStratum] {
        let first = first
            .dependencies
            .iter()
            .find(|entry| entry.role == role)
            .ok_or("LC source placement")?;
        let second = second
            .dependencies
            .iter()
            .find(|entry| entry.role == role)
            .ok_or("LC receiver placement")?;
        pairs.push((first.target.clone(), second.target.clone()));
    }
    Ok(pairs)
}

struct Importer<'a> {
    source: &'a Cas,
    receiver: &'a mut Cas,
    pairs: Vec<(Handle, Handle)>,
    budget: &'a mut Budget,
}

impl Importer<'_> {
    fn copy(&mut self, source: &Handle) -> TestResult<Handle> {
        if let Some((_, target)) = self.pairs.iter().find(|(value, _)| value == source) {
            return Ok(target.clone());
        }
        let graph = self.source;
        let draft = if let Ok(view) = graph.solve_view(source) {
            Draft::Solve(self.solving(view)?)
        } else {
            self.expression(source)?
        };
        let target = admit(self.receiver, draft, self.budget)?;
        self.pairs.push((source.clone(), target.clone()));
        Ok(target)
    }

    fn reference(&mut self, source: &Handle) -> TestResult<Reference> {
        Ok(reference(&self.copy(source)?))
    }

    fn references(&mut self, values: &[Handle]) -> TestResult<Vec<Reference>> {
        values.iter().map(|value| self.reference(value)).collect()
    }
}
