// SPDX-License-Identifier: AGPL-3.0-only
//! Complete mixed cubic pieces by explicit FTA selections and exact monic division.

use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::solving::coefficients::{
    arithmetic::Engine,
    maps::canonical,
    model::{CoefficientFailure, Element},
    storage::vector,
};
use crate::solving::elementary::recipes::GeneralRecipe;
use crate::solving::radicals::{construction::Construction, roots::Values};
use crate::solving::towers::dense::Dense;

/// Only an explicit family map may use this separate discovery route.
pub(crate) fn applies(cas: &Cas, map: &Handle) -> Result<bool, Fault> {
    let crate::solving::SolveDraft::CoefficientMap(value) =
        crate::solving::graph::object(&cas.state, cas.checked(map)?)?
    else {
        return Err(invalid());
    };
    Ok(matches!(
        value.target,
        crate::solving::MapTarget::Family { .. }
    ))
}

/// Synthetic division uses no inverse in the selected quotient algebra.
fn quotient(
    cas: &Cas,
    polynomial: &Dense,
    root: &Element,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &root.domain,
    };
    let mut output = vector(polynomial.coefficients.len().saturating_sub(1), budget)?;
    let mut tail = canonical(
        &cas.state,
        polynomial.coefficients.last().ok_or_else(invalid)?,
        &root.domain,
        budget,
    )?;
    output.push(engine.copy(&tail, budget)?);
    for coefficient in polynomial
        .coefficients
        .iter()
        .rev()
        .skip(1)
        .take(polynomial.coefficients.len().saturating_sub(2))
    {
        let coefficient = canonical(&cas.state, coefficient, &root.domain, budget)?;
        tail = engine.add(&coefficient, &engine.mul(root, &tail, budget)?, budget)?;
        output.push(engine.copy(&tail, budget)?);
    }
    let constant = canonical(
        &cas.state,
        polynomial.coefficients.first().ok_or_else(invalid)?,
        &root.domain,
        budget,
    )?;
    let remainder = engine.add(&constant, &engine.mul(root, &tail, budget)?, budget)?;
    if !engine.formal_zero(&remainder, budget)? {
        return Err(invalid().into());
    }
    output.reverse();
    Dense::new(&engine, output, budget)
}

/// Every proposed root is selected in the actual parent algebra; completeness still replays multiplication.
pub(crate) fn roots(
    cas: &mut Cas,
    construction: &mut Construction,
    recipe: &GeneralRecipe,
    original_map: &Handle,
    budget: &mut Budget,
) -> Result<Values, CoefficientFailure> {
    if recipe.coefficients.coefficients.len() != 4 {
        return Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::PolynomialFragment,
        ));
    }
    let mut polynomial = monic(cas, construction, &recipe.coefficients, budget)?;
    let mut map = original_map.clone();
    let mut roots = vector(3, budget)?;
    while polynomial.coefficients.len() > 2 {
        let extension = construction.adjoin(cas, &polynomial, &map, None, budget)?;
        polynomial = quotient(cas, &polynomial, &extension.generator, budget)?;
        for root in &mut roots {
            *root = canonical(&cas.state, root, &extension.domain, budget)?;
        }
        roots.push(extension.generator);
        map = extension.map;
    }
    let engine = Engine {
        state: &cas.state,
        domain: &polynomial.domain,
    };
    let [constant, leading] = polynomial.coefficients.as_slice() else {
        return Err(invalid().into());
    };
    if !engine.formal_equal(leading, &engine.constant(true, budget)?, budget)? {
        return Err(invalid().into());
    }
    roots.push(engine.neg(constant, budget)?);
    let shift = canonical(&cas.state, &recipe.shift, &polynomial.domain, budget)?;
    for root in &mut roots {
        *root = engine.sub(root, &shift, budget)?;
    }
    for root in &roots {
        let engine = Engine {
            state: &cas.state,
            domain: &polynomial.domain,
        };
        let original = engine.add(root, &shift, budget)?;
        equation(
            cas,
            construction,
            &recipe.coefficients,
            &original,
            &map,
            budget,
        )?;
    }
    Ok(Values {
        domain: polynomial.domain,
        map,
        roots,
    })
}

/// Every nested generator contributes an explicit defining equality before later factor reduction.
fn equation(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Dense,
    selected: &Element,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), CoefficientFailure> {
    let root = construction.literal(cas, selected, map, budget)?;
    let zero = construction.expression(
        cas,
        crate::model::Operator::Rational(crate::number::Rational::zero()),
        &[],
        false,
        budget,
    )?;
    let mut left = zero.clone();
    for coefficient in source.coefficients.iter().rev() {
        let promoted = canonical(&cas.state, coefficient, &selected.domain, budget)?;
        let value = construction.literal(cas, &promoted, map, budget)?;
        let product = construction.expression(
            cas,
            crate::model::Operator::Mul,
            &[left, root.clone()],
            false,
            budget,
        )?;
        left = construction.expression(
            cas,
            crate::model::Operator::Add,
            &[product, value],
            false,
            budget,
        )?;
    }
    crate::solving::radicals::proof::identity(construction, [&left, &zero], map, budget)?;
    construction
        .steps
        .last_mut()
        .ok_or_else(invalid)?
        .conclusion = crate::proof_model::Statement::Equality {
        left,
        right: zero,
        domain: construction.region.clone(),
        domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
    };
    Ok(())
}

/// Divide by the actual leading coefficient only after its regional unit decision succeeds.
fn monic(
    cas: &Cas,
    construction: &Construction,
    source: &Dense,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    use crate::solving::coefficients::model::{EvaluationScope, UnitDecision};
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let scope = EvaluationScope {
        context: &construction.context,
        region: &construction.region,
        premises: &[],
        localization: &[],
    };
    let UnitDecision::Unit { inverse, .. } = engine.inverse(
        source.coefficients.last().ok_or_else(invalid)?,
        &scope,
        budget,
    )?
    else {
        return Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::MissingUnitProof,
        ));
    };
    source.scaled(&inverse, 0, &cas.state, budget)
}
