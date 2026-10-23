// SPDX-License-Identifier: AGPL-3.0-only
//! Parse the original univariate polynomial without borrowing an oracle or dropping denominators.

use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, LogicalId};
use crate::model::{Operator, PredicateKind};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::fraction::Fraction;
use crate::solving::coefficients::model::{CoefficientFailure, Element, Value};
use crate::solving::coefficients::polynomial::{Polynomial, Term};
use crate::solving::coefficients::scalar::Scalar;
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::elementary::{evidence, proof};
use crate::solving::towers::dense::Dense;
use crate::solving::{CoefficientDomainData, SolveProblemData, SolveReason};

/// Unsupported grammar is an honest missing capability, not an invalid mathematical request.
const fn unsupported() -> CoefficientFailure {
    CoefficientFailure::Unsupported(SolveReason::PolynomialFragment)
}

/// A parameter uses the coefficient field's declared coordinate order, never its display spelling.
fn parameter(
    engine: &Engine<'_>,
    key: &str,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    let CoefficientDomainData::Fraction { coordinates, .. } =
        crate::solving::coefficients::storage::domain(engine.state, engine.domain)?
    else {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::CoefficientDomainUnavailable,
        ));
    };
    let crate::solving::SolveDraft::CoordinateSpace(frame) =
        crate::solving::graph::object(engine.state, coordinates.logical_id())?
    else {
        return Err(invalid().into());
    };
    let (ground, width, _) = engine.fraction_shape()?;
    budget.work(crate::graph::size(width, super::AT)?, super::AT)?;
    let slot = frame
        .coordinates
        .iter()
        .position(|coordinate| coordinate.key == key)
        .ok_or_else(unsupported)?;
    let mut powers = vector(width, budget)?;
    powers.resize(width, 0);
    *powers.get_mut(slot).ok_or_else(invalid)? = 1;
    let mut terms = vector(1, budget)?;
    terms.push(Term {
        powers,
        coefficient: Scalar::constant(ground, true, budget)?,
    });
    let numerator = Polynomial::new(width, ground, terms, budget)?;
    let denominator = numerator.one(budget)?;
    Ok(engine.element(Value::Fraction(Fraction::new(
        &numerator,
        &denominator,
        budget,
    )?)))
}

/// Leaves supply the unknown polynomial or an exact constant coefficient.
fn atom(
    engine: &Engine<'_>,
    operator: &Operator,
    unknown: &str,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let mut coefficients = vector(2, budget)?;
    match operator {
        Operator::Free(key) if key == unknown => {
            coefficients.push(engine.constant(false, budget)?);
            coefficients.push(engine.constant(true, budget)?);
        }
        Operator::Free(key) => coefficients.push(parameter(engine, key, budget)?),
        Operator::Rational(value) => coefficients.push(engine.rational(value, budget)?),
        _ => return Err(unsupported()),
    }
    Dense::new(engine, coefficients, budget)
}

/// Repeated squaring keeps the full polynomial work on the request's ledger.
fn power(
    value: &Dense,
    exponent: u32,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    budget.check(BudgetKind::PolynomialDegree, u64::from(exponent), super::AT)?;
    let mut result = Dense::one(engine, budget)?;
    let mut factor = value.copy(engine.state, budget)?;
    let mut remaining = exponent;
    while remaining > 0 {
        if remaining & 1 == 1 {
            result = result.mul(&factor, engine.state, budget)?;
        }
        remaining >>= 1;
        if remaining > 0 {
            factor = factor.mul(&factor, engine.state, budget)?;
        }
    }
    Ok(result)
}

/// Strict division transports only constant coefficients with checked regional nonvanishing.
pub(crate) fn operation(
    engine: &Engine<'_>,
    operator: &Operator,
    operands: &[Dense],
    unknown: &str,
    region: &crate::graph::Handle,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    match (operator, operands) {
        (Operator::Add, [left, right]) => left.sum(right, false, engine.state, budget),
        (Operator::Sub, [left, right]) => left.sum(right, true, engine.state, budget),
        (Operator::Mul, [left, right]) => left.mul(right, engine.state, budget),
        (Operator::Div, [left, right]) => {
            let [denominator] = right.coefficients.as_slice() else {
                return Err(unsupported());
            };
            let inverse = crate::solving::coefficients::transport::reciprocal(
                engine,
                denominator,
                region,
                budget,
            )?;
            left.scaled(&inverse, 0, engine.state, budget)
        }
        (Operator::Neg, [value]) => {
            let minus = engine.neg(&engine.constant(true, budget)?, budget)?;
            value.scaled(&minus, 0, engine.state, budget)
        }
        (Operator::IntegerPower(exponent), [value]) if *exponent > 0 => power(
            value,
            u32::try_from(*exponent).map_err(|_| unsupported())?,
            engine,
            budget,
        ),
        (_, []) => atom(engine, operator, unknown, budget),
        _ => Err(unsupported()),
    }
}

