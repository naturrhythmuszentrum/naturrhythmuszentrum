// SPDX-License-Identifier: AGPL-3.0-only
//! Exact square-free reduction and signed Euclidean identities for the Sturm theorem.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Field;
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::{arithmetic::Engine, model::CoefficientFailure, proof, storage};
use crate::solving::elementary::proof::premise;
use crate::solving::towers::{check_split::derivative, dense::Dense};
use crate::solving::{CertificateRealRootCount, SolveStatement};

/// Preserve resource failures while refusing unsupported coefficient arithmetic.
pub(super) fn arithmetic<T>(result: Result<T, CoefficientFailure>) -> Result<T, Fault> {
    result.map_err(|error| proof::fault(&error))
}

/// A count chain uses the relation's actual Real polynomial frame and defined coefficient algebra.
fn read(
    cas: &Cas,
    handle: &Handle,
    certificate: &CertificateRealRootCount,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    if storage::polynomial(&cas.state, handle)?.variables != [Field::Real] {
        return Err(invalid());
    }
    proof::defined(cas, handle, &certificate.region, premises, budget)?;
    arithmetic(Dense::read(&cas.state, handle, budget))
}

/// Compare complete polynomials only on the original coefficient stratum.
fn equal(
    cas: &Cas,
    left: &Dense,
    right: &Dense,
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !arithmetic(left.equals_on(right, &cas.state, region, budget))? {
        return Err(invalid());
    }
    Ok(())
}

/// Divisibilities and a Bezout identity identify the actual monic gcd without a trusted gcd oracle.
fn support(
    cas: &Cas,
    original: &Handle,
    certificate: &CertificateRealRootCount,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    let source = read(cas, original, certificate, premises, budget)?;
    let differentiated = read(cas, &certificate.derivative, certificate, premises, budget)?;
    equal(
        cas,
        &differentiated,
        &derivative(cas, &source, budget)?,
        &certificate.region,
        budget,
    )?;
    let divisor = read(cas, &certificate.gcd, certificate, premises, budget)?;
    let support = read(cas, &certificate.square_free, certificate, premises, budget)?;
    let first = read(
        cas,
        certificate.quotients.first().ok_or_else(invalid)?,
        certificate,
        premises,
        budget,
    )?;
    let second = read(
        cas,
        certificate.quotients.get(1).ok_or_else(invalid)?,
        certificate,
        premises,
        budget,
    )?;
    let third = read(
        cas,
        certificate.quotients.get(2).ok_or_else(invalid)?,
        certificate,
        premises,
        budget,
    )?;
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let leading = divisor.coefficients.last().ok_or_else(invalid)?;
    let one = arithmetic(engine.constant(true, budget))?;
    if !arithmetic(engine.equal_on(leading, &one, &certificate.region, budget))? {
        return Err(invalid());
    }
    equal(
        cas,
        &arithmetic(divisor.mul(&support, &cas.state, budget))?,
        &source,
        &certificate.region,
        budget,
    )?;
    equal(
        cas,
        &arithmetic(divisor.mul(&first, &cas.state, budget))?,
        &differentiated,
        &certificate.region,
        budget,
    )?;
    let combination = arithmetic(source.mul(&second, &cas.state, budget))?;
    let other = arithmetic(differentiated.mul(&third, &cas.state, budget))?;
    equal(
        cas,
        &arithmetic(combination.sum(&other, false, &cas.state, budget))?,
        &divisor,
        &certificate.region,
        budget,
    )?;
    Ok(support)
}

/// Each chain polynomial has a nonvanishing leading coefficient on this exact stratum.
fn unit(
    cas: &Cas,
    handle: &Handle,
    slot: u32,
    certificate: &CertificateRealRootCount,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let polynomial = storage::polynomial(&cas.state, handle)?;
    let leading = &polynomial.terms.first().ok_or_else(invalid)?.coefficient;
    let Statement::Solve(statement) = &premise(premises, slot)?.judgment.statement else {
        return Err(invalid());
    };
    if !matches!(statement.as_ref(), SolveStatement::Unit(value)
        if value.value == *leading && value.domain == polynomial.domain && value.region == certificate.region)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Strict degree drop is checked after every signed remainder; terminal constant gives square-freeness.
pub(super) fn check(
    cas: &Cas,
    original: &Handle,
    certificate: &CertificateRealRootCount,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Vec<Dense>, Fault> {
    let length = certificate.chain.len();
    if length < 2
        || certificate.units.len() != length
        || certificate.quotients.len() != length + 2
        || !certificate.identities.is_empty()
        || !certificate.rescalings.is_empty() && certificate.rescalings.len() != length - 2
    {
        return Err(invalid());
    }
    let support = support(cas, original, certificate, premises, budget)?;
    let mut chain = storage::vector(length, budget)?;
    for (handle, slot) in certificate.chain.iter().zip(&certificate.units) {
        budget.solving(BudgetKind::IsolationSteps, 1, AT)?;
        unit(cas, handle, *slot, certificate, premises, budget)?;
        chain.push(read(cas, handle, certificate, premises, budget)?);
    }
    equal(
        cas,
        chain.first().ok_or_else(invalid)?,
        &support,
        &certificate.region,
        budget,
    )?;
    equal(
        cas,
        chain.get(1).ok_or_else(invalid)?,
        &derivative(cas, &support, budget)?,
        &certificate.region,
        budget,
    )?;
    if chain.last().ok_or_else(invalid)?.coefficients.len() != 1 {
        return Err(invalid());
    }
    for index in 1..length {
        division(cas, certificate, &chain, index, premises, budget)?;
    }
    Ok(chain)
}

/// The certificate stores f'/g, Bezout multipliers, then every Euclidean quotient including the last.
fn division(
    cas: &Cas,
    certificate: &CertificateRealRootCount,
    chain: &[Dense],
    index: usize,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.solving(BudgetKind::IsolationSteps, 1, AT)?;
    let previous = chain.get(index - 1).ok_or_else(invalid)?;
    let divisor = chain.get(index).ok_or_else(invalid)?;
    if previous.coefficients.len() <= divisor.coefficients.len() {
        return Err(invalid());
    }
    let quotient = read(
        cas,
        certificate.quotients.get(index + 2).ok_or_else(invalid)?,
        certificate,
        premises,
        budget,
    )?;
    let product = arithmetic(divisor.mul(&quotient, &cas.state, budget))?;
    let difference = arithmetic(product.sum(previous, true, &cas.state, budget))?;
    if let Some(remainder) = chain.get(index + 1) {
        let actual = if let Some(scale) = certificate.rescalings.get(index - 1) {
            proof::defined(cas, scale, &certificate.region, premises, budget)?;
            let engine = Engine {
                state: &cas.state,
                domain: &difference.domain,
            };
            let value = arithmetic(engine.read(scale, budget))?;
            arithmetic(difference.scaled(&value, 0, &cas.state, budget))?
        } else {
            difference
        };
        equal(cas, &actual, remainder, &certificate.region, budget)
    } else {
        let engine = Engine {
            state: &cas.state,
            domain: &difference.domain,
        };
        let zero = arithmetic(Dense::zero(&engine, budget))?;
        equal(cas, &difference, &zero, &certificate.region, budget)
    }
}
