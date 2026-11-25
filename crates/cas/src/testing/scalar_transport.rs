// SPDX-License-Identifier: AGPL-3.0-only
//! Ordinary restriction chains retain a nontrigonometric nonzero residual.

use super::budgets::proof_budget as budget;
use super::builders::admit;
use super::cases::Fixture;
use super::common::TestResult;
use super::witness::equality;
use crate::test_api::domain::{AmbientDomain, ContextDraft, Layer, RegionDraft, RegionExpr};
use crate::test_api::proof::{
    ApplicationDraft, Certificate, DomainContract, GuardedObligation, Judgment, Primitive,
    Statement,
};
use crate::test_api::syntax::{Draft, Operator, PredicateKind};
use crate::test_api::{Handle, Reference};

pub(crate) struct Chain {
    pub(crate) fixture: Fixture,
    pub(crate) frames: Vec<Handle>,
    pub(crate) restriction: Handle,
    pub(crate) predicate: Handle,
    pub(crate) steps: Vec<ApplicationDraft>,
    pub(crate) expected: Judgment,
}

pub(crate) fn meet(f: &mut Fixture, members: &[Handle]) -> TestResult<Handle> {
    admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: vec![],
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Meet(
                members.iter().cloned().map(Reference::Existing).collect(),
            ),
        }),
        &mut budget()?,
    )
}

pub(crate) fn chain(reordered: bool, depth: usize) -> TestResult<Chain> {
    let mut f = Fixture::new()?;
    if reordered {
        f.number("37")?;
    }
    let predicate = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let restriction = f.region(&predicate, true)?;
    let quotient = f.term(Operator::Div, &[f.x.clone(), f.x.clone()])?;
    let mut frames = vec![f.context.clone()];
    let mut steps = vec![equality(
        &f.context,
        &f.domain,
        Primitive::Cancel,
        &quotient,
        &f.one,
    )];
    let mut previous_domain = f.domain.clone();
    let mut guarded_domains = vec![f.domain.clone()];
    for _ in 0..depth {
        let parent = frames.last().ok_or("missing parent")?.clone();
        let frame = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(parent),
                region: Reference::Existing(restriction.clone()),
                hypotheses: vec![],
            }),
            &mut budget()?,
        )?;
        let domain = f
            .cas
            .admissible_region(&frame, &mut crate::testing::budgets::graph_budget()?)?;
        for guarded in &mut guarded_domains {
            *guarded = meet(&mut f, &[guarded.clone(), domain.clone()])?;
        }
        let source = u32::try_from(steps.len() - 1)?;
        steps.push(ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::RegionConstruct,
            conclusion: Statement::Subset {
                subset: domain.clone(),
                superset: previous_domain,
            },
            premises: vec![],
            substitutions: vec![],
            certificate: Certificate::None,
        });
        steps.push(ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::ContextWeaken,
            conclusion: Statement::Equality {
                left: quotient.clone(),
                right: f.one.clone(),
                domain: domain.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: vec![source, source + 1],
            substitutions: vec![],
            certificate: Certificate::None,
        });
        previous_domain = domain;
        frames.push(frame);
    }
    let frame = frames.last().ok_or("missing frame")?.clone();
    let mut consumer = equality(
        &frame,
        &previous_domain,
        Primitive::Symmetry,
        &f.one,
        &quotient,
    );
    consumer.premises = vec![u32::try_from(steps.len() - 1)?];
    guarded_domains.push(previous_domain);
    guarded_domains.sort_by_key(Handle::logical_id);
    guarded_domains.dedup();
    let expected = Judgment {
        context: frame.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: consumer.conclusion.clone(),
        residuals: guarded_domains
            .into_iter()
            .map(|region| GuardedObligation {
                home: frame.clone(),
                scope: vec![],
                layer: Layer::Pointwise,
                space: None,
                region,
                predicate: predicate.clone(),
            })
            .collect(),
    };
    steps.push(consumer);
    Ok(Chain {
        fixture: f,
        frames,
        restriction,
        predicate,
        steps,
        expected,
    })
}
