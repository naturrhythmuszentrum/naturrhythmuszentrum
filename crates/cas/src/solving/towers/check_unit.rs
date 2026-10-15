// SPDX-License-Identifier: AGPL-3.0-only
//! Independent gcd divisibility, Bezout and two-sided unit replay.

use super::dense::Dense;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::{defined, fault};
use crate::solving::coefficients::storage::{AT, domain, invalid, value, vector};
use crate::solving::{CertificateExtensionUnit, CoefficientDomainData, SolveStatement};

/// Ground units use the canonical auxiliary modulus X; quotient units use their exact defining relation.
fn subject(
    cas: &Cas,
    element: &Handle,
    modulus: &Handle,
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    let stored = value(&cas.state, element)?;
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    if matches!(
        domain(&cas.state, &stored.domain)?,
        CoefficientDomainData::Quotient { .. }
    ) {
        let (_, extension) = super::arithmetic::level(&engine).map_err(|error| fault(&error))?;
        if extension.polynomial != *modulus {
            return Err(invalid());
        }
        return super::arithmetic::dense(
            &engine,
            &engine
                .read(element, budget)
                .map_err(|error| fault(&error))?,
            budget,
        )
        .map_err(|error| fault(&error));
    }
    let polynomial = Dense::read(&cas.state, modulus, budget).map_err(|error| fault(&error))?;
    if polynomial.domain != stored.domain || polynomial.coefficients.len() != 2 {
        return Err(invalid());
    }
    if !engine
        .formal_zero(polynomial.coefficients.first().ok_or_else(invalid)?, budget)
        .map_err(|error| fault(&error))?
        || !engine
            .formal_equal(
                polynomial.coefficients.get(1).ok_or_else(invalid)?,
                &engine
                    .constant(true, budget)
                    .map_err(|error| fault(&error))?,
                budget,
            )
            .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let mut coefficients = vector(1, budget)?;
    coefficients.push(
        engine
            .read(element, budget)
            .map_err(|error| fault(&error))?,
    );
    Dense::new(&engine, coefficients, budget).map_err(|error| fault(&error))
}

