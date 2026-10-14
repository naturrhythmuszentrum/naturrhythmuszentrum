// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded derivative production; all formulas remain untrusted until kernel replay.

use std::collections::BTreeMap;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::producer::{Plan, integer, step, term};
use crate::rule::Primitive;
use crate::scalar_model::Relation;

/// Automatic calculus location.
const AT: Location = Location::new("derivative generation");

/// Shared, context-sensitive partial derivatives with their untrusted proof positions.
#[derive(Default)]
pub(crate) struct Deriver {
    /// Steps are pruned to the final requested relation before checking.
    pub(crate) plan: Plan,
    /// Complete context, term and variable identities; syntax alone carries no assumptions.
    memo: BTreeMap<(LogicalId, LogicalId, String), (LogicalId, u32)>,
}

impl Deriver {
    /// Build a scalar derivative by an explicit postorder traversal.
    pub(crate) fn derive(
        &mut self,
        cas: &mut Cas,
        context: LogicalId,
        root: LogicalId,
        variable: &str,
        budget: &mut Budget,
    ) -> Result<(LogicalId, u32), Fault> {
        crate::derivative_check::variable(&cas.state, context, variable, budget)?;
        let mut pending = vec![(root, false)];
        while let Some((id, expanded)) = pending.pop() {
            self.charge(variable, budget)?;
            let key = (context, id, variable.to_owned());
            if self.memo.contains_key(&key) {
                continue;
            }
            let expression = cas.state.expression(id)?;
            crate::rule_check::scalar(&cas.state, context, id)?;
            let primitive = primitive(&expression.operator).map_err(|mut fault| {
                fault.location.logical_id = Some(id.serial);
                fault
            })?;
            budget.check(BudgetKind::Depth, expression.depth, AT)?;
            if !expanded {
                pending
                    .try_reserve(
                        expression
                            .operands
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?,
                    )
                    .map_err(|_| Fault::allocation(AT))?;
                pending.push((id, true));
                pending.extend(
                    expression
                        .operands
                        .iter()
                        .rev()
                        .map(|child| (*child, false)),
                );
                continue;
            }
            let children = expression.operands.clone();
            let mut derivatives = Vec::new();
            let mut premises = Vec::new();
            derivatives
                .try_reserve_exact(children.len())
                .map_err(|_| Fault::allocation(AT))?;
            premises
                .try_reserve_exact(children.len())
                .map_err(|_| Fault::allocation(AT))?;
            for child in children {
                self.charge(variable, budget)?;
                let (value, proof) = self
                    .memo
                    .get(&(context, child, variable.to_owned()))
                    .copied()
                    .ok_or_else(pending_derivative)?;
                derivatives.push(value);
                premises.push(proof);
            }
            let raw = formula(cas, id, variable, &derivatives, budget)?;
            let relation = Relation::Derivative {
                variable: variable.to_owned(),
            };
            let proposal = step(cas, primitive, relation.clone(), id, raw, premises);
            let proof = self.plan.push(proposal, budget)?;
            let result = crate::polynomial::normalize_derivative(cas, raw, budget)?;
            let proof = self.transport(cas, result, relation, proof, budget)?;
            self.memo.insert(key, (result, proof));
        }
        self.charge(variable, budget)?;
        self.memo
            .get(&(context, root, variable.to_owned()))
            .copied()
            .ok_or_else(pending_derivative)
    }

    /// Account for bounded key comparisons and retained memo/plan payload before each access.
    fn charge(&self, variable: &str, budget: &mut Budget) -> Result<(), Fault> {
        let count = size(self.memo.len(), AT)?
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?;
        let width = size(variable.len(), AT)?
            .checked_add(128)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let bytes = count
            .checked_mul(width)
            .and_then(|n| n.checked_add(size(self.plan.steps.len(), AT).ok()?.checked_mul(1024)?))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(
            count
                .checked_mul(width)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )
    }

    /// Simplify only by proposing and then transporting an exact rational identity.
    fn transport(
        &mut self,
        cas: &Cas,
        result: LogicalId,
        relation: Relation,
        proof: u32,
        budget: &mut Budget,
    ) -> Result<u32, Fault> {
        let previous = self.plan.get(proof)?;
        let input = cas.checked(&previous.left)?;
        let raw = cas.checked(&previous.right)?;
        if result == raw {
            return Ok(proof);
        }
        let equality = self.plan.push(
            step(
                cas,
                Primitive::RationalIdentity,
                Relation::EqualOnDomain,
                raw,
                result,
                Vec::new(),
            ),
            budget,
        )?;
        self.plan.push(
            step(
                cas,
                Primitive::DerivativeTransport,
                relation,
                input,
                result,
                vec![proof, equality],
            ),
            budget,
        )
    }

