// SPDX-License-Identifier: AGPL-3.0-only
//! Replay one supplied power identity, without exponent, divisor or branch search.

use super::proof::{conditions, fault, identity_map, mapping};
use super::storage::invalid;
use super::{arithmetic::Engine, model::Value, polynomial::Polynomial};
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::solving::CertificateCoefficientIdentity;
use crate::{
    budget::Budget,
    error::Fault,
    graph::{Cas, ExpressionAttachment, Handle, Vertex},
};

/// Every entry is a total polynomial in the exact original fraction domain.
fn polynomial(
    engine: &Engine<'_>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let value = engine.read(handle, budget).map_err(|error| fault(&error))?;
    let Value::Fraction(fraction) = value.value else {
        return Err(invalid());
    };
    if !fraction
        .denominator
        .equals(&fraction.denominator.one(budget)?, budget)?
    {
        return Err(invalid());
    }
    Ok(fraction.numerator)
}

/// The fact repeats the exact queried coefficient, scalar map and region.
fn conclusion(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateCoefficientIdentity,
) -> Result<(), Fault> {
    let Statement::PredicateFact { predicate, region } = &draft.conclusion else {
        return Err(invalid());
    };
    if *region != certificate.region {
        return Err(invalid());
    }
    let Vertex::Condition(condition) = cas.state.node(cas.checked(predicate)?)? else {
        return Err(invalid());
    };
    let [argument] = condition.arguments.as_slice() else {
        return Err(invalid());
    };
    let frame = cas.state.region(cas.checked(region)?)?;
    if condition.kind != crate::model::PredicateKind::NonZero
        || condition.header != frame.header
        || condition.scope != frame.scope
    {
        return Err(invalid());
    }
    let expression = cas.state.expression(*argument)?;
    let Some(ExpressionAttachment::ExactCoefficient { value, map }) = &expression.attachment else {
        return Err(invalid());
    };
    if *value != certificate.left.logical_id() || *map != certificate.map.logical_id() {
        return Err(invalid());
    }
    Ok(())
}

/// Each named factor has independent nonvanishing evidence in this exact context and region.
fn product(
    engine: &Engine<'_>,
    certificate: &CertificateCoefficientIdentity,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let right = polynomial(engine, &certificate.right, budget)?;
    let mut result = right.one(budget)?;
    for handle in &certificate.denominators {
        let factor = polynomial(engine, handle, budget)?;
        if !super::nonzero::proves(
            engine.state,
            engine.domain,
            &factor,
            &certificate.region,
            premises.iter().map(|premise| &premise.judgment.statement),
            budget,
        )? {
            return Err(invalid());
        }
        result = result.mul(&factor, budget)?;
    }
    if !result.equals(&right, budget)? {
        return Err(invalid());
    }
    Ok(right)
}

/// Only named contained zero guards can be used in the supplied ideal combination.
fn identity(
    engine: &Engine<'_>,
    certificate: &CertificateCoefficientIdentity,
    exponent: u32,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = polynomial(engine, &certificate.left, budget)?;
    let mut difference = left.pow(exponent, budget)?.sub(right, budget)?;
    for multiple in &certificate.combinations {
        if multiple.hypothesis.is_some() {
            return Err(invalid());
        }
        let relation = polynomial(engine, &multiple.relation, budget)?;
        if !super::regional::proves(
            engine.state,
            engine.domain,
            &relation,
            &certificate.region,
            true,
            budget,
        )? {
            return Err(invalid());
        }
        let cofactor = polynomial(engine, &multiple.multiplier, budget)?;
        difference = difference.sub(&cofactor.mul(&relation, budget)?, budget)?;
    }
    if !difference.zero(budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// f^k equals a product of units on this cell, so the field law implies f is nonzero there.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateCoefficientIdentity,
    exponent: u32,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if exponent == 0 {
        return Err(invalid());
    }
    conclusion(cas, draft, certificate)?;
    let inherited = conditions(cas, draft, premises, budget)?;
    let map = mapping(
        cas,
        &certificate.map,
        &draft.context,
        &certificate.region,
        budget,
    )?;
    if !map.denominators.is_empty()
        || !super::nonvanishing::canonical_map(
            &cas.state,
            &map.source,
            certificate.map.logical_id(),
            &certificate.region,
            budget,
        )?
    {
        return Err(invalid());
    }
    identity_map(cas, map, premises, budget)?;
    let engine = Engine {
        state: &cas.state,
        domain: &map.source,
    };
    let right = product(&engine, certificate, premises, budget)?;
    identity(&engine, certificate, exponent, &right, budget)?;
    Ok(inherited)
}
