// SPDX-License-Identifier: AGPL-3.0-only
//! Nontrigonometric endpoint hypotheses and independently constructed closure claims.

use super::budgets::proof_budget as budget;
use super::builders::admit;
use super::cases::Fixture;
use super::common::TestResult;
use super::witness::equality;
use crate::test_api::domain::{AssumptionDraft, ContextDraft, Layer};
use crate::test_api::proof::{
    ApplicationDraft, Certificate, ConstructorLaw, DomainContract, Judgment, Primitive,
    RegionCertificate, Statement,
};
use crate::test_api::syntax::{Draft, Operator, PredicateKind};
use crate::test_api::{Handle, Rational, Reference};

/// Original syntax, exact introducing slot and a complete close with independently stated claim.
pub(crate) struct Chain {
    pub(crate) fixture: Fixture,
    pub(crate) frame: Handle,
    pub(crate) predicate: Handle,
    pub(crate) steps: Vec<ApplicationDraft>,
    pub(crate) expected: Judgment,
}

/// The guarded axis proves nonzero independently of the child's local assumption.
pub(crate) fn chain(reordered: bool, consume: bool) -> TestResult<Chain> {
    let mut fixture = Fixture::new()?;
    if reordered {
        fixture.number("37")?;
    }
    let predicate = fixture.predicate(PredicateKind::NonZero, &[fixture.x.clone()])?;
    let restriction = fixture.region(&predicate, true)?;
    let frame = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(restriction),
            hypotheses: vec![AssumptionDraft {
                key: "endpoint-x".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
        &mut budget()?,
    )?;
    let region = fixture
        .cas
        .admissible_region(&frame, &mut crate::testing::budgets::graph_budget()?)?;
    let quotient = fixture.term(Operator::Div, &[fixture.x.clone(), fixture.x.clone()])?;
    let conclusion = Statement::Equality {
        left: quotient.clone(),
        right: fixture.one.clone(),
        domain: region.clone(),
        domain_contract: DomainContract::DefinedAndEqual,
    };
    let child = equality(&frame, &region, Primitive::Cancel, &quotient, &fixture.one);
    let construction = ApplicationDraft {
        context: fixture.context.clone(),
        rule: Primitive::RegionConstruct,
        conclusion: Statement::Constructor {
            region: region.clone(),
            law: ConstructorLaw::Membership,
        },
        premises: vec![],
        substitutions: vec![],
        certificate: Certificate::None,
    };
    let fact = ApplicationDraft {
        context: fixture.context.clone(),
        rule: Primitive::RegionDecide,
        conclusion: Statement::PredicateFact {
            region: region.clone(),
            predicate: predicate.clone(),
        },
        premises: vec![],
        substitutions: vec![],
        certificate: Certificate::Region(RegionCertificate {
            cuts: vec![Rational::zero()],
            membership: vec![vec![true, false, true], vec![true, false, true]],
        }),
    };
    let close = ApplicationDraft {
        context: fixture.context.clone(),
        rule: Primitive::ContextClose,
        conclusion: conclusion.clone(),
        premises: if consume { vec![0, 1, 2] } else { vec![0, 1] },
        substitutions: vec![],
        certificate: Certificate::Close {
            frame: frame.clone(),
        },
    };
    let mut steps = vec![child, construction];
    if consume {
        steps.push(fact);
    }
    steps.push(close);
    let residuals = if consume {
        vec![]
    } else {
        vec![crate::test_api::proof::GuardedObligation {
            home: fixture.context.clone(),
            scope: vec![],
            layer: Layer::Pointwise,
            space: None,
            region,
            predicate: predicate.clone(),
        }]
    };
    let expected = Judgment {
        context: fixture.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: conclusion,
        residuals,
    };
    Ok(Chain {
        fixture,
        frame,
        predicate,
        steps,
        expected,
    })
}
