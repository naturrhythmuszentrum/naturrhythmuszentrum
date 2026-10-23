// SPDX-License-Identifier: AGPL-3.0-only
//! A strict-sign contradiction proves the actual meet of a local family context empty.

use super::{empty_witness, ledger, setup};
use crate::Handle;
use crate::domain::{ContextDraft, RegionDraft, RegionExpr};
use crate::proof::{
    ApplicationDraft, Certificate, EmptinessDag, EmptyEvidence, EmptyFact, EmptyLeaf, EmptyNode,
    Statement,
};
use crate::syntax::{Draft, Field, Operator, PredicateKind};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};

/// Keep every total-family and base-stratum identity while adding one exact true guard.
fn guarded(
    fixture: &mut Fixture,
    base: &Handle,
    kind: PredicateKind,
    arguments: &[Handle],
) -> TestResult<Handle> {
    let original = fixture.cas.state.region(base.logical_id())?.clone();
    let over = original.lies_over.ok_or("family metadata")?;
    let family = fixture.cas.handle(over.family);
    let stratum = fixture.cas.handle(over.stratum);
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind,
            arguments: arguments.iter().map(reference).collect(),
        },
    )?;
    admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
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
    )
}

/// A local context's exact meet, rather than its restriction alone, is the theorem region.
fn local_steps(fixture: &mut Fixture, parent: &Handle) -> TestResult<Vec<ApplicationDraft>> {
    let base = fixture.cas.admissible_region(parent)?;
    let subject = fixture.real.clone();
    let positive = guarded(
        fixture,
        &base,
        PredicateKind::Positive,
        std::slice::from_ref(&subject),
    )?;
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(crate::number::Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    let restriction = guarded(
        fixture,
        &positive,
        PredicateKind::Equal,
        &[subject.clone(), zero],
    )?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: reference(parent),
            region: reference(&restriction),
            hypotheses: Vec::new(),
        }),
    )?;
    let region = fixture.cas.admissible_region(&context)?;
    assert_ne!(region, restriction);
    Ok(vec![ApplicationDraft {
        rule: crate::rule::Primitive::RegionDecide,
        context,
        conclusion: Statement::Empty {
            region: region.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Emptiness(Box::new(EmptinessDag {
            nodes: vec![EmptyNode {
                region,
                evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::Sign {
                    subject,
                    nonzero: EmptyFact::Guard(positive),
                })),
            }],
        })),
    }])
}

/// A sign leaf on an actual local meet enters the ordinary family-piece inventory and fresh replay.
/// ´test:crate:family-emptiness-local-sign-piece´
#[test]
fn family_emptiness_local_sign_piece() -> TestResult {
    let (mut fixture, problem, parent, _) = setup()?;
    let steps = local_steps(&mut fixture, &parent)?;
    let witness = empty_witness(&mut fixture, &steps)?;
    let piece =
        fixture
            .cas
            .family_empty_piece(&problem, &witness, &mut ledger()?, &mut ledger()?)?;
    let bytes = fixture
        .cas
        .export_witness(&piece.coverage, &mut ledger()?)?;
    assert!(
        crate::Cas::new(crate::limits::Limits::blind_cubic())?
            .inspect(&bytes, &mut ledger()?)?
            .is_discharged()
    );
    let request = crate::solving::FamilyCoverRequest {
        context: parent,
        problem,
        base_cover: None,
        groups: vec![crate::solving::FamilyCoverGroup {
            context: witness.judgment().context.clone(),
            pieces: vec![piece.coverage],
            cover: None,
        }],
    };
    let result = fixture
        .cas
        .family_cover(&request, &mut ledger()?, &mut ledger()?)?;
    assert_eq!(
        result.pending,
        Some(crate::solving::SolveReason::BaseCoveragePending)
    );
    assert_eq!(result.pieces.len(), 1);
    assert_eq!(
        result.pieces.first().ok_or("retained piece")?.descriptor,
        piece.descriptor
    );
    Ok(())
}
