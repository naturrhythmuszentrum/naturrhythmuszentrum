// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical coefficient closures survive the ordinary typed witness lifecycle.

use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::Fixture;
use crate::testing::solve_graph::admit;
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::solving::{CoefficientMapData, MapMode, MapTarget, SolveDraft};
use nrz_cas::syntax::{Draft, Field, PredicateKind};
use nrz_cas::{Cas, Reference};

/// A supplied contextual property makes the fraction closure reachable without asserting an identity theorem.
fn fraction_witness() -> TestResult<(Cas, VerifiedWitness)> {
    let mut fixture = Fixture::new()?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let bottom = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let proposed = fixture.fraction(&top, &bottom);
    let value = fixture
        .cas
        .admit(
            std::slice::from_ref(&proposed),
            &mut Budget::new(Limits::blind_cubic())?,
        )?
        .into_iter()
        .next()
        .ok_or("missing fraction")?;
    let map = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(fixture.fractions),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(fixture.header.clone()),
                field: Field::Real,
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(fixture.region.clone()),
            images: Vec::new(),
            denominators: vec![Reference::Existing(bottom)],
        })),
    )?;
    let literal = admit(
        &mut fixture.cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(fixture.header.clone()),
            value: Reference::Existing(value),
            map: Reference::Existing(map),
        },
    )?;
    let predicate = admit(
        &mut fixture.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Named("supplied-fraction-property".into()),
            arguments: vec![Reference::Existing(literal)],
        },
    )?;
    let context = admit(
        &mut fixture.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header),
            admissible: Reference::Existing(fixture.region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "given".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let step = ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact {
            region: fixture.region,
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: context.clone(),
            slot: 0,
        }),
    };
    let witness = fixture.cas.check_proof(
        &context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok((fixture.cas, witness))
}

/// Fresh replay revalidates multivariate canonical fractions and retains the explicit used hypothesis.
/// ´test:integration:fraction-closure-replay´
#[test]
fn fraction_closure_replay() -> TestResult {
    let (source, witness) = fraction_witness()?;
    let bytes = source.export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, expected) = fraction_witness()?;
    let replayed = target.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(
        target.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}
