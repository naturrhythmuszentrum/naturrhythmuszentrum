// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical parameter interpretation of total polynomial guard expressions.

use super::polynomial::{Polynomial, Term};
use super::scalar::{Ground, Scalar, rational_copy};
use super::storage::{AT, domain, invalid, reserve, vector};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, LogicalId, State};
use crate::model::Operator;
use crate::solving::{
    CoefficientDomainData, CoefficientValueData, CoordinateSpaceData, SolveDraft,
};

/// Interpret declarations by full header and ordered keys, never their display names.
struct Parameters<'a> {
    /// Read-only graph containing the admitted guard expression.
    state: &'a State,
    /// Exact fraction domain whose ordered parameters are interpreted.
    source: &'a Handle,
    /// Complete declaration header and ordered parameter keys.
    product: &'a CoordinateSpaceData<Handle>,
    /// Ground field used by canonical polynomial arithmetic.
    ground: Ground,
    /// Guard region retaining its full header and binder scope.
    region: LogicalId,
}

impl Parameters<'_> {
    /// Constants and declared coordinates are total on this exact parameter frame.
    fn atom(&self, id: LogicalId, budget: &mut Budget) -> Result<Option<Polynomial>, Fault> {
        let expression = self.state.expression(id)?;
        let count = self.product.coordinates.len();
        let scalar = match (&expression.operator, &expression.attachment) {
            (Operator::Rational(value), None) => match self.ground {
                Ground::Rational => Scalar::Rational(rational_copy(value, budget)?),
                Ground::Gaussian => {
                    Scalar::Gaussian(crate::solving::numeric::Gaussian::from_parts(
                        value,
                        &crate::number::Rational::zero(),
                        budget,
                    )?)
                }
            },
            (Operator::Free(key), None) => {
                let mut powers = vector(count, budget)?;
                let mut found = false;
                for entry in &self.product.coordinates {
                    budget.work(crate::graph::size(entry.key.len(), AT)?, AT)?;
                    let matches = entry.key == *key;
                    found |= matches;
                    powers.push(u32::from(matches));
                }
                if !found {
                    return Ok(None);
                }
                let mut terms = vector(1, budget)?;
                terms.push(Term {
                    powers,
                    coefficient: Scalar::constant(self.ground, true, budget)?,
                });
                return Ok(Some(Polynomial::new(count, self.ground, terms, budget)?));
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) => {
                return self.literal(*value, *map, budget);
            }
            _ => return Ok(None),
        };
        Ok(Some(Polynomial::constant(count, scalar, budget)?))
    }

    /// Only a total polynomial literal is expanded; a parameter denominator is never erased.
    fn literal(
        &self,
        value: LogicalId,
        map: LogicalId,
        budget: &mut Budget,
    ) -> Result<Option<Polynomial>, Fault> {
        let SolveDraft::CoefficientMap(mapping) = crate::solving::graph::object(self.state, map)?
        else {
            return Err(invalid());
        };
        if !super::regional::contained(
            self.state,
            self.region,
            mapping.region.logical_id(),
            budget,
        )? || !super::nonvanishing::canonical_map(
            self.state,
            self.source,
            map,
            &mapping.region,
            budget,
        )? {
            return Ok(None);
        }
        let SolveDraft::CoefficientValue(stored) =
            crate::solving::graph::object(self.state, value)?
        else {
            return Err(invalid());
        };
        if stored.domain != *self.source {
            return Ok(None);
        }
        let CoefficientValueData::Fraction {
            numerator,
            denominator,
        } = &stored.value
        else {
            return Ok(None);
        };
        let bottom = super::read::sparse(self.state, denominator, budget)?;
        let Some(constant) = bottom.scalar(budget)? else {
            return Ok(None);
        };
        if constant.zero(budget)? {
            return Ok(None);
        }
        Ok(Some(
            super::read::sparse(self.state, numerator, budget)?.scale_down(&constant, budget)?,
        ))
    }

    /// Unsupported operations remain undecided, including partial divisions and negative powers.
    fn operation(
        &self,
        id: LogicalId,
        operands: &[Polynomial],
        budget: &mut Budget,
    ) -> Result<Option<Polynomial>, Fault> {
        let expression = self.state.expression(id)?;
        let value = match (&expression.operator, operands) {
            (Operator::Neg, [value]) => value.neg(budget)?,
            (Operator::Add, [left, right]) => left.add(right, budget)?,
            (Operator::Sub, [left, right]) => left.sub(right, budget)?,
            (Operator::Mul, [left, right]) => left.mul(right, budget)?,
            (Operator::IntegerPower(exponent), [value]) if *exponent > 0 => {
                value.pow(u32::try_from(*exponent).map_err(|_| invalid())?, budget)?
            }
            (_, []) => return self.atom(id, budget),
            _ => return Ok(None),
        };
        Ok(Some(value))
    }

    /// Iterative bounded traversal preserves strict definedness even when arithmetic cancels.
    fn expression(
        &self,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<Option<Polynomial>, Fault> {
        let mut pending = vector(1, budget)?;
        pending.push((root, false));
        let mut values = vector(1, budget)?;
        let region = self.state.region(self.region)?;
        while let Some((id, ready)) = pending.pop() {
            budget.work(1, AT)?;
            let expression = self.state.expression(id)?;
            budget.check(BudgetKind::Depth, expression.depth, AT)?;
            if (expression.header, &expression.scope, expression.space)
                != (self.product.header.logical_id(), &region.scope, None)
            {
                return Ok(None);
            }
            if !ready && !expression.operands.is_empty() {
                reserve(
                    &mut pending,
                    expression
                        .operands
                        .len()
                        .checked_add(1)
                        .ok_or_else(invalid)?,
                    budget,
                )?;
                pending.push((id, true));
                pending.extend(
                    expression
                        .operands
                        .iter()
                        .rev()
                        .map(|child| (*child, false)),
                );
            } else {
                let start = values
                    .len()
                    .checked_sub(expression.operands.len())
                    .ok_or_else(invalid)?;
                let Some(value) =
                    self.operation(id, values.get(start..).ok_or_else(invalid)?, budget)?
                else {
                    return Ok(None);
                };
                values.truncate(start);
                reserve(&mut values, 1, budget)?;
                values.push(value);
            }
        }
        if values.len() != 1 {
            return Err(invalid());
        }
        Ok(values.pop())
    }
}

/// Parse only the source fraction field's exact ordered parameter polynomial fragment.
pub(crate) fn read(
    state: &State,
    source: &Handle,
    region: LogicalId,
    expression: LogicalId,
    budget: &mut Budget,
) -> Result<Option<Polynomial>, Fault> {
    let CoefficientDomainData::Fraction { base, coordinates } = domain(state, source)? else {
        return Ok(None);
    };
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(state, coordinates.logical_id())?
    else {
        return Err(invalid());
    };
    let scope = state.region(region)?;
    if scope.header != product.header.logical_id() {
        return Ok(None);
    }
    Parameters {
        state,
        source,
        product,
        ground: super::read::ground(state, base)?,
        region,
    }
    .expression(expression, budget)
}
