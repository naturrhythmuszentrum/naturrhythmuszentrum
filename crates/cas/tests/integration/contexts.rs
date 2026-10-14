// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped theorem export and replay with conditional parent evidence.

use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef, Layer};
use nrz_cas::proof::{ApplicationDraft, Certificate, ConstructorLaw, GuardedObligation};
use nrz_cas::proof::{Primitive, Statement};
use nrz_cas::syntax::{Draft, PredicateKind};
use nrz_cas::{Cas, Reference};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::proofs::Fixture;

/// Parent evidence keeps its own residual through closure and a fresh-store proof lifecycle.
/// ´test:integration:conditional-parent-evidence-replay´
#[test]
fn conditional_parent_evidence_replay() -> TestResult {
    let mut fixture = Fixture::new()?;
    let header = fixture.cas.context_header(&fixture.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(fixture.x.clone())],
        },
        &mut budget()?,
    )?;
    let frame = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(fixture.domain.clone()),
            hypotheses: vec![AssumptionDraft {
                key: "local-x".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
        &mut budget()?,
    )?;
    let region = fixture.cas.admissible_region(&frame)?;
    let fact = Statement::PredicateFact {
        region: region.clone(),
        predicate: predicate.clone(),
    };
    let child = ApplicationDraft {
        context: frame.clone(),
        rule: Primitive::ContextHypothesis,
        conclusion: fact.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: frame.clone(),
            slot: 1,
        }),
    };
    let construction = ApplicationDraft {
        context: fixture.context.clone(),
        rule: Primitive::RegionConstruct,
        conclusion: Statement::Constructor {
            region: region.clone(),
            law: ConstructorLaw::Membership,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let first_close = ApplicationDraft {
        context: fixture.context.clone(),
        rule: Primitive::ContextClose,
        conclusion: fact,
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: Certificate::Close { frame },
    };
    let mut second_close = first_close.clone();
    second_close.premises = vec![0, 1, 2];
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[child, construction, first_close, second_close],
        &mut budget()?,
    )?;
    assert_eq!(
        witness.judgment().residuals,
        vec![GuardedObligation {
            home: fixture.context.clone(),
            scope: Vec::new(),
            layer: Layer::Logic,
            space: None,
            region,
            predicate,
        }]
    );
    assert_eq!(witness.hypotheses(), []);
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let imported = receiver.inspect(&bytes, &mut budget()?)?;
    assert!(!imported.is_discharged());
    assert_eq!(receiver.export_witness(&imported, &mut budget()?)?, bytes);
    let mut stripped = imported.judgment().clone();
    stripped.residuals.clear();
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    assert!(receiver.replay(&bytes, &stripped, &mut budget()?).is_err());
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}
