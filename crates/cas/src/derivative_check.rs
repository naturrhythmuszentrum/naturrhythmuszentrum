// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar derivative and ordered Jacobian schemas, independent of a producer.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, State, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{CheckedStep, ProofStep, Relation};
use crate::rule::Primitive;
use crate::rule_check::{binary, literal, mismatch, premise, scalar, unary};

/// Derivative-checking location.
const AT: Location = Location::new("derivative schema");

/// Validate a declared real scalar variable before accepting derivative relations.
pub(crate) fn variable(
    state: &State,
    context: LogicalId,
    key: &str,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(size(state.context(context)?.header.len(), AT)?, AT)?;
    let environment = &state.context(context)?.environment;
    if environment.field != Field::Real
        || !environment.declarations.iter().any(|declaration| {
            declaration.key == key
                && declaration.sort == Sort::Value(AtomicSort::Scalar(Field::Real))
        })
    {
        return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
    }
    Ok(())
}

/// Check derivative endpoints and all ordered child premises.
pub(crate) fn check(
    cas: &Cas,
    step: &ProofStep,
    earlier: &[CheckedStep],
    context: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = cas.checked(&step.left)?;
    let right = cas.checked(&step.right)?;
    if step.primitive == Primitive::Jacobian {
        return jacobian(&cas.state, step, earlier, context, left, right, budget);
    }
    let Relation::Derivative { variable: key } = &step.relation else {
        return Err(mismatch());
    };
    variable(&cas.state, context, key, budget)?;
    scalar(&cas.state, context, left)?;
    if !cas.state.expression(left)?.scope.is_empty() {
        return Err(mismatch());
    }
    let valid = if step.primitive == Primitive::DerivativeTransport {
        if step.premises.len() != 2 {
            return Err(mismatch());
        }
        let derivative = premise(step, earlier, 0)?;
        let equality = premise(step, earlier, 1)?;
        derivative.relation == step.relation
            && equality.relation == Relation::EqualOnDomain
            && derivative.left == left
            && derivative.right == equality.left
            && equality.right == right
    } else {
        let term = cas.state.expression(left)?;
        if step.premises.len() != term.operands.len() {
            return Err(mismatch());
        }
        let mut derivatives = Vec::new();
        derivatives
            .try_reserve_exact(term.operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (position, child) in term.operands.iter().enumerate() {
            let previous = premise(step, earlier, position)?;
            if previous.relation != step.relation || previous.left != *child {
                return Err(mismatch());
            }
            derivatives.push(previous.right);
        }
        formula(
            &cas.state,
            step.primitive,
            left,
            right,
            key,
            &derivatives,
            budget,
        )?
    };
    if valid { Ok(()) } else { Err(mismatch()) }
}

/// Reconstruct the exact right-hand-side shape of one elementary derivative.
fn formula(
    state: &State,
    primitive: Primitive,
    left: LogicalId,
    right: LogicalId,
    variable: &str,
    derivatives: &[LogicalId],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let term = state.expression(left)?;
    let derivative = |position: usize| derivatives.get(position).copied().ok_or_else(mismatch);
    let operand = |position: usize| term.operands.get(position).copied().ok_or_else(mismatch);
    match (primitive, &term.operator) {
        (Primitive::DifferentiateConstant, Operator::Rational(_) | Operator::Constant(_)) => {
            literal(state, right, 0, budget)
        }
        (Primitive::DifferentiateVariable, Operator::Free(key)) => {
            literal(state, right, i64::from(key == variable), budget)
        }
        (Primitive::DifferentiateNeg, Operator::Neg) => {
            Ok(unary(state, right, &Operator::Neg)? == derivative(0)?)
        }
        (Primitive::DifferentiateAdd, Operator::Add)
        | (Primitive::DifferentiateSub, Operator::Sub) => {
            Ok(binary(state, right, &term.operator)? == (derivative(0)?, derivative(1)?))
        }
        (Primitive::DifferentiateProduct, Operator::Mul) => {
            let (a, b) = binary(state, right, &Operator::Add)?;
            Ok(
                binary(state, a, &Operator::Mul)? == (derivative(0)?, operand(1)?)
                    && binary(state, b, &Operator::Mul)? == (operand(0)?, derivative(1)?),
            )
        }
        (Primitive::DifferentiateQuotient, Operator::Div) => {
            let (numerator, denominator) = binary(state, right, &Operator::Div)?;
            let (a, b) = binary(state, numerator, &Operator::Sub)?;
            Ok(
                binary(state, a, &Operator::Mul)? == (derivative(0)?, operand(1)?)
                    && binary(state, b, &Operator::Mul)? == (operand(0)?, derivative(1)?)
                    && unary(state, denominator, &Operator::IntegerPower(2))? == operand(1)?,
            )
        }
        (Primitive::DifferentiatePower, Operator::IntegerPower(exponent)) => {
            power(state, operand(0)?, derivative(0)?, right, *exponent, budget)
        }
        _ => Err(mismatch()),
    }
}

/// The zero and one cases preserve strict domains without manufacturing a zero power.
fn power(
    state: &State,
    base: LogicalId,
    derivative: LogicalId,
    right: LogicalId,
    exponent: i64,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    match exponent {
        0 => literal(state, right, 0, budget),
        1 => Ok(right == derivative),
        _ => {
            let previous = exponent
                .checked_sub(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::DecimalExponent, AT))?;
            let (coefficient_power, actual_derivative) = binary(state, right, &Operator::Mul)?;
            let (coefficient, power) = binary(state, coefficient_power, &Operator::Mul)?;
            Ok(actual_derivative == derivative
                && literal(state, coefficient, exponent, budget)?
                && unary(state, power, &Operator::IntegerPower(previous))? == base)
        }
    }
}

