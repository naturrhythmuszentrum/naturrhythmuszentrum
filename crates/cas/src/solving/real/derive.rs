// SPDX-License-Identifier: AGPL-3.0-only
//! Generic affine composition and expansion of a sum ansatz derive the Real cubic invariants.

use super::construction::Frame;
use crate::budget::Budget;
use crate::graph::Cas;
use crate::solving::SolveReason;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element, EvaluationScope, UnitDecision},
    storage::{invalid, vector},
};
use crate::solving::elementary::recipes::{self, Recipe};
use crate::solving::towers::dense::Dense;

/// The complete monic original and its computed affine translation.
pub(crate) struct Translated {
    /// Normalized original equation in the original unknown.
    pub(crate) source: Dense,
    /// Computed translated monic coefficients.
    pub(crate) coefficients: Dense,
    /// Original unknown equals translated unknown minus this shift.
    pub(crate) shift: Element,
    /// Original leading scalar, independently nonzero on this stratum.
    pub(crate) scale: Element,
    /// Computed binomial right-hand side; used only for a translated quadratic.
    pub(crate) radicand: Element,
}

/// Invariants obtained from the actual depressed polynomial and expanded sum ansatz.
pub(crate) struct Cubic {
    /// Original and translated source.
    pub(crate) base: Translated,
    /// Actual linear coefficient p.
    pub(crate) linear: Element,
    /// Actual constant coefficient q.
    pub(crate) constant: Element,
    /// The product constraint derived from the mixed ansatz coefficient.
    pub(crate) product: Element,
    /// Generic quadratic translation of the two root powers; its radicand is D.
    pub(crate) resolvent: Translated,
}

/// Every exact reciprocal retains the regional unit decision made before division.
pub(crate) fn inverse(
    engine: &Engine<'_>,
    value: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    match engine.inverse(value, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => Ok(inverse),
        _ => Err(CoefficientFailure::Unsupported(
            SolveReason::MissingUnitProof,
        )),
    }
}

/// Reconstruct a generic binomial densely; no cubic or quadratic formula table is consulted.
pub(crate) fn binomial(
    engine: &Engine<'_>,
    radicand: &Element,
    degree: u32,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let mut coefficients = vector(
        usize::try_from(degree)
            .map_err(|_| invalid())?
            .saturating_add(1),
        budget,
    )?;
    coefficients.push(engine.neg(radicand, budget)?);
    for _ in 1..degree {
        coefficients.push(engine.constant(false, budget)?);
    }
    coefficients.push(engine.constant(true, budget)?);
    Dense::new(engine, coefficients, budget)
}

/// The existing degree-generic Horner composition removes the actual subleading term.
pub(crate) fn depress(
    cas: &Cas,
    frame: &Frame,
    polynomial: &Dense,
    budget: &mut Budget,
) -> Result<Translated, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &polynomial.domain,
    };
    let scope = EvaluationScope {
        context: &frame.context,
        region: &frame.region,
        premises: &[],
        localization: &[],
    };
    let scale = engine.copy(polynomial.coefficients.last().ok_or_else(invalid)?, budget)?;
    let reciprocal = inverse(&engine, &scale, &scope, budget)?;
    let source = polynomial.scaled(&reciprocal, 0, &cas.state, budget)?;
    match recipes::derive(&engine, &source, &scope, budget)? {
        Recipe::Binomial(recipe) => {
            let translated = binomial(&engine, &recipe.radicand, recipe.degree, budget)?;
            Ok(Translated {
                source,
                coefficients: translated,
                shift: recipe.shift,
                scale,
                radicand: recipe.radicand,
            })
        }
        Recipe::General(recipe) => Ok(Translated {
            source,
            coefficients: recipe.coefficients,
            shift: recipe.shift,
            scale,
            radicand: engine.constant(false, budget)?,
        }),
        _ => Err(invalid().into()),
    }
}

/// Expanding (1+X)^degree obtains the mixed coefficient of the two-variable sum ansatz.
fn mixed(
    engine: &Engine<'_>,
    degree: u32,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let mut coefficients = vector(2, budget)?;
    coefficients.push(engine.constant(true, budget)?);
    coefficients.push(engine.constant(true, budget)?);
    let sum = Dense::new(engine, coefficients, budget)?;
    let mut power = Dense::one(engine, budget)?;
    for _ in 0..degree {
        power = power.mul(&sum, engine.state, budget)?;
    }
    engine.copy(power.coefficients.get(1).ok_or_else(invalid)?, budget)
}

/// The powers have a derived sum and product; generic quadratic translation computes their discriminant.
pub(crate) fn cubic(
    cas: &Cas,
    frame: &Frame,
    base: Translated,
    budget: &mut Budget,
) -> Result<Cubic, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &base.source.domain,
    };
    let scope = EvaluationScope {
        context: &frame.context,
        region: &frame.region,
        premises: &[],
        localization: &[],
    };
    let degree = u32::try_from(
        base.coefficients
            .coefficients
            .len()
            .checked_sub(1)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    if degree != 3 {
        return Err(invalid().into());
    }
    let linear = engine.copy(
        base.coefficients.coefficients.get(1).ok_or_else(invalid)?,
        budget,
    )?;
    let constant = engine.copy(
        base.coefficients.coefficients.first().ok_or_else(invalid)?,
        budget,
    )?;
    let mixed = mixed(&engine, degree, budget)?;
    let product = engine.neg(
        &engine.mul(&linear, &inverse(&engine, &mixed, &scope, budget)?, budget)?,
        budget,
    )?;
    let mut coefficients = vector(3, budget)?;
    coefficients.push(engine.pow(&product, degree, budget)?);
    coefficients.push(engine.copy(&constant, budget)?);
    coefficients.push(engine.constant(true, budget)?);
    let powers = Dense::new(&engine, coefficients, budget)?;
    let resolvent = depress(cas, frame, &powers, budget)?;
    Ok(Cubic {
        base,
        linear,
        constant,
        product,
        resolvent,
    })
}

/// Regional zero coefficients are removed before recomposition; replay still proves the original source equality.
pub(crate) fn regional_source(
    cas: &Cas,
    frame: &Frame,
    translated: &Translated,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &translated.source.domain,
    };
    let mut coefficients = vector(translated.coefficients.coefficients.len(), budget)?;
    for coefficient in &translated.coefficients.coefficients {
        coefficients.push(if engine.zero_on(coefficient, &frame.region, budget)? {
            engine.constant(false, budget)?
        } else {
            engine.copy(coefficient, budget)?
        });
    }
    let shift = engine.neg(&translated.shift, budget)?;
    recipes::compose(&engine, &coefficients, &shift, budget)
}
