// SPDX-License-Identifier: AGPL-3.0-only
//! Crate fixture exposes checked applications solely for adversarial mutation tests.

use super::common::TestResult;
use super::native::{Fixture, budget};
use crate::boundary_derivative::DerivativeJob;
use crate::boundary_model::{CaseName, NamedCase};
use crate::boundary_producer::Runner;
use crate::case_producer::Plan;
use crate::graph::Handle;
use crate::proof_model::{ApplicationDraft, CheckedApplication};
use crate::request_model::Request;
use crate::search::SearchPolicy;
use crate::task::{BoundaryFact, Task};

/// Checked point application and its actual ordered predecessor records.
pub(crate) struct Evidence {
    /// Independent exact syntax roots.
    pub(crate) fixture: Fixture,
    /// Proposed root copied from a valid primitive application.
    pub(crate) draft: ApplicationDraft,
    /// Previously checked premises for that root.
    pub(crate) premises: Vec<CheckedApplication>,
}

/// Construct an absolute-value obstruction without making an expected-value oracle.
pub(crate) fn obstruction() -> TestResult<Evidence> {
    point_evidence(true)
}

/// Construct the derivative of the coordinate at zero for independent value mutation.
pub(crate) fn smooth() -> TestResult<Evidence> {
    point_evidence(false)
}

/// Production supplies a valid premise graph; tests independently select all mutations.
fn point_evidence(obstructed: bool) -> TestResult<Evidence> {
    let mut fixture = Fixture::new()?;
    let function = if obstructed {
        fixture.cas.named_case(
            &fixture.domain,
            &NamedCase {
                name: CaseName::Abs,
                coordinate: "x".into(),
                parameter: None,
            },
            &mut budget()?,
        )?
    } else {
        fixture.x.clone()
    };
    let point = crate::boundary_regions::boundary(
        &mut fixture.cas,
        &fixture.domain,
        &crate::number::Rational::zero(),
        &mut budget()?,
    )?;
    let request = Request {
        task: Task::Boundary {
            coordinate: "x".into(),
            point: point.clone(),
            requested_facts: vec![BoundaryFact::Derivative { order: 1 }],
        },
        context: fixture.context.clone(),
        inputs: Vec::new(),
        premises: Vec::new(),
        limits: crate::budget::LimitsOverride::default(),
    };
    let policy = SearchPolicy::default();
    let mut generation = budget()?;
    let mut checker = budget()?;
    let mut runner = Runner {
        cas: &mut fixture.cas,
        request: &request,
        plan: Plan::new(&policy),
        generation: &mut generation,
        checker: &mut checker,
        facts: Vec::new(),
        deferred: None,
    };
    let job = DerivativeJob {
        function: function.clone(),
        domain: fixture.domain.clone(),
        coordinate: "x".into(),
    };
    let proof = runner.point(&job, &function, &point, 1, None)?;
    extract(fixture, &proof)
}

/// Preserve all actual premise identities before testing a modified proposal.
fn extract(fixture: Fixture, proof: &Handle) -> TestResult<Evidence> {
    let application =
        crate::proof_view::application(&fixture.cas, proof.logical_id(), &mut budget()?)?;
    let mut premises = Vec::new();
    for id in &application.premises {
        premises.push(crate::proof_view::application(
            &fixture.cas,
            *id,
            &mut budget()?,
        )?);
    }
    let draft = ApplicationDraft {
        context: application.judgment.context,
        rule: application.rule,
        conclusion: application.judgment.statement,
        premises: (0..premises.len())
            .map(u32::try_from)
            .collect::<Result<_, _>>()?,
        substitutions: application.substitutions,
        certificate: application.certificate,
    };
    Ok(Evidence {
        fixture,
        draft,
        premises,
    })
}
