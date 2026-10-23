// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded use of a family sign hypothesis is discharged only by exact covering evidence.

use crate::domain::{AssumptionDraft, ContextDraft, HypothesisRef, RegionDraft, RegionExpr};
use crate::limits::{Budget, Limits};
use crate::proof::{
    ApplicationDraft, Certificate, ConstructorLaw, HypothesisKind, Primitive, Statement,
};
use crate::solving::{
    CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep, SolveCertificate,
    SolveSchema,
};
use crate::syntax::{Draft, Field, Operator, PredicateKind};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};
use crate::{Handle, Rational};

/// Keep construction, extraction and replay on independent existing family ledgers.
fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::blind_cubic())?)
}

/// Preserve the exact mixed family, original base stratum and parameter declaration.
fn guard(fixture: &mut Fixture, base: &Handle, subject: &Handle) -> TestResult<(Handle, Handle)> {
    let original = fixture.cas.state.region(base.logical_id())?.clone();
    let over = original.lies_over.ok_or("original family")?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: original.scope.clone(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(subject)],
        },
    )?;
    let family = fixture.cas.handle(over.family);
    let stratum = fixture.cas.handle(over.stratum);
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: original.scope,
                ambient: original.ambient,
                expression: RegionExpr::GuardTrue {
                    base: reference(base),
                    predicate: reference(&predicate),
                },
            },
            lies_over: crate::solving::LiesOver {
                family: reference(&family),
                stratum: reference(&stratum),
            },
        },
    )?;
    Ok((region, predicate))
}

/// Exact open bounds on the existing family's unspecialized Real parameter.
fn interval(fixture: &mut Fixture, base: &Handle, lower: i64, upper: i64) -> TestResult<Handle> {
    let low = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(Rational::from_ratio(lower, 1, &mut ledger()?)?),
        Vec::new(),
        Vec::new(),
    )?;
    let high = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(Rational::from_ratio(upper, 1, &mut ledger()?)?),
        Vec::new(),
        Vec::new(),
    )?;
    let above = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Sub,
        Vec::new(),
        vec![reference(&fixture.real), reference(&low)],
    )?;
    let below = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Sub,
        Vec::new(),
        vec![reference(&high), reference(&fixture.real)],
    )?;
    let (first, _) = guard(fixture, base, &above)?;
    Ok(guard(fixture, &first, &below)?.0)
}

/// A closed proposal retains every explicit premise edge and the exact context.
fn application(
    context: &Handle,
    rule: Primitive,
    conclusion: Statement,
    certificate: Certificate,
    premises: Vec<u32>,
) -> ApplicationDraft {
    ApplicationDraft {
        rule,
        context: context.clone(),
        conclusion,
        premises,
        substitutions: Vec::new(),
        certificate,
    }
}

/// Prove the covering sign from its original true guard, with no context assumption.
fn covering(
    context: &Handle,
    region: &Handle,
    predicate: &Handle,
    subject: &Handle,
) -> ApplicationDraft {
    application(
        context,
        Primitive::Solve(SolveSchema::RealSign),
        Statement::PredicateFact {
            region: region.clone(),
            predicate: predicate.clone(),
        },
        Certificate::Solve(Box::new(SolveCertificate::RealSign(CertificateRealSign {
            subject: subject.clone(),
            region: region.clone(),
            root: 0,
            steps: vec![RealSignStep {
                rule: RealSignRule::Hypothesis,
                subject: subject.clone(),
                conclusion: RealSignConclusion::Positive,
                arguments: Vec::new(),
                premises: Vec::new(),
                exponent: 0,
            }],
        }))),
        Vec::new(),
    )
}

/// Build a bounded local use inside the original family's checked positive covering region.
fn setup() -> TestResult<(Fixture, Handle, Handle, Vec<ApplicationDraft>)> {
    let (mut fixture, request) = super::request(2)?;
    let ambient = fixture.cas.admissible_region(&request.context)?;
    let subject = fixture.real.clone();
    let (positive, predicate) = guard(&mut fixture, &ambient, &subject)?;
    let cover = interval(&mut fixture, &positive, 0, 3)?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&cover),
            assumptions: Vec::new(),
        }),
    )?;
    let bounded = interval(&mut fixture, &ambient, 1, 2)?;
    let local = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: reference(&context),
            region: reference(&bounded),
            hypotheses: vec![AssumptionDraft {
                key: "positive-on-use".into(),
                condition: reference(&predicate),
            }],
        }),
    )?;
    let domain = fixture.cas.admissible_region(&local)?;
    let proposals = discharge_steps(&context, &local, &cover, &domain, &predicate, &subject);
    Ok((fixture, context, ambient, proposals))
}

