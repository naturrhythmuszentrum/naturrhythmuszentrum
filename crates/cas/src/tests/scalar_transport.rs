// SPDX-License-Identifier: AGPL-3.0-only
//! Guarded scalar conversion checks complete region authority independently of producers.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, ContextDraft, Layer, RegionDraft, RegionExpr};
use crate::error::{FaultKind, InvalidCode, Location};
use crate::graph::{Draft, Handle, Reference, Vertex};
use crate::model::PredicateKind;
use crate::proof_model::{DomainContract, GuardedObligation, Judgment, Statement};
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, admit, predicate};

struct Input {
    f: Fixture,
    frames: Vec<Handle>,
    restriction: Handle,
    claim: Judgment,
}

fn region(f: &mut Fixture, expression: RegionExpr<Reference>) -> TestResult<Handle> {
    admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: vec![],
            ambient: AmbientDomain::Real("x".into()),
            expression,
        }),
    )
}

fn meet(f: &mut Fixture, members: &[Handle]) -> TestResult<Handle> {
    region(
        f,
        RegionExpr::Meet(members.iter().cloned().map(Reference::Existing).collect()),
    )
}

fn input(depth: usize) -> TestResult<Input> {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = Fixture::new()?;
    let p = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::NonZero,
        &[f.x.clone()],
    )?;
    let base = Reference::Existing(f.domain.clone());
    let restriction = region(
        &mut f,
        RegionExpr::GuardTrue {
            base,
            predicate: Reference::Existing(p.clone()),
        },
    )?;
    let mut frames = vec![f.context.clone()];
    for _ in 0..depth {
        let frame = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(frames.last().ok_or("missing frame")?.clone()),
                region: Reference::Existing(restriction.clone()),
                hypotheses: vec![],
            }),
        )?;
        frames.push(frame);
    }
    let context = frames.last().ok_or("missing frame")?.clone();
    let domain = f.cas.admissible_region(&context, &mut budget)?;
    let claim = Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: f.x.clone(),
            right: f.x.clone(),
            domain: domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: vec![GuardedObligation {
            home: context,
            scope: vec![],
            layer: Layer::Pointwise,
            space: None,
            region: domain,
            predicate: p,
        }],
    };
    Ok(Input {
        f,
        frames,
        restriction,
        claim,
    })
}

fn supported(i: &Input) -> TestResult {
    let result = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )?;
    assert_eq!(result.residuals.len(), 1);
    assert_eq!(result.residuals[0].term, i.f.x);
    Ok(())
}

fn unsupported(i: &Input) -> TestResult {
    let before =
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?;
    let fault = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )
    .expect_err("unsupported region");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::ScalarTransportUnsupported)
    );
    assert_eq!(fault.location.operation, "scalar guarded-region transport");
    assert_eq!(
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?,
        before
    );
    Ok(())
}

/// Canonical member order and nested intersections preserve opaque whole-admissible leaves.
/// ´test:crate:scalar-transport-ancestor-meets´
#[test]
fn scalar_transport_ancestor_meets() -> TestResult {
    for depth in [1, 3] {
        let mut i = input(depth)?;
        let m = i.claim.residuals[0].region.clone();
        let d = i.f.domain.clone();
        let r = meet(&mut i.f, &[m.clone(), d.clone()])?;
        assert_eq!(r, meet(&mut i.f, &[d.clone(), m.clone(), d.clone()])?);
        i.claim.residuals[0].region = r.clone();
        supported(&i)?;
        i.claim.residuals[0].region = meet(&mut i.f, &[r, m, d])?;
        supported(&i)?;
    }
    Ok(())
}