/// A ground literal retains its declared algebra and canonical scalar embedding on the original region.
fn literal(
    cas: &Cas,
    engine: &Engine<'_>,
    id: LogicalId,
    region: &crate::graph::Handle,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    use crate::graph::ExpressionAttachment;
    use crate::solving::{MapTarget, SolveDraft};
    let expression = cas.state.expression(id)?;
    let Some(ExpressionAttachment::ExactCoefficient { value, map }) = &expression.attachment else {
        return Err(unsupported());
    };
    let SolveDraft::CoefficientMap(mapping) = crate::solving::graph::object(&cas.state, *map)?
    else {
        return Err(invalid().into());
    };
    let MapTarget::ScalarAmbient {
        header,
        field: crate::model::Field::Complex,
    } = &mapping.target
    else {
        return Err(unsupported());
    };
    if mapping.source != *engine.domain
        || mapping.region != *region
        || header.logical_id() != expression.header
    {
        return Err(unsupported());
    }
    crate::solving::coefficients::proof::ground_map(cas, mapping, budget)?;
    let handle = cas.handle(*value);
    crate::solving::coefficients::proof::defined(cas, &handle, region, &[], budget)?;
    let mut coefficients = vector(1, budget)?;
    coefficients.push(engine.read(&handle, budget)?);
    Dense::new(engine, coefficients, budget)
}

/// Visit only the original expression closure using a bounded explicit postfix stack.
pub(crate) fn expression(
    cas: &Cas,
    engine: &Engine<'_>,
    root: LogicalId,
    unknown: &str,
    region: &crate::graph::Handle,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let mut pending = vector(1, budget)?;
    pending.push((root, false));
    let mut values = vector(1, budget)?;
    while let Some((id, ready)) = pending.pop() {
        budget.work(1, super::AT)?;
        if crate::solving::family::local(cas, id)? {
            values.push(crate::solving::family::polynomial::read(
                cas, engine, id, unknown, region, budget,
            )?);
            continue;
        }
        let current = cas.state.expression(id)?;
        budget.check(BudgetKind::Depth, current.depth, super::AT)?;
        if !ready && !current.operands.is_empty() {
            reserve(
                &mut pending,
                current.operands.len().checked_add(1).ok_or_else(invalid)?,
                budget,
            )?;
            pending.push((id, true));
            pending.extend(current.operands.iter().rev().map(|child| (*child, false)));
        } else {
            let start = values
                .len()
                .checked_sub(current.operands.len())
                .ok_or_else(invalid)?;
            let value =
                if current.operator == Operator::ExactCoefficient && current.operands.is_empty() {
                    literal(cas, engine, id, region, budget)?
                } else {
                    operation(
                        engine,
                        &current.operator,
                        values.get(start..).ok_or_else(invalid)?,
                        unknown,
                        region,
                        budget,
                    )?
                };
            values.truncate(start);
            reserve(&mut values, 1, budget)?;
            values.push(value);
        }
    }
    if values.len() != 1 {
        return Err(invalid().into());
    }
    values.pop().ok_or_else(|| invalid().into())
}

/// Preserve the original equation and decline systems or unsupported strict-domain fragments.
pub(crate) fn polynomial(
    cas: &Cas,
    problem: &SolveProblemData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    let coordinates = proof::coordinates(cas, &problem.unknowns)?;
    let [unknown] = coordinates.coordinates.as_slice() else {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::GeneralNonlinearSystem,
        ));
    };
    let (PredicateKind::Equal, [left, right]) =
        evidence::predicate(cas, problem.predicate.logical_id())?
    else {
        return Err(unsupported());
    };
    let engine = Engine {
        state: &cas.state,
        domain: &problem.coefficient_domain,
    };
    let first = expression(cas, &engine, *left, &unknown.key, &problem.region, budget)?;
    let second = expression(cas, &engine, *right, &unknown.key, &problem.region, budget)?;
    first.sum(&second, true, &cas.state, budget)
}