    /// Build every requested cell and a single row-major matrix assembly proposal.
    pub(crate) fn jacobian(
        &mut self,
        cas: &mut Cas,
        context: LogicalId,
        inputs: &[LogicalId],
        variables: &[String],
        budget: &mut Budget,
    ) -> Result<u32, Fault> {
        let entries = size(inputs.len(), AT)?
            .checked_mul(size(variables.len(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
        budget.check(BudgetKind::MatrixEntries, entries, AT)?;
        let capacity =
            usize::try_from(entries).map_err(|_| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
        let mut cells = Vec::new();
        let mut premises = Vec::new();
        cells
            .try_reserve_exact(capacity)
            .map_err(|_| Fault::allocation(AT))?;
        premises
            .try_reserve_exact(capacity)
            .map_err(|_| Fault::allocation(AT))?;
        for input in inputs {
            for variable in variables {
                let (cell, proof) = self.derive(cas, context, *input, variable, budget)?;
                cells.push(cell);
                premises.push(proof);
            }
        }
        let rows = u32::try_from(inputs.len())
            .map_err(|_| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
        let columns = u32::try_from(variables.len())
            .map_err(|_| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
        let source = matrix(cas, context, rows, 1, inputs, budget)?;
        let output = matrix(cas, context, rows, columns, &cells, budget)?;
        self.plan.push(
            step(
                cas,
                Primitive::Jacobian,
                Relation::Jacobian {
                    variables: variables.to_vec(),
                },
                source,
                output,
                premises,
            ),
            budget,
        )
    }
}

/// Missing symbolic derivative grammar is an explicit pending task.
const fn pending_derivative() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

/// Select only the admitted scalar derivative family.
const fn primitive(operator: &Operator) -> Result<Primitive, Fault> {
    match operator {
        Operator::Rational(_) | Operator::Constant(_) => Ok(Primitive::DifferentiateConstant),
        Operator::Free(_) => Ok(Primitive::DifferentiateVariable),
        Operator::Neg => Ok(Primitive::DifferentiateNeg),
        Operator::Add => Ok(Primitive::DifferentiateAdd),
        Operator::Sub => Ok(Primitive::DifferentiateSub),
        Operator::Mul => Ok(Primitive::DifferentiateProduct),
        Operator::Div => Ok(Primitive::DifferentiateQuotient),
        Operator::IntegerPower(_) => Ok(Primitive::DifferentiatePower),
        _ => Err(pending_derivative()),
    }
}

/// Generate the exact formula shape expected by the independent derivative schema.
fn formula(
    cas: &mut Cas,
    input: LogicalId,
    variable: &str,
    derivatives: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let expression = cas.state.expression(input)?;
    let operator = expression.operator.clone();
    let operands = expression.operands.clone();
    let operand = |position| {
        operands
            .get(position)
            .copied()
            .ok_or_else(pending_derivative)
    };
    let derivative = |position| {
        derivatives
            .get(position)
            .copied()
            .ok_or_else(pending_derivative)
    };
    match operator {
        Operator::Rational(_) | Operator::Constant(_) => integer(cas, input, 0, budget),
        Operator::Free(key) => integer(cas, input, i64::from(key == variable), budget),
        Operator::Neg | Operator::Add | Operator::Sub => {
            term(cas, input, operator, derivatives, budget)
        }
        Operator::Mul | Operator::Div => {
            let first = term(
                cas,
                input,
                Operator::Mul,
                &[derivative(0)?, operand(1)?],
                budget,
            )?;
            let second = term(
                cas,
                input,
                Operator::Mul,
                &[operand(0)?, derivative(1)?],
                budget,
            )?;
            if operator == Operator::Mul {
                term(cas, input, Operator::Add, &[first, second], budget)
            } else {
                let numerator = term(cas, input, Operator::Sub, &[first, second], budget)?;
                let denominator = term(
                    cas,
                    input,
                    Operator::IntegerPower(2),
                    &[operand(1)?],
                    budget,
                )?;
                term(cas, input, Operator::Div, &[numerator, denominator], budget)
            }
        }
        Operator::IntegerPower(exponent) => {
            power(cas, input, exponent, operand(0)?, derivative(0)?, budget)
        }
        _ => Err(pending_derivative()),
    }
}

/// Avoid introducing a zero power in the identity-exponent derivative.
fn power(
    cas: &mut Cas,
    input: LogicalId,
    exponent: i64,
    base: LogicalId,
    derivative: LogicalId,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    budget.check(BudgetKind::IntegerExponent, exponent.unsigned_abs(), AT)?;
    match exponent {
        0 => integer(cas, input, 0, budget),
        1 => Ok(derivative),
        _ => {
            let previous = exponent
                .checked_sub(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::IntegerExponent, AT))?;
            let coefficient = integer(cas, input, exponent, budget)?;
            let raised = term(
                cas,
                input,
                Operator::IntegerPower(previous),
                &[base],
                budget,
            )?;
            let product = term(cas, input, Operator::Mul, &[coefficient, raised], budget)?;
            term(cas, input, Operator::Mul, &[product, derivative], budget)
        }
    }
}

/// Construct a real row-major matrix after entry-count validation.
fn matrix(
    cas: &mut Cas,
    context: LogicalId,
    rows: u32,
    columns: u32,
    entries: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    cas.generated(
        context,
        Operator::Matrix { rows, columns },
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        }),
        entries,
        budget,
    )
}
