// SPDX-License-Identifier: AGPL-3.0-only
//! Only coefficient entries selected from the sealed native Jacobian become executable.

use super::{AT, HeapError, ReciprocalDerivativeError as Error};
use nrz_bindings::{
    Budget,
    readings::{
        ReciprocalJacobian,
        algebra::{
            Cas, Handle,
            limits::BudgetKind,
            source::{SyntaxNode, SyntaxRole},
            syntax::Operator,
        },
    },
};

/// Closed scalar arithmetic; indices always refer to earlier admitted steps.
#[derive(Debug)]
pub(crate) enum Step {
    /// Exact rational spelling, enclosed before reading any numerical argument.
    Constant(String),
    /// Original formal physical input position.
    Input(usize),
    /// Exact sign change.
    Neg(usize),
    /// Ordered addition.
    Add(usize, usize),
    /// Ordered subtraction.
    Sub(usize, usize),
    /// Ordered multiplication.
    Mul(usize, usize),
    /// Division with outward denominator separation.
    Div(usize, usize),
    /// Integer power with bounded binary evaluation.
    Pow(usize, i64),
}

/// One shared DAG evaluates all operator partials without discarding original evidence.
#[derive(Debug)]
pub(crate) struct Program {
    /// Child-before-parent closed scalar operations.
    pub(crate) steps: Vec<Step>,
    /// Operator coefficient partials in logical column order.
    pub(crate) outputs: Vec<usize>,
    /// Complete program, memo and exact spelling allowance.
    pub(crate) bytes: u64,
}

/// Common memoization preserves shared subexpressions within this exact matrix.
struct Builder<'a> {
    /// Actual graph that owns the independently checked derivative.
    graph: &'a Cas,
    /// Original argument declarations and whole-family proof.
    exact: &'a ReciprocalJacobian,
    /// One native expression per compiled step.
    memo: Vec<Handle>,
    /// Complete bounded program under construction.
    program: Program,
}

/// Homogeneous offsets and the final affine row remain covered by the original native witness.
pub(crate) fn compile(
    graph: &Cas,
    exact: &ReciprocalJacobian,
    budget: &mut Budget,
) -> Result<Program, Error> {
    let matrix = graph.describe_syntax(exact.native().matrix(), budget)?;
    let SyntaxNode::Expression(value) = matrix.node else {
        return Err(Error::Unsupported);
    };
    let Operator::Matrix { rows, columns } = value.operator else {
        return Err(Error::Unsupported);
    };
    let [operator_rows, operator_columns] = exact.definition().shape();
    let homogeneous_rows = operator_rows
        .checked_add(1)
        .ok_or(HeapError::ExtentOverflow)?;
    let expected_rows = homogeneous_rows
        .checked_mul(
            operator_columns
                .checked_add(1)
                .ok_or(HeapError::ExtentOverflow)?,
        )
        .ok_or(HeapError::ExtentOverflow)?;
    if rows != expected_rows || usize::try_from(columns).ok() != Some(exact.key().positions().len())
    {
        return Err(Error::Identity);
    }
    let entries = matrix
        .dependencies
        .into_iter()
        .map(|dependency| dependency.target)
        .collect::<Vec<_>>();
    let mut builder = Builder {
        graph,
        exact,
        memo: Vec::new(),
        program: Program {
            steps: Vec::new(),
            outputs: Vec::new(),
            bytes: 1024,
        },
    };
    for source in 0..columns {
        for column in 0..operator_columns {
            for row in 0..operator_rows {
                let output = u64::from(column) * u64::from(homogeneous_rows) + u64::from(row);
                let index = output
                    .checked_mul(u64::from(columns))
                    .and_then(|value| value.checked_add(u64::from(source)))
                    .ok_or(HeapError::ExtentOverflow)?;
                let expression = entries
                    .get(usize::try_from(index).map_err(|_| HeapError::ExtentOverflow)?)
                    .ok_or(Error::Identity)?;
                let selected = builder.visit(expression, 1, budget)?;
                builder.reserve(128, budget)?;
                builder
                    .program
                    .outputs
                    .try_reserve(1)
                    .map_err(|_| HeapError::Capacity)?;
                builder.program.outputs.push(selected);
            }
        }
    }
    Ok(builder.program)
}

impl Builder<'_> {
    /// Every recursive descent is depth-bounded; every DAG comparison and allocation is metered.
    fn visit(&mut self, node: &Handle, depth: u64, budget: &mut Budget) -> Result<usize, Error> {
        budget.check(BudgetKind::Depth, depth, AT)?;
        budget.work(
            u64::try_from(self.memo.len())
                .map_err(|_| HeapError::ExtentOverflow)?
                .saturating_add(1),
            AT,
        )?;
        if let Some(index) = self.memo.iter().position(|previous| previous == node) {
            return Ok(index);
        }
        let description = self.graph.describe_syntax(node, budget)?;
        let SyntaxNode::Expression(expression) = description.node else {
            return Err(Error::Unsupported);
        };
        if !description.scope.is_empty() || description.binding.is_some() {
            return Err(Error::Unsupported);
        }
        let mut operands = Vec::new();
        for dependency in description.dependencies {
            if !matches!(dependency.role, SyntaxRole::Operand(_)) {
                return Err(Error::Unsupported);
            }
            operands.try_reserve(1).map_err(|_| HeapError::Capacity)?;
            operands.push(self.visit(&dependency.target, depth.saturating_add(1), budget)?);
        }
        let step = self.step(node, expression.operator, &operands, budget)?;
        let text = match &step {
            Step::Constant(value) => {
                u64::try_from(value.len()).map_err(|_| HeapError::ExtentOverflow)?
            }
            _ => 0,
        };
        self.reserve(text.saturating_add(256), budget)?;
        self.program
            .steps
            .try_reserve(1)
            .map_err(|_| HeapError::Capacity)?;
        self.memo.try_reserve(1).map_err(|_| HeapError::Capacity)?;
        let index = self.program.steps.len();
        self.program.steps.push(step);
        self.memo.push(node.clone());
        Ok(index)
    }

    /// Unsupported syntax has no numerical fallback or sampled reinterpretation.
    fn step(
        &self,
        node: &Handle,
        operator: Operator,
        operands: &[usize],
        budget: &mut Budget,
    ) -> Result<Step, Error> {
        Ok(match (operator, operands) {
            (Operator::Rational(value), []) => Step::Constant(value.exact_string(budget)?),
            (Operator::Free(_), []) => Step::Input(
                self.exact
                    .definition()
                    .inputs()
                    .iter()
                    .position(|input| input == node)
                    .ok_or(Error::Unsupported)?,
            ),
            (Operator::Neg, [value]) => Step::Neg(*value),
            (Operator::Add, [left, right]) => Step::Add(*left, *right),
            (Operator::Sub, [left, right]) => Step::Sub(*left, *right),
            (Operator::Mul, [left, right]) => Step::Mul(*left, *right),
            (Operator::Div, [left, right]) => Step::Div(*left, *right),
            (Operator::IntegerPower(exponent), [value]) => Step::Pow(*value, exponent),
            _ => return Err(Error::Unsupported),
        })
    }

    /// Memo slots, operands and owned constant spellings remain live together.
    fn reserve(&mut self, amount: u64, budget: &Budget) -> Result<(), Error> {
        self.program.bytes = self
            .program
            .bytes
            .checked_add(amount)
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(BudgetKind::StagedPayloadBytes, self.program.bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, self.program.bytes, AT)?;
        Ok(())
    }
}
