// SPDX-License-Identifier: AGPL-3.0-only
//! Kernel evidence mutations and exact guarded-condition provenance for native cases.

use crate::case_producer::Plan;
use crate::domain_model::{ContextDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, Reference};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{ApplicationDraft, Certificate, DomainContract, Statement};
use crate::rule::Primitive;
use crate::search::SearchPolicy;
use crate::source_model::{SourceClauseDraft, SourceFormDraft, SourceFormKind};
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, admit, budget, predicate, term};

/// Certificates cannot omit a row, flip a cell or insert an unused syntax role.
/// ´test:crate:case-certificate-tamper-atomicity´
#[test]
fn case_certificate_tamper_atomicity() -> TestResult {
    let mut f = Fixture::new()?;
    let policy = SearchPolicy::default();
    let mut plan = Plan::new(&policy);
    let conclusion = Statement::Equality {
        left: f.cases.clone(),
        right: f.zero.clone(),
        domain: f.domain.clone(),
        domain_contract: DomainContract::SameDefinedDomain,
    };
    let proof = plan.push(
        &mut f.cas,
        &f.context,
        Primitive::CaseCongruence,
        conclusion.clone(),
        &[],
        Certificate::None,
        &mut budget()?,
        &mut budget()?,
    )?;
    let original = crate::proof_view::application(&f.cas, proof.logical_id(), &mut budget()?)?;
    for mutation in 0..3 {
        let mut certificate = original.certificate.clone();
        let Certificate::Cases(value) = &mut certificate else {
            return Err("expected case evidence".into());
        };
        match mutation {
            0 => {
                value.regions.pop();
            }
            1 => {
                let cell = value
                    .cells
                    .first_mut()
                    .and_then(|cells| cells.membership.first_mut())
                    .and_then(|row| row.first_mut())
                    .ok_or("missing cell")?;
                *cell = !*cell;
            }
            _ => value.arms.push(f.zero.clone()),
        }
        let draft = ApplicationDraft {
            context: f.context.clone(),
            rule: Primitive::CaseCongruence,
            conclusion: conclusion.clone(),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate,
        };
        let before = f.cas.canonical_snapshot(&mut budget()?)?;
        assert!(
            f.cas
                .check_proof(&f.context, &[draft], &mut budget()?)
                .is_err()
        );
        assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok(())
}

/// The reciprocal denominator requirement is confined to the punctured reached branch.
/// ´test:crate:case-reached-nonzero-requirement´
#[test]
fn case_reached_nonzero_requirement() -> TestResult {
    let mut f = Fixture::new()?;
    let guard = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::Equal,
        &[f.x.clone(), f.zero.clone()],
    )?;
    let reciprocal = term(
        &mut f.cas,
        &f.header,
        Operator::Div,
        &[f.one.clone(), f.x.clone()],
    )?;
    let source = admit(
        &mut f.cas,
        Draft::SourceForm(SourceFormDraft {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            kind: SourceFormKind::OrderedCases,
            domain: Reference::Existing(f.domain.clone()),
            clauses: vec![SourceClauseDraft {
                selector: Reference::Existing(guard),
                value: Reference::Existing(f.zero.clone()),
            }],
            otherwise: Some(Reference::Existing(reciprocal)),
        }),
    )?;
    let context = f.context.logical_id();
    let built = crate::case_ordered::construct(
        &mut f.cas,
        context,
        source.logical_id(),
        &[],
        &mut budget()?,
    )?;
    let no_residuals: &[crate::proof_model::GuardedObligation] = &[];
    assert_eq!(built.conditions.residuals.as_slice(), no_residuals);
    let mut found = 0;
    for required in &built.conditions.requirements {
        if f.cas.predicate_kind(&required.predicate)? == &PredicateKind::NonZero {
            let cells = crate::region::partition(
                &f.cas.state,
                &[required.region.logical_id()],
                &mut budget()?,
            )?;
            assert_eq!(cells.cuts, vec![crate::number::Rational::zero()]);
            assert_eq!(cells.membership, vec![vec![true, false, true]]);
            assert_eq!(required.home, f.context);
            found += 1;
        }
    }
    assert_eq!(found, 1);
    Ok(())
}

