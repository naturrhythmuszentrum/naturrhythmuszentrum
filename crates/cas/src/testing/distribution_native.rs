// SPDX-License-Identifier: AGPL-3.0-only
//! Internal distribution routes expose exact proposals only for adversarial checking.

use super::common::TestResult;
use super::native::{self, budget};
use crate::boundary_model::{CaseName, NamedCase};
use crate::domain_model::SpaceDraft;
use crate::graph::{Draft, Handle, Reference};
use crate::model::{Field, Operator};
use crate::proof_model::{ApplicationDraft, CheckedApplication};
use crate::request_model::Request;
use crate::search::{SearchPolicy, SearchResult};
use crate::task::Task;

/// Independent syntax and an exact whole-axis distribution space.
pub(crate) struct Fixture {
    /// Existing real-axis source-free crate fixture.
    pub(crate) base: native::Fixture,
    /// Exact space.
    pub(crate) space: Handle,
}

impl Fixture {
    /// Build syntax without invoking a distribution rule.
    pub(crate) fn new() -> TestResult<Self> {
        let mut base = native::Fixture::new()?;
        let space = native::admit(
            &mut base.cas,
            Draft::Space(SpaceDraft {
                header: Reference::Existing(base.header.clone()),
                domain: Reference::Existing(base.domain.clone()),
                coordinate: "x".into(),
                field: Field::Real,
                test_family: "CompactSmooth".into(),
            }),
        )?;
        Ok(Self { base, space })
    }

    /// Construct only the typed syntax requested by the test.
    pub(crate) fn node(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        Ok(crate::distribution_terms::node(
            &mut self.base.cas,
            &self.base.header,
            &self.space,
            operator,
            operands,
            &mut budget()?,
        )?)
    }

    /// Transparent absolute value with an explicit regular embedding.
    pub(crate) fn absolute(&mut self) -> TestResult<Handle> {
        let function = self.base.cas.named_case(
            &self.base.domain,
            &NamedCase {
                name: CaseName::Abs,
                coordinate: "x".into(),
                parameter: None,
            },
            &mut budget()?,
        )?;
        self.node(Operator::RegularEmbed, &[function])
    }

    /// Independent request envelope for calling the native route directly.
    pub(crate) fn request(&self, task: Task) -> Request {
        Request {
            task,
            context: self.base.context.clone(),
            inputs: Vec::new(),
            premises: Vec::new(),
            limits: crate::budget::LimitsOverride::default(),
        }
    }

    /// Force either scheduled route while retaining the same kernel and independent replay.
    pub(crate) fn route(
        &mut self,
        task: Task,
        inputs: &[Handle],
        named: bool,
    ) -> TestResult<SearchResult> {
        let request = self.request(task);
        let ids: Vec<_> = inputs.iter().map(Handle::logical_id).collect();
        Ok(crate::distribution_search::route(
            &mut self.base.cas,
            &request,
            &ids,
            &SearchPolicy::default(),
            named,
            &[],
            &mut budget()?,
            &mut budget()?,
        )?)
    }
}

/// Copy one actual checked application and all its ordered predecessors.
pub(crate) fn evidence(
    fixture: &Fixture,
    proof: &Handle,
) -> TestResult<(ApplicationDraft, Vec<CheckedApplication>)> {
    let application =
        crate::proof_view::application(&fixture.base.cas, proof.logical_id(), &mut budget()?)?;
    let mut premises = Vec::new();
    for id in application.premises {
        premises.push(crate::proof_view::application(
            &fixture.base.cas,
            id,
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
    Ok((draft, premises))
}
