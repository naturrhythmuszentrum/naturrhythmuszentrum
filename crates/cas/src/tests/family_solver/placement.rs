// SPDX-License-Identifier: AGPL-3.0-only
//! Original placement associations and missing native arms remain explicit.

use crate::budget::{Budget, Limits};
use crate::domain_model::{CaseArmDraft, ContextDraft};
use crate::graph::Draft;
use crate::model::{Field, Operator, PredicateKind};
use crate::request_model::{CoveredInput, Outcome, Request};
use crate::solving::{PlacementUse, SolveDraft, SpacePlacementData};
use crate::task::Task;
use crate::testing::common::TestResult;
use crate::testing::sorts::{Family, Fixture, admit, expression, reference};

/// Construct one original association with a separately retained canonical piece.
fn arm(fixture: &mut Fixture, family: &Family) -> TestResult<CaseArmDraft> {
    let body = fixture.body(family)?;
    let piece = fixture.piece(&family.region)?;
    Ok(CaseArmDraft {
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&family.region),
        value: reference(&body),
        placements: vec![PlacementUse {
            operand: reference(&fixture.real),
            piece: reference(&piece),
        }],
    })
}

/// A same-sort expression cannot replace the original placement operand.
/// ´test:crate:mixed-placement-rejects-wrong-operand´
#[test]
fn mixed_placement_rejects_wrong_operand() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let mut original = arm(&mut fixture, &family)?;
    admit(&mut fixture.cas, Draft::CaseArm(original.clone()))?;
    let replacement = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Neg,
        Vec::new(),
        vec![reference(&fixture.real)],
    )?;
    let pair = original.placements.first_mut().ok_or("missing placement")?;
    assert_eq!(pair.operand, reference(&fixture.real));
    assert_ne!(pair.operand, reference(&replacement));
    pair.operand = reference(&replacement);
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::CaseArm(original)],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// A different piece and a changed directed edge are each rejected as pinned field mutations.
/// ´test:crate:mixed-placement-rejects-changed-piece´
#[test]
fn mixed_placement_rejects_changed_piece() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let mut original = arm(&mut fixture, &family)?;
    admit(&mut fixture.cas, Draft::CaseArm(original.clone()))?;
    let expected = fixture.piece(&family.region)?;
    let replacement = fixture.piece(&family.base_region)?;
    let pair = original.placements.first_mut().ok_or("missing placement")?;
    assert_eq!(pair.piece, reference(&expected));
    assert_ne!(pair.piece, reference(&replacement));
    pair.piece = reference(&replacement);
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::CaseArm(original)],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    let mut edge = SpacePlacementData {
        source: reference(&fixture.real_space),
        ambient: reference(&fixture.complex_space),
        specification: "placement.real-in-complex".into(),
        version: 1,
    };
    admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SpacePlacement(edge.clone())),
    )?;
    assert_eq!(edge.source, reference(&fixture.real_space));
    assert_ne!(edge.source, reference(&fixture.complex_space));
    edge.source = reference(&fixture.complex_space);
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::SpacePlacement(edge))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Build an actual positive base stratum and its exact total-space preimage.
fn positive(fixture: &mut Fixture, family: &Family) -> TestResult<Family> {
    use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(&fixture.real)],
        },
    )?;
    let base_region = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: family.parameters.clone(),
            },
            expression: RegionExpr::GuardTrue {
                base: reference(&family.base_region),
                predicate: reference(&predicate),
            },
        }),
    )?;
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: family.total.clone(),
                },
                expression: RegionExpr::GuardTrue {
                    base: reference(&family.region),
                    predicate: reference(&predicate),
                },
            },
            lies_over: crate::solving::LiesOver {
                family: reference(&family.handle),
                stratum: reference(&base_region),
            },
        },
    )?;
    Ok(Family {
        parameters: family.parameters.clone(),
        base_region,
        handle: family.handle.clone(),
        region,
        total: family.total.clone(),
    })
}

/// A native coverage query cannot manufacture an association outside its original positive arm.
/// ´test:crate:mixed-family-missing-arm-stays-undefined´
#[test]
fn mixed_family_missing_arm_stays_undefined() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let local = positive(&mut fixture, &family)?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )?;
    let data = fixture.body_data(&local);
    let cases = fixture
        .cas
        .family_expression(&data, &mut Budget::new(Limits::default())?)?;
    assert_eq!(
        fixture.cas.state.expression(cases.logical_id())?.arms.len(),
        1
    );
    let root =
        fixture
            .cas
            .materialize_derived(&cases, &cases, &mut Budget::new(Limits::default())?)?;
    let receipt = fixture
        .cas
        .walk_derived(&root, &mut Budget::new(Limits::default())?)?
        .receipt;
    let mut request = Request {
        task: Task::Coverage {
            domain: local.region.clone(),
        },
        context,
        inputs: vec![CoveredInput { root, receipt }],
        premises: Vec::new(),
        limits: crate::budget::LimitsOverride::default(),
    };
    assert!(matches!(fixture.cas.run(&request), Outcome::Proved { .. }));
    let Task::Coverage { domain } = &mut request.task else {
        return Err("wrong task".into());
    };
    assert_eq!(*domain, local.region);
    assert_ne!(*domain, family.region);
    *domain = family.region;
    assert!(!matches!(fixture.cas.run(&request), Outcome::Proved { .. }));
    Ok(())
}

/// Changing only the ordered subject from an original Real base to a Complex unknown fails admission.
/// ´test:crate:mixed-family-rejects-complex-order´
#[test]
fn mixed_family_rejects_complex_order() -> TestResult {
    let mut fixture = Fixture::new()?;
    let unknown = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::Free("z".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let mut condition = Draft::Condition {
        header: reference(&fixture.header),
        scope: Vec::new(),
        kind: PredicateKind::Positive,
        arguments: vec![reference(&fixture.real)],
    };
    admit(&mut fixture.cas, condition.clone())?;
    let Draft::Condition { arguments, .. } = &mut condition else {
        return Err("wrong condition".into());
    };
    let subject = arguments.first_mut().ok_or("missing subject")?;
    assert_eq!(*subject, reference(&fixture.real));
    assert_ne!(*subject, reference(&unknown));
    *subject = reference(&unknown);
    assert!(
        fixture
            .cas
            .admit(&[condition], &mut Budget::new(Limits::default())?)
            .is_err()
    );
    Ok(())
}