/// Exact divisibilities and the ordered Bezout combination independently identify the claimed monic gcd.
fn identities(
    cas: &Cas,
    certificate: &CertificateExtensionUnit,
    element: &Dense,
    modulus: &Dense,
    divisor: &Dense,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [element_quotient, modulus_quotient] = certificate.quotients.as_slice() else {
        return Err(invalid());
    };
    let [first, second] = certificate.bezout.as_slice() else {
        return Err(invalid());
    };
    let first_quotient =
        Dense::read(&cas.state, element_quotient, budget).map_err(|error| fault(&error))?;
    let second_quotient =
        Dense::read(&cas.state, modulus_quotient, budget).map_err(|error| fault(&error))?;
    if !divisor
        .mul(&first_quotient, &cas.state, budget)
        .map_err(|error| fault(&error))?
        .equals(element, &cas.state, budget)
        .map_err(|error| fault(&error))?
        || !divisor
            .mul(&second_quotient, &cas.state, budget)
            .map_err(|error| fault(&error))?
            .equals(modulus, &cas.state, budget)
            .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let first_multiplier = Dense::read(&cas.state, first, budget).map_err(|error| fault(&error))?;
    let second_multiplier =
        Dense::read(&cas.state, second, budget).map_err(|error| fault(&error))?;
    let combination = element
        .mul(&first_multiplier, &cas.state, budget)
        .map_err(|error| fault(&error))?
        .sum(
            &modulus
                .mul(&second_multiplier, &cas.state, budget)
                .map_err(|error| fault(&error))?,
            false,
            &cas.state,
            budget,
        )
        .map_err(|error| fault(&error))?;
    if !combination
        .equals(divisor, &cas.state, budget)
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let parent = Engine {
        state: &cas.state,
        domain: &divisor.domain,
    };
    if !parent
        .formal_equal(
            divisor.coefficients.last().ok_or_else(invalid)?,
            &parent
                .constant(true, budget)
                .map_err(|error| fault(&error))?,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    Ok(())
}

/// A unit conclusion independently checks both products, not just the producer's inverse vector.
fn unit(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateExtensionUnit,
    divisor: &Dense,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Unit(unit) = statement.as_ref() else {
        return Err(invalid());
    };
    let stored = value(&cas.state, &certificate.element)?;
    if (&unit.value, &unit.region, &unit.domain)
        != (&certificate.element, &certificate.region, &stored.domain)
    {
        return Err(invalid());
    }
    let parent = Engine {
        state: &cas.state,
        domain: &divisor.domain,
    };
    if !divisor
        .equals(
            &Dense::one(&parent, budget).map_err(|error| fault(&error))?,
            &cas.state,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    let element = engine
        .read(&unit.value, budget)
        .map_err(|error| fault(&error))?;
    let inverse = engine
        .read(&unit.inverse, budget)
        .map_err(|error| fault(&error))?;
    let one = engine
        .constant(true, budget)
        .map_err(|error| fault(&error))?;
    if !engine
        .formal_equal(
            &engine
                .mul(&element, &inverse, budget)
                .map_err(|error| fault(&error))?,
            &one,
            budget,
        )
        .map_err(|error| fault(&error))?
        || !engine
            .formal_equal(
                &engine
                    .mul(&inverse, &element, budget)
                    .map_err(|error| fault(&error))?,
                &one,
                budget,
            )
            .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    Ok(())
}

/// The zero branch requires the exact modulus gcd and an independently zero reduced element.
fn zero(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateExtensionUnit,
    modulus: &Dense,
    divisor: &Dense,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::CoefficientEquality(equality) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&equality.left, &equality.region) != (&certificate.element, &certificate.region)
        || !divisor
            .equals(modulus, &cas.state, budget)
            .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let map = crate::solving::coefficients::proof::mapping(
        cas,
        &equality.map,
        &draft.context,
        &equality.region,
        budget,
    )?;
    crate::solving::coefficients::proof::identity_map(cas, map, premises, budget)?;
    let engine = Engine {
        state: &cas.state,
        domain: &map.source,
    };
    for endpoint in [&equality.left, &equality.right] {
        if !engine
            .formal_zero(
                &engine
                    .read(endpoint, budget)
                    .map_err(|error| fault(&error))?,
                budget,
            )
            .map_err(|error| fault(&error))?
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Replay all untrusted gcd data before deciding which exact statement can follow.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateExtensionUnit,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let frame = cas.state.context(cas.checked(&draft.context)?)?;
    if frame.admissible != cas.checked(&certificate.region)? {
        return Err(invalid());
    }
    for handle in [&certificate.element, &certificate.modulus, &certificate.gcd]
        .into_iter()
        .chain(&certificate.quotients)
        .chain(&certificate.bezout)
    {
        defined(cas, handle, &certificate.region, premises, budget)?;
    }
    let element = subject(cas, &certificate.element, &certificate.modulus, budget)?;
    let modulus =
        Dense::read(&cas.state, &certificate.modulus, budget).map_err(|error| fault(&error))?;
    let divisor =
        Dense::read(&cas.state, &certificate.gcd, budget).map_err(|error| fault(&error))?;
    identities(cas, certificate, &element, &modulus, &divisor, budget)?;
    match &draft.conclusion {
        Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::Unit(_)) => {
            let SolveStatement::Unit(claim) = statement.as_ref() else {
                return Err(invalid());
            };
            defined(cas, &claim.inverse, &certificate.region, premises, budget)?;
            unit(cas, draft, certificate, &divisor, budget)
        }
        _ => zero(
            cas,
            draft,
            certificate,
            &modulus,
            &divisor,
            premises,
            budget,
        ),
    }
}
