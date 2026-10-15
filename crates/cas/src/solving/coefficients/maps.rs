// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical constant inclusions and injective independent-variable renamings.

use super::arithmetic::Engine;
use super::fraction::Fraction;
use super::model::{CoefficientFailure, Element, Value};
use super::polynomial::Polynomial;
use super::scalar::{Ground, Scalar};
use super::storage::{AT, begin, domain, invalid, vector};
use crate::budget::Budget;
use crate::graph::{Handle, State};
use crate::solving::{CoefficientDomainData, CoefficientMapData, MapMode};

/// Extend a ground scalar to the target ground field using the canonical choice of i.
pub(crate) fn ground_image(
    value: &Scalar,
    target: Ground,
    budget: &mut Budget,
) -> Result<Scalar, crate::error::Fault> {
    if value.ground() == target {
        return value.copy(budget);
    }
    if let Scalar::Rational(real) = value
        && target == Ground::Gaussian
    {
        let zero = crate::number::Rational::zero();
        return Ok(Scalar::Gaussian(
            crate::solving::numeric::Gaussian::from_parts(real, &zero, budget)?,
        ));
    }
    Err(invalid())
}

/// Canonical constants into a ground or rational-function field have no image vector.
pub(crate) fn constant(
    state: &State,
    element: &Element,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    let Value::Scalar(value) = &element.value else {
        return Err(invalid().into());
    };
    let engine = Engine {
        state,
        domain: target,
    };
    let mapped = match domain(state, target)? {
        CoefficientDomainData::Rational | CoefficientDomainData::Gaussian => Value::Scalar(
            ground_image(value, super::read::ground(state, target)?, budget)?,
        ),
        CoefficientDomainData::Fraction { .. } => {
            let (ground, variables, _) = engine.fraction_shape()?;
            let numerator =
                Polynomial::constant(variables, ground_image(value, ground, budget)?, budget)?;
            let denominator = numerator.one(budget)?;
            Value::Fraction(Fraction {
                numerator,
                denominator,
            })
        }
        CoefficientDomainData::Quotient { .. } => return Err(invalid().into()),
    };
    Ok(engine.element(mapped))
}

/// A renaming image is exactly one target indeterminate, with coefficient and denominator one.
fn variable(value: &Fraction, budget: &mut Budget) -> Result<usize, CoefficientFailure> {
    let [term] = value.numerator.terms.as_slice() else {
        return Err(invalid().into());
    };
    let unit = Scalar::constant(value.numerator.ground, true, budget)?;
    if !term.coefficient.equals(&unit, budget)?
        || !value
            .denominator
            .equals(&value.numerator.one(budget)?, budget)?
    {
        return Err(invalid().into());
    }
    let mut result = None;
    for (index, exponent) in term.powers.iter().enumerate() {
        budget.work(1, AT)?;
        if *exponent == 1 && result.is_none() {
            result = Some(index);
        } else if *exponent != 0 {
            return Err(invalid().into());
        }
    }
    result.ok_or_else(|| invalid().into())
}

/// Reindex all formal exponents, retaining the target's complete variable frame.
fn rename(
    value: &Polynomial,
    positions: &[usize],
    target: Ground,
    width: usize,
    budget: &mut Budget,
) -> Result<Polynomial, CoefficientFailure> {
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        let mut powers = vector(width, budget)?;
        powers.resize(width, 0);
        for (exponent, position) in term.powers.iter().zip(positions) {
            *powers.get_mut(*position).ok_or_else(invalid)? = *exponent;
        }
        terms.push(super::polynomial::Term {
            powers,
            coefficient: ground_image(&term.coefficient, target, budget)?,
        });
    }
    Ok(Polynomial::new(width, target, terms, budget)?)
}

