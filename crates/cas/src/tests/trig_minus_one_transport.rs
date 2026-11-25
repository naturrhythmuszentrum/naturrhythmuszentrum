// SPDX-License-Identifier: AGPL-3.0-only
//! The original nonzero operand remains visible inside the scalar chain and its close.

use crate::conditions::{GuardKind, Obligation};
use crate::domain_model::{ContextDraft, HypothesisRef};
use crate::graph::{AssumptionDraft, Draft, Reference};
use crate::model::{Operator, PredicateKind};
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, DomainContract, Statement};
use crate::rule::Primitive;
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, admit, predicate, term};

/// Build the original conditional equality and its guard independently of conversion.
fn prepared() -> TestResult<(
    Fixture,
    crate::proof_model::VerifiedWitness,
    crate::graph::Handle,
)> {
    let mut f = Fixture::new()?;
    let minus = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::from_ratio(
            -1,
            1,
            &mut crate::testing::native::fresh_budget()?,
        )?),
        &[],
    )?;
    let quotient = term(
        &mut f.cas,
        &f.header,
        Operator::Div,
        &[f.zero.clone(), f.x.clone()],
    )?;
    let original = term(
        &mut f.cas,
        &f.header,
        Operator::Add,
        &[minus.clone(), quotient],
    )?;
    let function = term(
        &mut f.cas,
        &f.header,
        Operator::Arctangent,
        std::slice::from_ref(&original),
    )?;
    let defined = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::Defined,
        std::slice::from_ref(&original),
    )?;
    let nonzero = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::NonZero,
        &[f.x.clone()],
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.domain.clone()),
            assumptions: vec![AssumptionDraft {
                key: "original-defined".into(),
                condition: Reference::Existing(defined.clone()),
            }],
        }),
    )?;
    let guard = ApplicationDraft {
        context: f.context.clone(),
        rule: Primitive::ContextHypothesis,
        conclusion: Statement::PredicateFact {
            region: f.domain.clone(),
            predicate: defined,
        },
        premises: vec![],
        substitutions: vec![],
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: f.context.clone(),
            slot: 0,
        }),
    };
    let equality = ApplicationDraft {
        context: f.context.clone(),
        rule: Primitive::RationalIdentity,
        conclusion: Statement::Equality {
            left: original,
            right: minus,
            domain: f.domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: vec![],
        substitutions: vec![],
        certificate: Certificate::None,
    };
    let guard = f.cas.check_proof(
        &f.context,
        &[guard],
        &mut crate::testing::native::fresh_budget()?,
    )?;
    let equality = f.cas.check_proof(
        &f.context,
        &[equality],
        &mut crate::testing::native::fresh_budget()?,
    )?;
    let crate::trig::TrigRecipeOutcome::Proved(witness) = f.cas.trig_atan_minus_one(
        &function,
        guard.proof(),
        equality.proof(),
        &mut crate::testing::native::fresh_budget()?,
    )?
    else {
        return Err("conditional recipe".into());
    };
    Ok((f, *witness, nonzero))
}

/// A conditional checked equality retains NonZero(x) in scalar conditions, guarded claims and closure.
/// ´test:crate:trig-minus-one-transport-residual-identity´
#[test]
fn trig_minus_one_transport_residual_identity() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let (f, witness, nonzero) = prepared()?;
    assert_ne!(witness.judgment().residuals, []);
    assert!(
        witness
            .judgment()
            .residuals
            .iter()
            .all(|guarded| guarded.predicate == nonzero)
    );
    let citations = f.cas.proof_citations(
        witness.proof(),
        &mut crate::testing::native::fresh_budget()?,
    )?;
    let close = citations
        .iter()
        .find(|node| {
            node.primitive == Primitive::ContextClose
                && node.premises.first().is_some_and(|first| {
                    citations.iter().any(|child| {
                        child.proof == first.proof && child.primitive == Primitive::Transitivity
                    })
                })
        })
        .ok_or("scalar close")?;
    let child = crate::proof_view::application(
        &f.cas,
        f.cas.checked(&close.premises[0].proof, &mut budget)?,
        &mut crate::testing::native::fresh_budget()?,
    )?;
    let closed = crate::proof_view::application(
        &f.cas,
        f.cas.checked(&close.proof, &mut budget)?,
        &mut crate::testing::native::fresh_budget()?,
    )?;
    for step in [&child, &closed] {
        assert_ne!(step.judgment.residuals, []);
        assert!(
            step.judgment
                .residuals
                .iter()
                .all(|guarded| guarded.predicate == nonzero)
        );
        assert!(
            step.conditions
                .residuals
                .iter()
                .all(|guarded| guarded.predicate == nonzero)
        );
    }
    let scalar = crate::proof_view::scalar_application(
        &f.cas,
        &child,
        &mut crate::testing::native::fresh_budget()?,
    )?;
    assert_eq!(
        scalar.conditions.residuals,
        std::collections::BTreeSet::from([Obligation {
            kind: GuardKind::NonZero,
            term: f.cas.checked(&f.x, &mut budget)?
        }])
    );
    Ok(())
}
