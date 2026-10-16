// SPDX-License-Identifier: AGPL-3.0-only
//! Independent coefficient identity replay with exact maps and guarded denominators.

use super::arithmetic::Engine;
use super::normalize::Interpretation;
use super::proof::{conditions, fault, identity_map, mapping};
use super::storage::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::solving::{
    CertificateCoefficientIdentity, IdentityMode, SolveCertificate, SolveStatement,
};

/// The conclusion must repeat precisely the certificate endpoints, map and region.
fn conclusion(
    draft: &ApplicationDraft,
    certificate: &CertificateCoefficientIdentity,
) -> Result<(), Fault> {
    match &draft.conclusion {
        Statement::Solve(statement) => match statement.as_ref() {
            SolveStatement::CoefficientEquality(value)
                if value.left == certificate.left
                    && value.right == certificate.right
                    && value.map == certificate.map
                    && value.region == certificate.region =>
            {
                Ok(())
            }
            _ => Err(invalid()),
        },
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } if *left == certificate.left
            && *right == certificate.right
            && *domain == certificate.region =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// A relation multiple requires an earlier exact equality of that relation to formal zero.
fn relation(
    interpreter: &Interpretation<'_>,
    handle: &crate::graph::Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in interpreter.premises {
        budget.work(1, AT)?;
        let Statement::Solve(statement) = &premise.judgment.statement else {
            continue;
        };
        let SolveStatement::CoefficientEquality(equality) = statement.as_ref() else {
            continue;
        };
        if equality.map != *interpreter.map || equality.region != *interpreter.region {
            continue;
        }
        let endpoint = if equality.left == *handle {
            &equality.right
        } else if equality.right == *handle {
            &equality.left
        } else {
            continue;
        };
        if interpreter
            .endpoint(endpoint, budget)
            .map_err(|error| fault(&error))?
            .terms
            .is_empty()
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Replay both endpoints and every supplied ideal-combination term using independent products.
fn calculate(
    interpreter: &Interpretation<'_>,
    certificate: &CertificateCoefficientIdentity,
    context: &crate::graph::Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = interpreter
        .endpoint(&certificate.left, budget)
        .map_err(|error| fault(&error))?;
    let right = interpreter
        .endpoint(&certificate.right, budget)
        .map_err(|error| fault(&error))?;
    let engine = &interpreter.engine;
    let mut difference = left
        .add(
            &right.neg(engine, budget).map_err(|error| fault(&error))?,
            engine,
            budget,
        )
        .map_err(|error| fault(&error))?;
    for multiple in &certificate.combinations {
        if let Some(evidence) = &multiple.hypothesis {
            super::scoped::relation(interpreter, context, multiple, evidence, budget)?;
        } else {
            relation(interpreter, &multiple.relation, budget)?;
        }
        let multiplier = interpreter
            .endpoint(&multiple.multiplier, budget)
            .map_err(|error| fault(&error))?;
        let relation_form = interpreter
            .endpoint(&multiple.relation, budget)
            .map_err(|error| fault(&error))?;
        let product = multiplier
            .mul(&relation_form, engine, budget)
            .map_err(|error| fault(&error))?;
        difference = difference
            .add(
                &product.neg(engine, budget).map_err(|error| fault(&error))?,
                engine,
                budget,
            )
            .map_err(|error| fault(&error))?;
    }
    for term in &difference.terms {
        if !super::proof::zero(interpreter, &term.coefficient, budget)? {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Typed identity checking is read-only; publication remains the common atomic transaction's job.
pub(crate) fn check(
    cas: &crate::graph::Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    budget.solving(BudgetKind::SolveIdentities, 1, AT)?;
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let SolveCertificate::CoefficientIdentity(certificate) = certificate.as_ref() else {
        return Err(invalid());
    };
    if draft.rule != crate::rule::Primitive::Solve(crate::solving::SolveSchema::CoefficientIdentity)
    {
        return Err(invalid());
    }
    conclusion(draft, certificate)?;
    if certificate.mode == IdentityMode::Normalize && !certificate.combinations.is_empty() {
        return Err(invalid());
    }
    let mut inherited = conditions(cas, draft, premises, budget)?;
    let map = mapping(
        cas,
        &certificate.map,
        &draft.context,
        &certificate.region,
        budget,
    )?;
    identity_map(cas, map, premises, budget)?;
    if certificate.denominators != map.denominators {
        return Err(invalid());
    }
    for denominator in &certificate.denominators {
        super::proof::denominator(
            cas,
            denominator,
            &map.source,
            &certificate.region,
            premises,
            budget,
        )?;
    }
    let interpreter = Interpretation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &map.source,
        },
        map: &certificate.map,
        region: &certificate.region,
        premises,
    };
    calculate(&interpreter, certificate, &draft.context, budget)?;
    super::vieta::discharge(
        &interpreter,
        &draft.context,
        certificate,
        &mut inherited,
        budget,
    )?;
    Ok(inherited)
}