/// Replacing a recorded closed frame with a sibling cannot preserve elimination evidence.
/// ´test:crate:case-elimination-frame-identity´
#[test]
fn case_elimination_frame_identity() -> TestResult {
    let mut f = Fixture::new()?;
    let policy = SearchPolicy::default();
    let mut plan = Plan::new(&policy);
    let root = crate::case_recipes::merge(
        &mut plan,
        &mut f.cas,
        &f.context,
        &f.cases,
        &f.zero,
        &[f.domain.clone()],
        &mut budget()?,
        &mut budget()?,
    )?;
    let root = crate::proof_view::application(&f.cas, root.logical_id(), &mut budget()?)?;
    let id = *root.premises.get(2).ok_or("missing elimination")?;
    let mut elimination = crate::proof_view::application(&f.cas, id, &mut budget()?)?;
    let assumption = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::NonZero,
        &[f.x.clone()],
    )?;
    let sibling = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(f.context.clone()),
            region: Reference::Existing(f.domain.clone()),
            hypotheses: vec![crate::graph::AssumptionDraft {
                key: "sibling".into(),
                condition: Reference::Existing(assumption),
            }],
        }),
    )?;
    let Certificate::Cases(certificate) = &mut elimination.certificate else {
        return Err("missing case evidence".into());
    };
    *certificate.frames.first_mut().ok_or("missing frame")? = sibling;
    let mut earlier = Vec::new();
    for id in &elimination.premises {
        earlier.push(crate::proof_view::application(&f.cas, *id, &mut budget()?)?);
    }
    let draft = ApplicationDraft {
        context: f.context,
        rule: Primitive::CaseEliminate,
        conclusion: elimination.judgment.statement,
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: elimination.certificate,
    };
    let mut candidate = f.cas.fork(&mut budget()?)?;
    assert!(
        crate::scoped_check::check_application(&mut candidate, &draft, &earlier, &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// Exact predicate facts work on nonempty regions and vacuity never evaluates an undefined atom.
/// ´test:crate:region-predicate-fact-defined´
#[test]
fn region_predicate_fact_defined() -> TestResult {
    let mut f = Fixture::new()?;
    let nz = predicate(
        &mut f.cas,
        &f.header,
        PredicateKind::NonZero,
        &[f.x.clone()],
    )?;
    let region = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            ambient: crate::domain_model::AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(f.domain.clone()),
                predicate: Reference::Existing(nz.clone()),
            },
        }),
    )?;
    let draft = ApplicationDraft {
        context: f.context.clone(),
        rule: Primitive::RegionDecide,
        conclusion: Statement::PredicateFact {
            region,
            predicate: nz,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Region(crate::region::RegionCertificate {
            cuts: vec![crate::number::Rational::zero()],
            membership: vec![vec![true, false, true]; 2],
        }),
    };
    assert!(
        f.cas
            .check_proof(&f.context, &[draft], &mut budget()?)?
            .is_discharged()
    );
    Ok(())
}

/// A checked empty region discharges an otherwise undefined atom before evaluating it.
/// ´test:crate:region-predicate-fact-vacuous´
#[test]
fn region_predicate_fact_vacuous() -> TestResult {
    let mut f = Fixture::new()?;
    let bad = term(
        &mut f.cas,
        &f.header,
        Operator::Div,
        &[f.one.clone(), f.zero.clone()],
    )?;
    let predicate = predicate(&mut f.cas, &f.header, PredicateKind::Positive, &[bad])?;
    let empty = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header),
            scope: Vec::new(),
            ambient: crate::domain_model::AmbientDomain::Real("x".into()),
            expression: RegionExpr::Empty,
        }),
    )?;
    let draft = ApplicationDraft {
        context: f.context.clone(),
        rule: Primitive::RegionDecide,
        conclusion: Statement::PredicateFact {
            region: empty,
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Region(crate::region::RegionCertificate {
            cuts: Vec::new(),
            membership: vec![vec![false]],
        }),
    };
    assert!(
        f.cas
            .check_proof(&f.context, &[draft], &mut budget()?)?
            .is_discharged()
    );
    Ok(())
}

/// Bound instantiation reaches native values, predicate arguments, regions and boundary locations.
/// ´test:crate:case-bound-instantiation-typed-dependencies´
#[test]
fn case_bound_instantiation_typed_dependencies() -> TestResult {
    let mut f = Fixture::new()?;
    let (cases, boundary) = crate::testing::native::scoped_cases(&mut f)?;
    let output = f
        .cas
        .instantiate_bound(&cases, &[f.one.clone()], &mut budget()?)?;
    let expression = f.cas.state.expression(output.logical_id())?;
    let no_frames: &[Vec<crate::model::AtomicSort>] = &[];
    assert_eq!(expression.scope.as_slice(), no_frames);
    let arm = f
        .cas
        .state
        .arm(*expression.arms.first().ok_or("missing instantiated arm")?)?;
    assert_eq!(arm.value, f.one.logical_id());
    let region = f.cas.state.region(arm.region)?;
    assert_eq!(region.scope.as_slice(), no_frames);
    let RegionExpr::GuardTrue { predicate, .. } = region.expression else {
        return Err("missing predicate".into());
    };
    assert_eq!(
        f.cas
            .predicate_arguments(&f.cas.handle(predicate), &mut budget()?)?,
        vec![f.one.clone()]
    );
    let point = f
        .cas
        .instantiate_bound(&boundary, &[f.one.clone()], &mut budget()?)?;
    assert_eq!(
        f.cas.state.boundary(point.logical_id())?.location,
        f.one.logical_id()
    );
    let root = f.cas.materialize_derived(&output, &cases, &mut budget()?)?;
    assert!(
        !f.cas
            .walk_derived(&root, &mut budget()?)?
            .receipt
            .is_original()
    );
    Ok(())
}