/// An anchor alone cannot authorize sibling, restriction-only or missing members.
/// ´test:crate:scalar-transport-unrecognized-members´
#[test]
fn scalar_transport_unrecognized_members() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    for kind in 0..6 {
        let mut i = input(2)?;
        let m = i.claim.residuals[0].region.clone();
        let d = i.f.domain.clone();
        i.claim.residuals[0].region = match kind {
            0 => meet(&mut i.f, &[d])?,
            1 => meet(&mut i.f, &[])?,
            2 => i.restriction.clone(),
            3 => meet(&mut i.f, &[m, i.restriction.clone()])?,
            4 => {
                let other = region(&mut i.f, RegionExpr::Empty)?;
                meet(&mut i.f, &[m, other])?
            }
            _ => {
                let other = region(
                    &mut i.f,
                    RegionExpr::GuardFalse {
                        base: Reference::Existing(d),
                        predicate: Reference::Existing(i.claim.residuals[0].predicate.clone()),
                    },
                )?;
                let sibling = admit(
                    &mut i.f.cas,
                    Draft::Context(ContextDraft::Local {
                        parent: Reference::Existing(i.f.context.clone()),
                        region: Reference::Existing(other),
                        hypotheses: vec![],
                    }),
                )?;
                let sibling = i.f.cas.admissible_region(&sibling, &mut budget)?;
                meet(&mut i.f, &[m, sibling])?
            }
        };
        unsupported(&i)?;
    }
    Ok(())
}

/// Matching retained members confer no authority after a parent link is severed or retargeted.
/// ´test:crate:scalar-transport-chain-authority´
#[test]
fn scalar_transport_chain_authority() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    for sever in [false, true] {
        let mut i = input(2)?;
        let m = i.claim.residuals[0].region.clone();
        let d = i.f.domain.clone();
        i.claim.residuals[0].region = meet(&mut i.f, &[m, d])?;
        supported(&i)?;
        let id =
            i.f.cas
                .checked(i.frames.last().ok_or("missing frame")?, &mut budget)?;
        let index =
            i.f.cas
                .state
                .indices
                .peek(&id)
                .copied()
                .ok_or("index missing")?;
        let root = i.f.cas.checked(&i.f.context, &mut budget)?;
        let Vertex::Context(frame) = &mut i.f.cas.state.graph[index] else {
            return Err("context category".into());
        };
        frame.parent = if sever { None } else { Some(root) };
        unsupported(&i)?;
    }
    Ok(())
}

/// An excluded edge above the matched ancestor cannot veto an anchored lower-chain meet.
/// ´test:crate:scalar-transport-lower-chain´
#[test]
fn scalar_transport_lower_chain() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut i = input(0)?;
    i.f.header = admit(
        &mut i.f.cas,
        Draft::Header(crate::domain_model::HeaderDraft::Extend {
            parent: Reference::Existing(i.f.header.clone()),
            declarations: vec![crate::model::Declaration {
                key: "y".into(),
                display_name: "y".into(),
                sort: crate::testing::common::real(),
            }],
            spaces: vec![],
        }),
    )?;
    let root = admit(
        &mut i.f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(i.f.header.clone()),
            admissible: Reference::Existing(i.f.domain.clone()),
            assumptions: vec![],
        }),
    )?;
    i.restriction = region(
        &mut i.f,
        RegionExpr::GuardTrue {
            base: Reference::Existing(i.claim.residuals[0].region.clone()),
            predicate: Reference::Existing(i.claim.residuals[0].predicate.clone()),
        },
    )?;
    i.frames = vec![root];
    for _ in 0..2 {
        let frame = admit(
            &mut i.f.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(i.frames.last().ok_or("parent")?.clone()),
                region: Reference::Existing(i.restriction.clone()),
                hypotheses: vec![],
            }),
        )?;
        i.frames.push(frame);
    }
    let d = i.f.cas.admissible_region(&i.frames[1], &mut budget)?;
    let m = i.f.cas.admissible_region(&i.frames[2], &mut budget)?;
    assert_ne!(
        i.f.cas
            .state
            .region(i.f.domain.logical_id(), &mut budget)?
            .header,
        i.f.header.logical_id()
    );
    i.claim.context = i.frames[2].clone();
    i.claim.residuals[0].home = i.frames[2].clone();
    if let Statement::Equality { domain, .. } = &mut i.claim.statement {
        *domain = m.clone();
    }
    let r = meet(&mut i.f, &[d.clone(), m.clone()])?;
    i.claim.residuals[0].region = r.clone();
    supported(&i)?;
    i.claim.residuals[0].region = meet(&mut i.f, &[r, m.clone(), d])?;
    supported(&i)?;
    let beyond = i.f.domain.clone();
    i.claim.residuals[0].region = meet(&mut i.f, &[m, beyond])?;
    unsupported(&i)
}

