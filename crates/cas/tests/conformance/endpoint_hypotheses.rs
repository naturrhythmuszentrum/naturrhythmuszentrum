// SPDX-License-Identifier: AGPL-3.0-only
//! Exact predicate slots and ancestry at scalar endpoint boundaries.

use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use nrz_cas::syntax::Draft;
use nrz_cas::{FaultKind, Reference};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::endpoint_hypotheses::chain;

/// Scalar conditions retain exactly the introducing predicate slot, including ancestor use.
/// ´test:integration:endpoint-current-ancestor-slot´
#[test]
fn endpoint_current_ancestor_slot() -> TestResult {
    let mut f = chain(false, false)?;
    for nested in [false, true] {
        let context = if nested {
            let region = f
                .fixture
                .cas
                .admissible_region(&f.frame, &mut crate::testing::budgets::graph_budget()?)?;
            admit(
                &mut f.fixture.cas,
                Draft::Context(ContextDraft::Local {
                    parent: Reference::Existing(f.frame.clone()),
                    region: Reference::Existing(region),
                    hypotheses: vec![],
                }),
                &mut budget()?,
            )?
        } else {
            f.frame.clone()
        };
        let mut step = f.steps[0].clone();
        step.context = context.clone();
        let Statement::Equality { domain, .. } = &mut step.conclusion else {
            return Err("equality".into());
        };
        *domain = f
            .fixture
            .cas
            .admissible_region(&context, &mut crate::testing::budgets::graph_budget()?)?;
        let witness = f
            .fixture
            .cas
            .check_proof(&context, &[step], &mut budget()?)?;
        assert_eq!(witness.judgment().residuals, []);
        assert_eq!(
            witness.hypotheses(),
            &[HypothesisRef {
                context: f.frame.clone(),
                slot: 1
            }]
        );
    }
    Ok(())
}

/// Foreign, sibling, absent and membership references cannot supply a predicate theorem.
/// ´test:integration:endpoint-invalid-qualified-slots´
#[test]
fn endpoint_invalid_qualified_slots() -> TestResult {
    let mut f = chain(false, false)?;
    let foreign = chain(false, false)?;
    let sibling = admit(
        &mut f.fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(f.fixture.context.clone()),
            region: Reference::Existing(f.fixture.domain.clone()),
            hypotheses: vec![AssumptionDraft {
                key: "sibling-x".into(),
                condition: Reference::Existing(f.predicate.clone()),
            }],
        }),
        &mut budget()?,
    )?;
    let region = f
        .fixture
        .cas
        .admissible_region(&f.frame, &mut crate::testing::budgets::graph_budget()?)?;
    for reference in [
        HypothesisRef {
            context: foreign.frame,
            slot: 1,
        },
        HypothesisRef {
            context: sibling,
            slot: 1,
        },
        HypothesisRef {
            context: f.frame.clone(),
            slot: u32::MAX,
        },
        HypothesisRef {
            context: f.frame.clone(),
            slot: 0,
        },
    ] {
        let step = ApplicationDraft {
            context: f.frame.clone(),
            rule: Primitive::ContextHypothesis,
            conclusion: Statement::PredicateFact {
                region: region.clone(),
                predicate: f.predicate.clone(),
            },
            premises: vec![],
            substitutions: vec![],
            certificate: Certificate::Hypothesis(reference),
        };
        let before = f.fixture.cas.canonical_snapshot(&mut budget()?)?;
        let fault = f
            .fixture
            .cas
            .check_proof(&f.frame, &[step], &mut budget()?)
            .err()
            .ok_or("invalid reference accepted")?;
        assert!(matches!(fault.kind, FaultKind::Invalid(_)));
        assert_eq!(f.fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok(())
}

/// Changing only a predicate's declaration header cannot reuse the retained frame slot.
/// ´test:integration:endpoint-predicate-declaration-frame´
#[test]
fn endpoint_predicate_declaration_frame() -> TestResult {
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::{Declaration, PredicateKind};
    let mut f = chain(false, false)?;
    let header = admit(
        &mut f.fixture.cas,
        Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(f.fixture.header.clone()),
            declarations: vec![Declaration {
                key: "y".into(),
                display_name: "y".into(),
                sort: crate::testing::common::real(),
            }],
            spaces: vec![],
        }),
        &mut budget()?,
    )?;
    assert_ne!(header, f.fixture.header);
    let predicate = admit(
        &mut f.fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header),
            scope: vec![],
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(f.fixture.x.clone())],
        },
        &mut budget()?,
    )?;
    assert_ne!(predicate, f.predicate);
    let step = ApplicationDraft {
        context: f.frame.clone(),
        rule: Primitive::ContextHypothesis,
        conclusion: Statement::PredicateFact {
            region: f
                .fixture
                .cas
                .admissible_region(&f.frame, &mut crate::testing::budgets::graph_budget()?)?,
            predicate,
        },
        premises: vec![],
        substitutions: vec![],
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: f.frame.clone(),
            slot: 1,
        }),
    };
    let before = f.fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = f
        .fixture
        .cas
        .check_proof(&f.frame, &[step], &mut budget()?)
        .err()
        .ok_or("changed declaration frame accepted")?;
    assert!(matches!(fault.kind, FaultKind::Invalid(_)));
    assert_eq!(f.fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// An available predicate in a sibling frame supplies no implicit endpoint authority.
/// ´test:integration:endpoint-sibling-lookup-residual´
#[test]
fn endpoint_sibling_lookup_residual() -> TestResult {
    let mut f = chain(false, false)?;
    let sibling = admit(
        &mut f.fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(f.fixture.context.clone()),
            region: Reference::Existing(f.fixture.domain.clone()),
            hypotheses: vec![],
        }),
        &mut budget()?,
    )?;
    let mut step = f.steps[0].clone();
    step.context = sibling.clone();
    let Statement::Equality { domain, .. } = &mut step.conclusion else {
        return Err("equality".into());
    };
    *domain = f
        .fixture
        .cas
        .admissible_region(&sibling, &mut crate::testing::budgets::graph_budget()?)?;
    let witness = f
        .fixture
        .cas
        .check_proof(&sibling, &[step], &mut budget()?)?;
    assert_eq!(witness.hypotheses(), []);
    assert_eq!(witness.judgment().residuals.len(), 1);
    assert_eq!(witness.judgment().residuals[0].predicate, f.predicate);
    Ok(())
}
