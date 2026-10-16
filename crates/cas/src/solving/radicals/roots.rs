// SPDX-License-Identifier: AGPL-3.0-only
//! Generic binomial roots from a computed primitive unity and one selected radical.

use super::construction::Construction;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::maps::canonical;
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, UnitDecision,
};
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::elementary::recipes::BinomialRecipe;
use crate::solving::towers::{cyclotomic, dense::Dense};
use crate::solving::{
    BinomialMode, CertificateBinomialCoverage, PrimePower, SolveCertificate, SolveReason,
    SolveSchema, SolveStatement, StatementRootOrder,
};

/// Exact candidate values share one final coefficient algebra and scalar interpretation.
#[derive(Debug)]
pub(crate) struct Values {
    /// Exact coefficient algebra shared by the returned values.
    pub(crate) domain: Handle,
    /// Explicit scalar interpretation with its selected-root dependencies.
    pub(crate) map: Handle,
    /// All computed candidate roots, retaining repeated factor occurrences.
    pub(crate) roots: Vec<Element>,
}

/// Preserve component and specialization boundaries rather than treating a quotient as a field.
pub(crate) fn inverse(
    engine: &Engine<'_>,
    value: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    match engine.inverse(value, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => Ok(inverse),
        UnitDecision::Zero(_) => Err(CoefficientFailure::Unsupported(
            SolveReason::MissingUnitProof,
        )),
        UnitDecision::Split(_) => Err(CoefficientFailure::Unsupported(
            SolveReason::ComponentChoicePending,
        )),
        UnitDecision::Undetermined(_) => Err(CoefficientFailure::Unsupported(
            SolveReason::SpecializationGuardPending,
        )),
    }
}

/// Integer trial division is bounded by the admitted order and reconstructs every prime power.
fn factors(order: u32, budget: &mut Budget) -> Result<Vec<PrimePower>, Fault> {
    let mut result = vector(0, budget)?;
    let mut remainder = order;
    let mut divisor = 2_u32;
    while divisor <= remainder {
        budget.work(1, super::AT)?;
        let mut exponent = 0;
        while remainder.is_multiple_of(divisor) {
            budget.work(1, super::AT)?;
            remainder /= divisor;
            exponent += 1;
        }
        if exponent > 0 {
            crate::solving::coefficients::storage::reserve(&mut result, 1, budget)?;
            result.push(PrimePower {
                prime: divisor,
                exponent,
            });
        }
        divisor = divisor.checked_add(1).ok_or_else(invalid)?;
    }
    Ok(result)
}

/// Construct a primitive unity; linear cyclotomic polynomials need no extra tower level.
pub(crate) fn unity(
    cas: &mut Cas,
    construction: &mut Construction,
    domain: &Handle,
    map: &Handle,
    degree: u32,
    budget: &mut Budget,
) -> Result<Values, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain,
    };
    let polynomial = cyclotomic::polynomial(&engine, degree, budget)?;
    if let [constant, _leading] = polynomial.coefficients.as_slice() {
        let mut roots = vector(1, budget)?;
        roots.push(engine.neg(constant, budget)?);
        return Ok(Values {
            domain: domain.clone(),
            map: map.clone(),
            roots,
        });
    }
    let extension = construction.adjoin(cas, &polynomial, map, Some(degree), budget)?;
    construction.push(
        SolveSchema::BinomialCoverage,
        SolveStatement::RootOrder(StatementRootOrder {
            value: extension.root,
            order: degree,
            region: construction.region.clone(),
        }),
        SolveCertificate::BinomialCoverage(CertificateBinomialCoverage {
            mode: BinomialMode::Order,
            order: degree,
            unity: Some(extension.selection),
            selection: None,
            factors: factors(degree, budget)?,
            exclusions: Vec::new(),
            product: None,
        }),
        budget,
    )?;
    let mut roots = vector(1, budget)?;
    roots.push(extension.generator);
    Ok(Values {
        domain: extension.domain,
        map: extension.map,
        roots,
    })
}

/// Build the exact binomial relation in the current coefficient algebra.
pub(crate) fn polynomial(
    engine: &Engine<'_>,
    radicand: &Element,
    degree: u32,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    if degree == 0 {
        return Err(invalid().into());
    }
    budget.check(
        crate::budget::BudgetKind::PolynomialDegree,
        u64::from(degree),
        super::AT,
    )?;
    let count = usize::try_from(degree)
        .map_err(|_| invalid())?
        .checked_add(1)
        .ok_or_else(invalid)?;
    let mut coefficients = vector(count, budget)?;
    coefficients.push(engine.neg(radicand, budget)?);
    for _ in 1..degree {
        coefficients.push(engine.constant(false, budget)?);
    }
    coefficients.push(engine.constant(true, budget)?);
    Dense::new(engine, coefficients, budget)
}

/// A zero radicand has a single value, retained with full factor multiplicity by the caller.
fn zero(
    cas: &Cas,
    recipe: &BinomialRecipe,
    map: &Handle,
    budget: &mut Budget,
) -> Result<Values, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &recipe.shift.domain,
    };
    let mut roots = vector(
        usize::try_from(recipe.degree).map_err(|_| invalid())?,
        budget,
    )?;
    for _ in 0..recipe.degree {
        roots.push(engine.neg(&recipe.shift, budget)?);
    }
    Ok(Values {
        domain: recipe.shift.domain.clone(),
        map: map.clone(),
        roots,
    })
}

/// Cyclic powers are generated in increasing exponent order and every original translation is retained.
pub(crate) fn binomial(
    cas: &mut Cas,
    construction: &mut Construction,
    recipe: &BinomialRecipe,
    map: &Handle,
    budget: &mut Budget,
) -> Result<Values, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &recipe.radicand.domain,
    };
    if engine.formal_zero(&recipe.radicand, budget)? {
        return zero(cas, recipe, map, budget);
    }
    let primitive = unity(
        cas,
        construction,
        &recipe.radicand.domain,
        map,
        recipe.degree,
        budget,
    )?;
    let engine = Engine {
        state: &cas.state,
        domain: &primitive.domain,
    };
    let radicand = canonical(&cas.state, &recipe.radicand, &primitive.domain, budget)?;
    let defining = polynomial(&engine, &radicand, recipe.degree, budget)?;
    let extension = construction.adjoin(cas, &defining, &primitive.map, None, budget)?;
    let engine = Engine {
        state: &cas.state,
        domain: &extension.domain,
    };
    let unity = canonical(
        &cas.state,
        primitive.roots.first().ok_or_else(invalid)?,
        &extension.domain,
        budget,
    )?;
    let shift = canonical(&cas.state, &recipe.shift, &extension.domain, budget)?;
    let mut roots = vector(
        usize::try_from(recipe.degree).map_err(|_| invalid())?,
        budget,
    )?;
    for exponent in 0..recipe.degree {
        let rotated = engine.mul(
            &engine.pow(&unity, exponent, budget)?,
            &extension.generator,
            budget,
        )?;
        roots.push(engine.sub(&rotated, &shift, budget)?);
    }
    Ok(Values {
        domain: extension.domain,
        map: extension.map,
        roots,
    })
}