/// Distinct independent-variable images prove injectivity by formal polynomial substitution.
pub(crate) fn inclusion(
    state: &State,
    element: &Element,
    map: &CoefficientMapData<Handle>,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    if map.mode != MapMode::Embedding {
        return Err(invalid().into());
    }
    let Value::Fraction(value) = &element.value else {
        return Err(invalid().into());
    };
    if map.images.len() != value.numerator.variables {
        return Err(invalid().into());
    }
    let engine = Engine {
        state,
        domain: target,
    };
    let (ground, width, _) = engine.fraction_shape()?;
    let mut positions = vector(map.images.len(), budget)?;
    for handle in &map.images {
        let image = engine.read(handle, budget)?;
        let Value::Fraction(image) = image.value else {
            return Err(invalid().into());
        };
        let position = variable(&image, budget)?;
        budget.work(crate::graph::size(positions.len(), AT)?, AT)?;
        if positions.contains(&position) {
            return Err(invalid().into());
        }
        positions.push(position);
    }
    let numerator = rename(&value.numerator, &positions, ground, width, budget)?;
    let denominator = rename(&value.denominator, &positions, ground, width, budget)?;
    Ok(engine.element(Value::Fraction(Fraction::new(
        &numerator,
        &denominator,
        budget,
    )?)))
}

/// Follow the actual parent chain, sharing its graph identities and embedding from the bottom up.
pub(crate) fn canonical(
    state: &State,
    element: &Element,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    let mut chain = vector(0, budget)?;
    let mut current = target;
    while *current != element.domain {
        let CoefficientDomainData::Quotient { level } = domain(state, current)? else {
            break;
        };
        let crate::solving::SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(state, level.logical_id())?
        else {
            return Err(invalid().into());
        };
        super::storage::reserve(&mut chain, 1, budget)?;
        chain.push(current);
        budget.check(
            crate::budget::BudgetKind::Depth,
            crate::graph::size(chain.len(), AT)?,
            AT,
        )?;
        current = &extension.parent;
    }
    let mut result = if *current == element.domain {
        Engine {
            state,
            domain: current,
        }
        .copy(element, budget)?
    } else {
        constant(state, element, current, budget)?
    };
    for child in chain.into_iter().rev() {
        result = Engine {
            state,
            domain: child,
        }
        .include_parent(&result, budget)?;
    }
    Ok(result)
}

/// Horner evaluation of a parent-coefficient polynomial through canonical inclusion.
fn horner(
    state: &State,
    polynomial: &crate::solving::towers::dense::Dense,
    image: &Element,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let engine = Engine {
        state,
        domain: target,
    };
    let mut result = engine.constant(false, budget)?;
    for coefficient in polynomial.coefficients.iter().rev() {
        result = engine.add(
            &engine.mul(&result, image, budget)?,
            &canonical(state, coefficient, target, budget)?,
            budget,
        )?;
    }
    Ok(result)
}

/// Quotient specialization verifies the defining relation before reducing any projected value.
pub(crate) fn quotient(
    state: &State,
    element: &Element,
    map: &CoefficientMapData<Handle>,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    if map.mode != MapMode::Specialization {
        return Err(invalid().into());
    }
    let [image] = map.images.as_slice() else {
        return Err(invalid().into());
    };
    let source = Engine {
        state,
        domain: &element.domain,
    };
    let (_, extension) = crate::solving::towers::arithmetic::level(&source)?;
    let destination = Engine {
        state,
        domain: target,
    };
    let image = destination.read(image, budget)?;
    let modulus = crate::solving::towers::dense::Dense::read(state, &extension.polynomial, budget)?;
    if !destination.formal_zero(&horner(state, &modulus, &image, target, budget)?, budget)? {
        return Err(invalid().into());
    }
    horner(
        state,
        &crate::solving::towers::arithmetic::dense(&source, element, budget)?,
        &image,
        target,
        budget,
    )
}

/// Evaluate a formal polynomial into an admitted target algebra without asserting injectivity.
pub(crate) fn evaluate(
    state: &State,
    polynomial: &Polynomial,
    base: &Handle,
    target: &Handle,
    images: &[Element],
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    begin(budget)?;
    if polynomial.variables != images.len()
        || polynomial.ground != super::read::ground(state, base)?
    {
        return Err(invalid().into());
    }
    let engine = Engine {
        state,
        domain: target,
    };
    let mut result = engine.constant(false, budget)?;
    for term in &polynomial.terms {
        let ground = Element {
            domain: base.clone(),
            value: Value::Scalar(term.coefficient.copy(budget)?),
            origin: None,
        };
        let mut product = canonical(state, &ground, target, budget)?;
        for (image, exponent) in images.iter().zip(&term.powers) {
            product = engine.mul(&product, &engine.pow(image, *exponent, budget)?, budget)?;
        }
        result = engine.add(&result, &product, budget)?;
    }
    Ok(result)
}
