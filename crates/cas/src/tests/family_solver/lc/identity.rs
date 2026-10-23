// SPDX-License-Identifier: AGPL-3.0-only
//! An explicit checked product sign supplies the native component denominator premise.

use crate::budget::Budget;
use crate::graph::{Draft, Handle};
use crate::model::{Field, Operator, PredicateKind};
use crate::proof::{
    ApplicationDraft, Certificate, Judgment, Primitive, Statement, VerifiedWitness,
};
use crate::solving::{SolveCertificate, SolveSchema};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};

fn premise(
    fixture: &mut Fixture,
    claim: &Judgment,
    generation: &mut Budget,
    checking: &mut Budget,
) -> TestResult<VerifiedWitness> {
    let capacitance = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Free("C".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let product = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Mul,
        Vec::new(),
        vec![reference(&fixture.real), reference(&capacitance)],
    )?;
    let region = fixture.cas.admissible_region(&claim.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(&product)],
        },
    )?;
    let certificate = crate::strict_sign::certificate(&fixture.cas, &product, &region, generation)?;
    Ok(fixture.cas.check_proof(
        &claim.context,
        &[ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::RealSign),
            context: claim.context.clone(),
            conclusion: Statement::PredicateFact { predicate, region },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::RealSign(certificate))),
        }],
        checking,
    )?)
}

pub(super) fn prove(
    fixture: &mut Fixture,
    claim: &Judgment,
    generation: &mut Budget,
    checking: &mut Budget,
) -> TestResult<VerifiedWitness> {
    let sign = premise(fixture, claim, generation, checking)?;
    let supplied: &[Handle] = std::slice::from_ref(sign.proof());
    let policy = crate::search::SearchPolicy::default();
    let mut plan = crate::case_producer::Plan::new(&policy);
    let proof = crate::solving::family::recipes::push(
        &mut plan,
        &mut fixture.cas,
        &claim.context,
        Primitive::CaseCongruence,
        &claim.statement,
        supplied,
        generation,
        checking,
    )?;
    Ok(plan
        .finish(&fixture.cas, &proof, Vec::new(), checking)?
        .witness)
}
