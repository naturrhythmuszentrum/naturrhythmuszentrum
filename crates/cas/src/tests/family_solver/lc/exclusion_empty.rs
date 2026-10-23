// SPDX-License-Identifier: AGPL-3.0-only
//! Imported empty zero sets retain their exact base and independently checked sign subject.

use crate::domain_model::{ContextDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, Handle, Reference};
use crate::model::PredicateKind;
use crate::proof::{
    ApplicationDraft, Certificate, EmptinessDag, EmptyEvidence, EmptyFact, EmptyLeaf, EmptyNode,
    Statement,
};
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, admit, budget, predicate};

fn guarded(fixture: &mut Fixture, base: &Handle, condition: Handle) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: crate::domain_model::AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(base.clone()),
                predicate: Reference::Existing(condition),
            },
        }),
    )
}

fn setup() -> TestResult<(Fixture, Handle, Handle, ApplicationDraft)> {
    let mut fixture = Fixture::new()?;
    let positive = predicate(
        &mut fixture.cas,
        &fixture.header,
        PredicateKind::Positive,
        std::slice::from_ref(&fixture.x),
    )?;
    let ambient = fixture.domain.clone();
    let base = guarded(&mut fixture, &ambient, positive)?;
    let equal = predicate(
        &mut fixture.cas,
        &fixture.header,
        PredicateKind::Equal,
        &[fixture.x.clone(), fixture.zero.clone()],
    )?;
    let zero_set = guarded(&mut fixture, &base, equal)?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(base.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    let step = ApplicationDraft {
        rule: crate::rule::Primitive::RegionDecide,
        context,
        conclusion: Statement::Empty {
            region: zero_set.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Emptiness(Box::new(EmptinessDag {
            nodes: vec![EmptyNode {
                region: zero_set,
                evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::Sign {
                    subject: fixture.x.clone(),
                    nonzero: EmptyFact::Guard(base.clone()),
                })),
            }],
        })),
    };
    Ok((fixture, base, ambient, step))
}

/// A sign leaf proves only its exact zero set, and importing that theorem cannot widen its base.
/// ´test:crate:empty-zero-set-keeps-original-base´
#[test]
fn empty_zero_set_keeps_original_base() -> TestResult {
    let (mut fixture, base, ambient, mut step) = setup()?;
    let context = step.context.clone();
    let witness = fixture
        .cas
        .check_proof(&context, std::slice::from_ref(&step), &mut budget()?)?;
    assert_eq!(
        crate::solving::elementary::exclusion::subject(
            &fixture.cas,
            &base,
            &witness.judgment().statement
        )?,
        Some(fixture.x.logical_id())
    );
    assert_eq!(
        crate::solving::elementary::exclusion::subject(
            &fixture.cas,
            &ambient,
            &witness.judgment().statement
        )?,
        None
    );
    let Certificate::Emptiness(dag) = &mut step.certificate else {
        return Err("empty certificate".into());
    };
    let EmptyEvidence::Leaf(leaf) = &mut dag.nodes.first_mut().ok_or("sign node")?.evidence else {
        return Err("sign leaf".into());
    };
    let EmptyLeaf::Sign { subject, .. } = leaf.as_mut() else {
        return Err("sign leaf".into());
    };
    subject.clone_from(&fixture.one);
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(&context, &[step], &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
