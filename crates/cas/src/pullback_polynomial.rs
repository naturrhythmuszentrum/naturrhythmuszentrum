// SPDX-License-Identifier: AGPL-3.0-only
//! Polynomial coordinate maps must be total syntax, including every strict subexpression.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::number::Rational;
use crate::pullback_geometry::{AT, map_error};

/// Reconstruct coefficients only after ruling out hidden strict-power or denominator holes.
pub(crate) fn coefficients(
    cas: &Cas,
    function: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let coordinate = &cas.state.space(cas.checked(space)?)?.coordinate;
    let root = cas.checked(function)?;
    let ordered = crate::case_syntax::order(cas, &[root], budget)?;
    for id in ordered {
        budget.work(1, AT)?;
        let value = cas.state.expression(id)?;
        match &value.operator {
            Operator::Rational(_)
            | Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul => {}
            Operator::Free(key) if key == coordinate => {}
            Operator::IntegerPower(order) if *order > 0 => {}
            Operator::Div => {
                let denominator = *value.operands.get(1).ok_or_else(map_error)?;
                let actual =
                    crate::polynomial::univariate(&cas.state, denominator, coordinate, budget)?;
                if actual.len() != 1 || actual.first().is_none_or(Rational::is_zero) {
                    return Err(map_error());
                }
            }
            _ => return Err(map_error()),
        }
    }
    crate::polynomial::univariate(&cas.state, root, coordinate, budget)
}