/// Constructor membership is not intersection evidence, and malformed edge metadata grants nothing.
/// ´test:crate:scalar-transport-edge-exclusions´
#[test]
fn scalar_transport_edge_exclusions() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    for kind in 0..4 {
        let mut i = input(2)?;
        let m = i.claim.residuals[0].region.clone();
        let d = i.f.cas.admissible_region(&i.frames[1], &mut budget)?;
        i.claim.residuals[0].region = match kind {
            0 => region(
                &mut i.f,
                RegionExpr::Union(vec![Reference::Existing(m), Reference::Existing(d.clone())]),
            )?,
            1 => region(
                &mut i.f,
                RegionExpr::Difference {
                    base: Reference::Existing(m),
                    excluded: Reference::Existing(d.clone()),
                },
            )?,
            _ => meet(&mut i.f, &[m, d.clone()])?,
        };
        if kind >= 2 {
            supported(&i)?;
            let index =
                i.f.cas
                    .state
                    .indices
                    .peek(&d.logical_id())
                    .copied()
                    .ok_or("index missing")?;
            let Vertex::Region(value) = &mut i.f.cas.state.graph[index] else {
                return Err("region".into());
            };
            if kind == 2 {
                value.scope = vec![vec![crate::model::AtomicSort::Scalar(
                    crate::model::Field::Real,
                )]];
            } else {
                value.ambient = AmbientDomain::Assignments;
            }
        }
        unsupported(&i)?;
    }
    Ok(())
}

/// Parent inspection is prepaid even when its category is malformed.
/// ´test:crate:scalar-transport-parent-prepayment´
#[test]
fn scalar_transport_parent_prepayment() -> TestResult {
    for malformed in [false, true] {
        let mut i = measured_input(1)?;
        if malformed {
            let index =
                i.f.cas
                    .state
                    .indices
                    .peek(&i.frames[1].logical_id())
                    .copied()
                    .ok_or("index missing")?;
            let Vertex::Context(frame) = &mut i.f.cas.state.graph[index] else {
                return Err("frame".into());
            };
            frame.parent = Some(i.f.x.logical_id());
        }
        let mut limits = crate::testing::native::fresh_budget()?.limits().clone();
        limits.condition_steps = 1;
        let mut ledger = Budget::new(limits)?;
        let fault = crate::proof_view::scalar_claim(&i.f.cas, &i.claim, &mut ledger)
            .expect_err("parent charge");
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::ConditionSteps,
                ..
            }
        ));
        assert_eq!(ledger.usage().condition_steps, 1);
        assert_eq!(fault.location.operation, "scalar guarded-region transport");
        if malformed {
            let fault = crate::proof_view::scalar_claim(
                &i.f.cas,
                &i.claim,
                &mut crate::testing::native::fresh_budget()?,
            )
            .expect_err("parent category");
            assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
        }
    }
    Ok(())
}

/// A reduced chain allowance refuses before any scheduled member charge.
/// ´test:crate:scalar-transport-depth-limit´
#[test]
fn scalar_transport_depth_limit() -> TestResult {
    let mut i = input(3)?;
    let m = i.claim.residuals[0].region.clone();
    let d = i.f.domain.clone();
    i.claim.residuals[0].region = meet(&mut i.f, &[m, d])?;
    let mut limits = crate::testing::native::fresh_budget()?.limits().clone();
    limits.hypothesis_depth = 2;
    let mut ledger = Budget::new(limits)?;
    ledger.work(37, Location::new("prior scalar transport Work"))?;
    let fault =
        crate::proof_view::scalar_claim(&i.f.cas, &i.claim, &mut ledger).expect_err("depth limit");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::HypothesisDepth,
            ..
        }
    ));
    // Declaration compatibility is checked before capability classification; no member is scheduled.
    assert_eq!(ledger.usage().work, 39);
    Ok(())
}

