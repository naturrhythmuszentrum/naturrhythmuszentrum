// SPDX-License-Identifier: AGPL-3.0-only
//! Materialize and independently check regional consequences before arithmetic uses them.

use super::construction::{Construction, admit};
use crate::proof_model::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::solving::coefficients::{
    arithmetic::Engine,
    consequence::Consequence,
    fraction::Fraction,
    model::{CoefficientFailure, GuardedObligation, Value},
    polynomial::Polynomial,
    storage::{invalid, vector},
};
use crate::solving::{
    CertificateCoefficientIdentity, IdentityMode, RelationMultiple, SolveCertificate, SolveSchema,
};
use crate::{
    budget::Budget,
    error::Fault,
    graph::{Cas, Draft, Handle, Reference},
};

/// Total polynomial syntax keeps every original parameter coordinate.
fn coefficient(
    cas: &mut Cas,
    source: &Handle,
    polynomial: &Polynomial,
    budget: &mut Budget,
) -> Result<Handle, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: source,
    };
    let value = engine.element(Value::Fraction(Fraction {
        numerator: polynomial.copy(budget)?,
        denominator: polynomial.one(budget)?,
    }));
    Ok(crate::solving::towers::materialize::value(
        cas, &value, budget,
    )?)
}

/// Every guard cofactor and nonzero factor is explicit graph data.
fn certificate(
    cas: &mut Cas,
    construction: &Construction,
    obligation: &GuardedObligation,
    found: &Consequence,
    budget: &mut Budget,
) -> Result<CertificateCoefficientIdentity, CoefficientFailure> {
    let images = crate::solving::solve_search::coefficient_images(
        cas,
        construction,
        &obligation.domain,
        budget,
    )?;
    let map = construction.mapping(cas, &obligation.domain, &images, budget)?;
    let left = coefficient(cas, &obligation.domain, &obligation.polynomial, budget)?;
    let right = coefficient(cas, &obligation.domain, &found.product, budget)?;
    let mut combinations = vector(found.multiples.len(), budget)?;
    for (cofactor, relation) in &found.multiples {
        combinations.push(RelationMultiple {
            multiplier: coefficient(cas, &obligation.domain, cofactor, budget)?,
            relation: coefficient(cas, &obligation.domain, relation, budget)?,
            hypothesis: None,
        });
    }
    let mut denominators = vector(found.factors.len(), budget)?;
    for factor in &found.factors {
        denominators.push(coefficient(cas, &obligation.domain, factor, budget)?);
    }
    Ok(CertificateCoefficientIdentity {
        mode: IdentityMode::NonzeroPower(found.exponent),
        left,
        right,
        map,
        region: obligation.region.clone(),
        combinations,
        denominators,
    })
}

/// The predicate names precisely the queried coefficient and its canonical parameter map.
fn conclusion(
    cas: &mut Cas,
    construction: &Construction,
    certificate: &CertificateCoefficientIdentity,
    budget: &mut Budget,
) -> Result<Statement, Fault> {
    let literal = admit(
        cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(construction.header.clone()),
            value: Reference::Existing(certificate.left.clone()),
            map: Reference::Existing(certificate.map.clone()),
        },
        budget,
    )?;
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            kind: crate::model::PredicateKind::NonZero,
            arguments: vec![Reference::Existing(literal)],
        },
        budget,
    )?;
    Ok(Statement::PredicateFact {
        predicate,
        region: certificate.region.clone(),
    })
}

/// Search supplies data; independent replay alone creates usable nonvanishing evidence.
pub(crate) fn certify(
    cas: &mut Cas,
    construction: &mut Construction,
    obligation: &GuardedObligation,
    budget: &mut Budget,
) -> Result<Option<VerifiedWitness>, CoefficientFailure> {
    if obligation.context != construction.context || obligation.region != construction.region {
        return Err(invalid().into());
    }
    if !obligation.localization.is_empty() {
        return Ok(None);
    }
    let Some(found) = crate::solving::coefficients::consequence::search(
        &cas.state,
        &obligation.domain,
        &obligation.polynomial,
        &obligation.region,
        budget,
    )?
    else {
        return Ok(None);
    };
    let supplied = certificate(cas, construction, obligation, &found, budget)?;
    let statement = conclusion(cas, construction, &supplied, budget)?;
    let payload = Certificate::Solve(Box::new(SolveCertificate::CoefficientIdentity(supplied)));
    let rule = crate::rule::Primitive::Solve(SolveSchema::CoefficientIdentity);
    let step = ApplicationDraft {
        rule,
        context: construction.context.clone(),
        conclusion: statement.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: payload.clone(),
    };
    let witness = cas.check_proof(&construction.context, &[step], budget)?;
    construction.append(rule, statement, payload, budget)?;
    Ok(Some(witness))
}