/// Assemble a row-major Jacobian from one independent derivative proof per cell.
fn jacobian(
    state: &State,
    step: &ProofStep,
    earlier: &[CheckedStep],
    context: LogicalId,
    left: LogicalId,
    right: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Relation::Jacobian { variables } = &step.relation else {
        return Err(mismatch());
    };
    let input = state.expression(left)?;
    let output = state.expression(right)?;
    if state.context(context)?.environment.field != Field::Real
        || !matches!(
            input.sort,
            Sort::Value(AtomicSort::Matrix {
                field: Field::Real,
                ..
            })
        )
        || !matches!(
            output.sort,
            Sort::Value(AtomicSort::Matrix {
                field: Field::Real,
                ..
            })
        )
    {
        return Err(mismatch());
    }
    let Operator::Matrix { rows, columns: 1 } = input.operator else {
        return Err(mismatch());
    };
    let columns = u32::try_from(variables.len()).map_err(|_| mismatch())?;
    if output.operator != (Operator::Matrix { rows, columns }) {
        return Err(mismatch());
    }
    let entries = u64::from(rows)
        .checked_mul(u64::from(columns))
        .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
    budget.check(BudgetKind::MatrixEntries, entries, AT)?;
    if size(step.premises.len(), AT)? != entries {
        return Err(mismatch());
    }
    let mut unique = BTreeSet::new();
    for key in variables {
        variable(state, context, key, budget)?;
        budget.work(
            size(unique.len(), AT)?
                .checked_add(1)
                .and_then(|n| n.checked_mul(budget.limits().name_bytes))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        if !unique.insert(key) {
            return Err(mismatch());
        }
    }
    for (row, input) in input.operands.iter().enumerate() {
        for (column, key) in variables.iter().enumerate() {
            budget.work(
                size(key.len(), AT)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                AT,
            )?;
            let position = row
                .checked_mul(variables.len())
                .and_then(|n| n.checked_add(column))
                .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
            let previous = premise(step, earlier, position)?;
            if previous.relation
                != (Relation::Derivative {
                    variable: key.clone(),
                })
                || previous.left != *input
                || Some(&previous.right) != output.operands.get(position)
            {
                return Err(mismatch());
            }
        }
    }
    Ok(())
}