/// Each projected region position refuses even when the ordinary meet shape is retained.
/// ´test:crate:scalar-transport-projection-exclusion´
#[test]
fn scalar_transport_projection_exclusion() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    for position in 0..4 {
        let mut i = input(2)?;
        let m = i.claim.residuals[0].region.clone();
        let d = i.f.domain.clone();
        let r = meet(&mut i.f, &[m.clone(), d.clone()])?;
        i.claim.residuals[0].region = r.clone();
        supported(&i)?;
        let target = match position {
            0 => r,
            1 => m,
            2 => d,
            _ => i.restriction.clone(),
        };
        let id = i.f.cas.checked(&target, &mut budget)?;
        let index =
            i.f.cas
                .state
                .indices
                .peek(&id)
                .copied()
                .ok_or("index missing")?;
        let Vertex::Region(value) = &mut i.f.cas.state.graph[index] else {
            return Err("region category".into());
        };
        // A kernel mutation isolates the explicit projection exclusion at each position.
        value.lies_over = Some(crate::solving::LiesOver {
            family: id,
            stratum: id,
        });
        unsupported(&i)?;
    }
    Ok(())
}

/// Identity incurs no new inspection, and evidence-category faults remain distinct.
/// ´test:crate:scalar-transport-identity-and-faults´
#[test]
fn scalar_transport_identity_and_faults() -> TestResult {
    let mut i = input(1)?;
    let mut ledger = crate::testing::native::fresh_budget()?;
    ledger.work(37, Location::new("prior scalar transport Work"))?;
    let before = ledger.usage();
    crate::proof_view::scalar_claim(&i.f.cas, &i.claim, &mut ledger)?;
    assert_eq!(ledger.usage(), before);
    i.claim.residuals[0].region = i.f.x.clone();
    let fault = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )
    .expect_err("wrong region category");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
    i.claim.residuals[0].predicate = i.f.x.clone();
    let fault = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )
    .expect_err("old scalar category refusal");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fault.location.operation, "scalar statement view");
    Ok(())
}

/// An admitted formal local frame retains its lifted region and remains outside closed conversion.
/// ´test:crate:scalar-transport-formal-frame´
#[test]
fn scalar_transport_formal_frame() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut i = input(0)?;
    let lifted = admit(
        &mut i.f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(i.f.header.clone()),
            scope: vec![vec![crate::model::AtomicSort::Scalar(
                crate::model::Field::Complex,
            )]],
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    let frame = admit(
        &mut i.f.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(i.f.context.clone()),
            region: Reference::Existing(lifted.clone()),
            hypotheses: vec![],
        }),
    )?;
    assert_eq!(i.f.cas.admissible_region(&frame, &mut budget)?, lifted);
    i.claim.context = frame;
    let before =
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?;
    let fault = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )
    .expect_err("formal conversion");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fault.location.operation, "closed context required");
    assert_eq!(
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?,
        before
    );
    Ok(())
}

/// A compatible-looking predicate in a distinct declaration frame remains a context fault.
/// ´test:crate:scalar-transport-declaration-frame´
#[test]
fn scalar_transport_declaration_frame() -> TestResult {
    let mut i = input(1)?;
    let m = i.claim.residuals[0].region.clone();
    let d = i.f.domain.clone();
    i.claim.residuals[0].region = meet(&mut i.f, &[m, d])?;
    let header = admit(
        &mut i.f.cas,
        Draft::Header(crate::domain_model::HeaderDraft::Extend {
            parent: Reference::Existing(i.f.header.clone()),
            declarations: vec![crate::model::Declaration {
                key: "other-x".into(),
                display_name: "x".into(),
                sort: crate::testing::common::real(),
            }],
            spaces: vec![],
        }),
    )?;
    i.claim.residuals[0].predicate = predicate(
        &mut i.f.cas,
        &header,
        PredicateKind::NonZero,
        &[i.f.x.clone()],
    )?;
    let fault = crate::proof_view::scalar_claim(
        &i.f.cas,
        &i.claim,
        &mut crate::testing::native::fresh_budget()?,
    )
    .expect_err("predicate declaration frame");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Context));
    Ok(())
}

