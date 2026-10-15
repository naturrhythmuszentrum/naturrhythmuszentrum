// SPDX-License-Identifier: AGPL-3.0-only
//! Unsealed degree-generic translation, consumed by the later solve search scheduler.

use crate::budget::Budget;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, UnitDecision, ZeroDecision,
};
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::towers::dense::Dense;

/// The recipe classifies the actual regional degree before any division or root request.
#[derive(Debug)]
pub(crate) enum Recipe {
    /// Every unknown is free; a later identity-family certificate supplies coverage.
    Zero,
    /// A regional nonzero constant has no solutions.
    Constant(Element),
    /// Translation leaves just a leading term and constant, in arbitrary positive degree.
    Binomial(Box<BinomialRecipe>),
    /// Further generic work is needed; no completeness flag is fabricated.
    General(Box<GeneralRecipe>),
}

/// Unsealed parameters of the translated degree-generic binomial.
#[derive(Debug)]
pub(crate) struct BinomialRecipe {
    /// Positive specialized degree.
    pub(crate) degree: u32,
    /// Translation from the original variable to the depressed variable.
    pub(crate) shift: Element,
    /// Nonzero specialized leading coefficient.
    pub(crate) scale: Element,
    /// Exact right-hand side after division by the leading coefficient.
    pub(crate) radicand: Element,
}

/// A translated polynomial requiring a later generic resolvent step.
#[derive(Debug)]
pub(crate) struct GeneralRecipe {
    /// Translation whose subleading coefficient vanishes.
    pub(crate) shift: Element,
    /// Complete translated coefficient vector, without a completeness assertion.
    pub(crate) coefficients: Dense,
}

/// Regional inversion never treats a formal nonzero parameter as a proved unit.
fn inverse(
    engine: &Engine<'_>,
    value: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    match engine.inverse(value, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => Ok(inverse),
        UnitDecision::Zero(_) => Err(invalid().into()),
        UnitDecision::Split(_) | UnitDecision::Undetermined(_) => Err(
            CoefficientFailure::Unsupported(crate::solving::SolveReason::MissingUnitProof),
        ),
    }
}

/// Drop only leading coefficients that are independently zero on this exact specialization region.
fn degree(
    engine: &Engine<'_>,
    polynomial: &Dense,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<usize, CoefficientFailure> {
    let mut count = polynomial.coefficients.len();
    while let Some(value) = count
        .checked_sub(1)
        .and_then(|index| polynomial.coefficients.get(index))
    {
        match engine.zero_test(value, scope, budget)? {
            ZeroDecision::Zero(_) => count -= 1,
            ZeroDecision::Nonzero(_) => return Ok(count),
            ZeroDecision::Split(_) | ZeroDecision::Undetermined(_) => {
                return Err(CoefficientFailure::Unsupported(
                    crate::solving::SolveReason::SpecializationGuardPending,
                ));
            }
        }
    }
    Ok(0)
}

/// Horner composition with X-shift derives every translated coefficient without a stored formula.
fn compose(
    engine: &Engine<'_>,
    coefficients: &[Element],
    shift: &Element,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let mut affine = vector(2, budget)?;
    affine.push(engine.neg(shift, budget)?);
    affine.push(engine.constant(true, budget)?);
    let affine = Dense::new(engine, affine, budget)?;
    let mut result = Dense::zero(engine, budget)?;
    for coefficient in coefficients.iter().rev() {
        let mut constant = vector(1, budget)?;
        constant.push(engine.copy(coefficient, budget)?);
        let constant = Dense::new(engine, constant, budget)?;
        result = result.mul(&affine, engine.state, budget)?.sum(
            &constant,
            false,
            engine.state,
            budget,
        )?;
    }
    Ok(result)
}

/// Classify the derived translated polynomial; the quadratic succeeds because its middle term vanishes.
fn classify(
    engine: &Engine<'_>,
    translated: Dense,
    shift: Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Recipe, CoefficientFailure> {
    let degree = translated
        .coefficients
        .len()
        .checked_sub(1)
        .ok_or_else(invalid)?;
    for value in translated
        .coefficients
        .iter()
        .skip(1)
        .take(degree.saturating_sub(1))
    {
        if !matches!(
            engine.zero_test(value, scope, budget)?,
            ZeroDecision::Zero(_)
        ) {
            return Ok(Recipe::General(Box::new(GeneralRecipe {
                shift,
                coefficients: translated,
            })));
        }
    }
    let scale = engine.copy(translated.coefficients.last().ok_or_else(invalid)?, budget)?;
    let reciprocal = inverse(engine, &scale, scope, budget)?;
    let constant = translated.coefficients.first().ok_or_else(invalid)?;
    let radicand = engine.neg(&engine.mul(constant, &reciprocal, budget)?, budget)?;
    Ok(Recipe::Binomial(Box::new(BinomialRecipe {
        degree: u32::try_from(degree).map_err(|_| invalid())?,
        shift,
        scale,
        radicand,
    })))
}

/// A single generic translation cancels the subleading coefficient for every positive degree.
pub(crate) fn derive(
    engine: &Engine<'_>,
    polynomial: &Dense,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Recipe, CoefficientFailure> {
    let count = degree(engine, polynomial, scope, budget)?;
    if count == 0 {
        return Ok(Recipe::Zero);
    }
    if count == 1 {
        return Ok(Recipe::Constant(engine.copy(
            polynomial.coefficients.first().ok_or_else(invalid)?,
            budget,
        )?));
    }
    let degree = count.checked_sub(1).ok_or_else(invalid)?;
    let integer = crate::number::Rational::from_ratio(
        i64::try_from(degree).map_err(|_| invalid())?,
        1,
        budget,
    )?;
    let leading = polynomial.coefficients.get(degree).ok_or_else(invalid)?;
    let number = engine.rational(&integer, budget)?;
    let reciprocal = engine.mul(
        &inverse(engine, leading, scope, budget)?,
        &inverse(engine, &number, scope, budget)?,
        budget,
    )?;
    let shift = engine.mul(
        polynomial
            .coefficients
            .get(degree - 1)
            .ok_or_else(invalid)?,
        &reciprocal,
        budget,
    )?;
    let translated = compose(
        engine,
        polynomial.coefficients.get(..count).ok_or_else(invalid)?,
        &shift,
        budget,
    )?;
    classify(engine, translated, shift, scope, budget)
}