/// An independent restriction and close supplies the exact parent-frame discharge theorem.
fn discharge_steps(
    context: &Handle,
    local: &Handle,
    cover: &Handle,
    domain: &Handle,
    predicate: &Handle,
    subject: &Handle,
) -> Vec<ApplicationDraft> {
    let fact = Statement::PredicateFact {
        region: domain.clone(),
        predicate: predicate.clone(),
    };
    vec![
        covering(context, cover, predicate, subject),
        application(
            local,
            Primitive::ContextHypothesis,
            fact.clone(),
            Certificate::Hypothesis(HypothesisRef {
                context: local.clone(),
                slot: 1,
            }),
            Vec::new(),
        ),
        application(
            context,
            Primitive::RegionConstruct,
            Statement::Constructor {
                region: domain.clone(),
                law: ConstructorLaw::Membership,
            },
            Certificate::None,
            Vec::new(),
        ),
        application(
            local,
            Primitive::RegionConstruct,
            Statement::Subset {
                subset: domain.clone(),
                superset: cover.clone(),
            },
            Certificate::None,
            Vec::new(),
        ),
        application(
            local,
            Primitive::ContextWeaken,
            fact.clone(),
            Certificate::None,
            vec![0, 3],
        ),
        application(
            context,
            Primitive::ContextClose,
            fact.clone(),
            Certificate::Close {
                frame: local.clone(),
            },
            vec![4, 2],
        ),
        application(
            context,
            Primitive::ContextClose,
            fact,
            Certificate::Close {
                frame: local.clone(),
            },
            vec![1, 2, 5],
        ),
    ]
}

/// The covering lemma discharges the bounded use, and fresh replay preserves the exact region.
/// ´test:crate:family-regional-hypothesis-discharge´
#[test]
fn family_regional_hypothesis_discharge() -> TestResult {
    let (mut fixture, context, _, proposals) = setup()?;
    let witness = fixture
        .cas
        .check_proof(&context, &proposals, &mut ledger()?)?;
    assert!(witness.is_unconditional());
    let citations = fixture
        .cas
        .proof_citations(witness.proof(), &mut ledger()?)?;
    assert!(citations.iter().any(|row| {
        row.hypotheses
            .iter()
            .any(|used| used.kind == HypothesisKind::StrictSign)
    }));
    let root = citations.last().ok_or("missing discharged conclusion")?;
    assert_eq!(root.formal_schema().lemma.name, "ctx.close");
    assert_eq!(
        root.hypotheses,
        [] as [crate::proof::QualifiedHypothesis; 0]
    );
    assert_eq!(root.residuals, [] as [HypothesisKind; 0]);
    let bytes = fixture.cas.export_witness(&witness, &mut ledger()?)?;
    let mut fresh = crate::Cas::new(Limits::blind_cubic())?;
    let checked = fresh.inspect(&bytes, &mut ledger()?)?;
    assert!(checked.is_unconditional());
    assert_eq!(fresh.export_witness(&checked, &mut ledger()?)?, bytes);
    Ok(())
}

/// Missing discharge retains a residual, while a false containment or a widened claim refuses.
/// ´test:crate:family-regional-discharge-retains-boundary´
#[test]
fn family_regional_discharge_retains_boundary() -> TestResult {
    let (mut fixture, context, ambient, proposals) = setup()?;
    let witness = fixture
        .cas
        .check_proof(&context, &proposals, &mut ledger()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut ledger()?)?;
    let mut widened = witness.judgment().clone();
    let Statement::PredicateFact { region, .. } = &mut widened.statement else {
        return Err("predicate fact".into());
    };
    region.clone_from(&ambient);
    assert!(
        fixture
            .cas
            .replay(&bytes, &widened, &mut ledger()?)
            .is_err()
    );
    let mut missing = vec![
        proposals.get(1).ok_or("assumption")?.clone(),
        proposals.get(2).ok_or("constructor")?.clone(),
        proposals.last().ok_or("close")?.clone(),
    ];
    missing.last_mut().ok_or("close")?.premises = vec![0, 1];
    let conditional = fixture
        .cas
        .check_proof(&context, &missing, &mut ledger()?)?;
    assert!(!conditional.is_unconditional());
    assert_eq!(conditional.judgment().residuals.len(), 1);
    let mut wrong = proposals;
    let Statement::Subset { subset, .. } = &mut wrong.get_mut(3).ok_or("containment")?.conclusion
    else {
        return Err("subset".into());
    };
    *subset = ambient;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(&context, &wrong, &mut ledger()?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