fn measured_input(kind: usize) -> TestResult<Input> {
    let mut i = input(if kind == 2 { 3 } else { 1 })?;
    let m = i.claim.residuals[0].region.clone();
    let d = i.f.domain.clone();
    if kind != 0 {
        let r = meet(&mut i.f, &[m.clone(), d.clone()])?;
        i.claim.residuals[0].region = match kind {
            3 => meet(&mut i.f, &[r, m, d])?,
            4 => meet(&mut i.f, &[m, i.restriction.clone()])?,
            _ => r,
        };
    }
    Ok(i)
}

fn attempt(i: &Input, resource: BudgetKind, amount: u64) -> TestResult<(bool, u64, u64)> {
    let mut limits = crate::testing::native::fresh_budget()?.limits().clone();
    match resource {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::ConditionSteps => limits.condition_steps = amount,
        BudgetKind::Depth => limits.depth = amount,
        _ => return Err("measurement resource".into()),
    }
    let mut ledger = Budget::new(limits)?;
    ledger.work(37, Location::new("prior scalar transport Work"))?;
    let before =
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?;
    let result = ledger.with_retained_payload(
        19,
        Location::new("prior scalar transport payload"),
        |ledger| crate::proof_view::scalar_claim(&i.f.cas, &i.claim, ledger),
    );
    assert_eq!(
        i.f.cas
            .canonical_snapshot(&mut crate::testing::native::fresh_budget()?)?,
        before
    );
    let completed = match result {
        Ok(_) => true,
        Err(fault) if fault.kind == FaultKind::Invalid(InvalidCode::ScalarTransportUnsupported) => {
            true
        }
        Err(fault) => {
            assert!(
                matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == resource)
            );
            false
        }
    };
    Ok((
        completed,
        ledger.usage().work,
        ledger.usage().condition_steps,
    ))
}

/// Direct conversion prepays entries, retains their workspace and consumes only its checked prefix.
/// ´test:crate:scalar-transport-resource-boundaries´
#[test]
fn scalar_transport_resource_boundaries() -> TestResult {
    // Complete scalar_claim operations include 37 prior Work and 19 live caller bytes.
    // Identity schedules no entries or additional Work.
    let exact = measured_input(0)?;
    for (resource, amount) in [
        (BudgetKind::Work, 37),
        (BudgetKind::StagedPayloadBytes, 19),
        (BudgetKind::ConditionSteps, 0),
        (BudgetKind::Depth, 0),
    ] {
        assert_eq!(attempt(&exact, resource, amount)?, (true, 37, 0));
    }
    // one ancestor, deeper ancestry, nested meets, and completed unsupported inspection.
    for (
        kind,
        work,
        conditions,
        staged,
        staged_prefix,
        staged_conditions,
        depth,
        depth_prefix,
        depth_conditions,
        condition_work,
    ) in [
        (1, 455, 4, 317, 402, 4, 2, 199, 2, 392),
        (2, 707, 8, 317, 528, 6, 2, 325, 4, 644),
        (3, 871, 8, 605, 818, 8, 3, 615, 6, 808),
        (4, 455, 4, 317, 402, 4, 2, 199, 2, 392),
    ] {
        let i = measured_input(kind)?;
        for (resource, amount, prefix, condition_prefix) in [
            (BudgetKind::Work, work, work - 2, conditions),
            (
                BudgetKind::ConditionSteps,
                conditions,
                condition_work,
                conditions - 1,
            ),
            (
                BudgetKind::StagedPayloadBytes,
                staged,
                staged_prefix,
                staged_conditions,
            ),
            (BudgetKind::Depth, depth, depth_prefix, depth_conditions),
        ] {
            assert_eq!(attempt(&i, resource, amount)?, (true, work, conditions));
            assert_eq!(
                attempt(&i, resource, amount - 1)?,
                (false, prefix, condition_prefix)
            );
        }
    }
    Ok(())
}
