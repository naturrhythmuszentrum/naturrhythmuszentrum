// SPDX-License-Identifier: AGPL-3.0-only
//! Gaussian constants carried through explicit assumptions and independent expected-claim replay.

use crate::testing::common::TestResult;
use crate::testing::solve_graph::admit;
use crate::testing::solve_numeric::{parts, stored};
use nrz_cas::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
    RegionExpr,
};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::solving::{CoefficientMapData, MapMode, MapTarget, SolveDraft};
use nrz_cas::syntax::{Branch, Draft, Environment, Field, PredicateKind};
use nrz_cas::{Cas, Handle, Reference};

/// Admit a standard Gaussian embedding into the declared complex scalar ambient.
fn literal(cas: &mut Cas, domain: Handle, value: Handle) -> TestResult<(Handle, Handle, Handle)> {
    let header = admit(
        cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations: Vec::new(),
        })),
    )?;
    let region = admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let map = admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Embedding,
            region: Reference::Existing(region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    let value = admit(
        cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(header.clone()),
            value: Reference::Existing(value),
            map: Reference::Existing(map),
        },
    )?;
    Ok((header, region, value))
}

/// Supply an explicit hypothesis about the literal; this does not assert a coefficient theorem.
fn supplied(
    cas: &mut Cas,
    header: Handle,
    region: Handle,
    value: Handle,
) -> TestResult<VerifiedWitness> {
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Named("given-gaussian-property".into()),
            arguments: vec![Reference::Existing(value)],
        },
    )?;
    let context = admit(
        cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "given".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let step = ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact { region, predicate },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: context.clone(),
            slot: 0,
        }),
    };
    Ok(cas.check_proof(
        &context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::default())?,
    )?)
}

/// Construct each expected claim independently using public graph and hypothesis APIs.
fn fixture(imaginary: &str) -> TestResult<(Cas, VerifiedWitness)> {
    let (real, imaginary) = parts("-355/113", imaginary)?;
    let (mut cas, domain, value) = stored(&real, &imaginary)?;
    let (header, region, value) = literal(&mut cas, domain, value)?;
    let witness = supplied(&mut cas, header, region, value)?;
    Ok((cas, witness))
}

/// Exact nonintegral Gaussian components survive export and replay against an independently built claim.
/// ´test:integration:gaussian-literal-expected-replay´
#[test]
fn gaussian_literal_expected_replay() -> TestResult {
    let (origin, witness) = fixture("22/7")?;
    let bytes = origin.export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    let (mut receiver, expected) = fixture("44/14")?;
    let checked = receiver.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(checked.judgment(), expected.judgment());
    assert_eq!(checked.hypotheses().len(), 1);
    assert_eq!(
        receiver.export_witness(&checked, &mut Budget::new(Limits::default())?)?,
        bytes
    );
    Ok(())
}

/// A changed imaginary component cannot be accepted as the caller's expected hypothesis claim.
/// ´test:integration:gaussian-literal-wrong-component-refused´
#[test]
fn gaussian_literal_wrong_component_refused() -> TestResult {
    let (origin, witness) = fixture("22/7")?;
    let bytes = origin.export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    let (mut receiver, expected) = fixture("-22/7")?;
    let before = receiver.counts();
    assert!(
        receiver
            .replay(
                &bytes,
                expected.judgment(),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(receiver.counts(), before);
    Ok(())
}
