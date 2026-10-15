// SPDX-License-Identifier: AGPL-3.0-only
//! Entrywise solve relations preserve one common operator across all right-hand sides.

use std::collections::BTreeMap;

use nrz_cas::{Handle, Rational, limits::LimitsOverride, source::OriginalForm, syntax::Operator};

use super::{
    CheckedSensitivity, JacobianEvidence, SensitivityEntry, SensitivityError, SensitivityKernel,
    SensitivityObligation, SensitivityRecipe, graph::Graph, implicit, native,
};
use crate::{CheckedDeclarations, CheckedExpression, Context, PairingWitness};

/// Untrusted symbol inventory for a nonempty square solve, checked by the context.
///
/// Each array uses logical column order. All keys must be distinct continuous
/// input declarations. `solution` names output placeholders, not extra sources.
/// The operator inventory is shared across every right-hand side.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveSymbols {
    /// Positive square order of `A`.
    pub order: usize,
    /// Positive number of columns of `B` and `X`.
    pub right_hand_sides: usize,
    /// Exactly `order*order` keys for `A`.
    pub operator: Vec<String>,
    /// Exactly `order*right_hand_sides` keys for `B`.
    pub right_hand_side: Vec<String>,
    /// Exactly `order*right_hand_sides` distinct keys for the exact nominal `X`.
    pub solution: Vec<String>,
}

/// Descriptor-checked entries used to build the fixed defining equations.
struct SolveData {
    /// Checked positive operator order.
    order: usize,
    /// Checked positive right-hand-side count.
    columns: usize,
    /// Operator entries in logical column order.
    operator: Vec<CheckedExpression>,
    /// Right-hand-side entries in logical column order.
    right_hand_side: Vec<CheckedExpression>,
    /// Output placeholders in logical column order.
    solution: Vec<CheckedExpression>,
}

impl Context {
    /// Certify the fixed sensitivity recipe from entrywise `A*X-B=0` partials.
    ///
    /// Independent columns are `(vec(A), vec(B))`; outputs are coherent `vec(X)`.
    /// The polynomial Jacobian orders `vec(X)` before those independent columns.
    /// All original products and sums are physically checked before differentiation.
    /// The numerical owner must verify one common invertible operator, enclose the
    /// exact nominal solution and certify each differential solve, including input
    /// measure scales. A rounded candidate cannot replace that nominal enclosure.
    ///
    /// # Errors
    /// Refuses malformed or empty extents, repeated/discrete keys, incompatible entry
    /// kinds, conditional or incomplete CAS proofs and exhausted finite limits.
    pub fn check_solve_sensitivity(
        self,
        symbols: &SolveSymbols,
        limits: &LimitsOverride,
    ) -> Result<CheckedSensitivity, SensitivityError> {
        let checks = self.check_all()?;
        let prepared = SolveData::prepare(&self, &checks, symbols, limits)?;
        let relations = prepared.relations(&self, limits)?;
        let equations = relations
            .iter()
            .map(|original| self.check(original))
            .collect::<Result<Vec<_>, _>>()?;
        let coordinates = symbols
            .operator
            .iter()
            .chain(&symbols.right_hand_side)
            .cloned()
            .collect::<Vec<_>>();
        let inputs = prepared
            .operator
            .iter()
            .chain(&prepared.right_hand_side)
            .cloned()
            .collect::<Vec<_>>();
        let keys = symbols
            .solution
            .iter()
            .chain(&coordinates)
            .cloned()
            .collect::<Vec<_>>();
        let jacobian = prepared.lower(&self, equations, &keys, &inputs, limits)?;
        let entries = prepared.entries(&inputs)?;
        Ok(CheckedSensitivity {
            declarations: CheckedDeclarations::new(self, checks),
            recipe: SensitivityRecipe::Solve {
                order: symbols.order,
                right_hand_sides: symbols.right_hand_sides,
            },
            coordinates,
            inputs: inputs
                .iter()
                .map(|input| input.descriptor().clone())
                .collect(),
            outputs: prepared
                .solution
                .iter()
                .map(|output| output.descriptor().coherent())
                .collect(),
            originals: jacobian.originals.clone(),
            auxiliary: symbols.solution.clone(),
            jacobian,
            entries,
            obligations: vec![
                SensitivityObligation::Invertible,
                SensitivityObligation::NominalSolution,
            ],
        })
    }
}

impl SolveData {
    /// Check extents before construction work and preserve every declared scalar contract.
    fn prepare(
        builder: &Context,
        checks: &BTreeMap<String, CheckedExpression>,
        symbols: &SolveSymbols,
        limits: &LimitsOverride,
    ) -> Result<Self, SensitivityError> {
        let operator_count = symbols
            .order
            .checked_mul(symbols.order)
            .ok_or(SensitivityError::ResourceLimit)?;
        let output_count = symbols
            .order
            .checked_mul(symbols.right_hand_sides)
            .ok_or(SensitivityError::ResourceLimit)?;
        if symbols.order == 0
            || symbols.right_hand_sides == 0
            || symbols.operator.len() != operator_count
            || symbols.solution.len() != output_count
            || symbols.right_hand_side.len() != output_count
        {
            return Err(SensitivityError::Coordinates);
        }
        let columns = operator_count
            .checked_add(output_count)
            .and_then(|count| count.checked_add(output_count))
            .ok_or(SensitivityError::ResourceLimit)?;
        Graph::new(builder, limits)?.extent(output_count, columns)?;
        let inputs = symbols
            .operator
            .iter()
            .chain(&symbols.right_hand_side)
            .cloned()
            .collect::<Vec<_>>();
        implicit::distinct(&inputs, &symbols.solution)?;
        let checked = native::coordinate_checks(builder, checks, &inputs)?;
        let operator = checked
            .get(..operator_count)
            .ok_or(SensitivityError::Coordinates)?
            .to_vec();
        let right_hand_side = checked
            .get(operator_count..)
            .ok_or(SensitivityError::Coordinates)?
            .to_vec();
        Ok(Self {
            order: symbols.order,
            columns: symbols.right_hand_sides,
            operator,
            right_hand_side,
            solution: native::coordinate_checks(builder, checks, &symbols.solution)?,
        })
    }

    /// Materialize every original multiply, left-fold sum and final subtraction.
    fn relations(
        &self,
        builder: &Context,
        limits: &LimitsOverride,
    ) -> Result<Vec<OriginalForm>, SensitivityError> {
        let mut graph = Graph::new(builder, limits)?;
        let mut equations = Vec::new();
        for column in 0..self.columns {
            for row in 0..self.order {
                let sum = self.row_sum(&mut graph, row, column)?;
                let right = entry(&self.right_hand_side, row, column, self.order)?
                    .original()
                    .syntax
                    .clone();
                let residual = graph.term(Operator::Sub, &[sum, right])?;
                equations.push(graph.original(residual, &format!("units:solve:{row}:{column}"))?);
            }
        }
        Ok(equations)
    }

    /// A positive-order sum starts with its first typed product, never an untyped zero.
    fn row_sum(
        &self,
        graph: &mut Graph<'_>,
        row: usize,
        column: usize,
    ) -> Result<Handle, SensitivityError> {
        let mut sum = None;
        for contraction in 0..self.order {
            let left = entry(&self.operator, row, contraction, self.order)?
                .original()
                .syntax
                .clone();
            let right = entry(&self.solution, contraction, column, self.order)?
                .original()
                .syntax
                .clone();
            let product = graph.term(Operator::Mul, &[left, right])?;
            sum = Some(match sum {
                Some(previous) => graph.term(Operator::Add, &[previous, product])?,
                None => product,
            });
        }
        sum.ok_or(SensitivityError::Coordinates)
    }

    /// Retain all polynomial partials in `(vec(X), vec(A), vec(B))` coordinate order.
    fn lower(
        &self,
        builder: &Context,
        equations: Vec<CheckedExpression>,
        keys: &[String],
        inputs: &[CheckedExpression],
        limits: &LimitsOverride,
    ) -> Result<JacobianEvidence, SensitivityError> {
        let mut graph = Graph::new(builder, limits)?;
        graph.extent(equations.len(), keys.len())?;
        let variables = self
            .solution
            .iter()
            .chain(inputs)
            .cloned()
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        for (column, input) in variables.iter().enumerate() {
            for row in 0..equations.len() {
                expected.push(self.partial(&mut graph, column, row, input)?);
            }
        }
        implicit::partials(&mut graph, equations, keys, &variables, &expected)
    }

    /// Fixed entrywise product-rule coefficients, including exact measure chain factors.
    fn partial(
        &self,
        graph: &mut Graph<'_>,
        column: usize,
        equation: usize,
        input: &CheckedExpression,
    ) -> Result<Handle, SensitivityError> {
        let scale = graph.constant(input.descriptor().measure().scale().clone())?;
        let row = equation % self.order;
        let rhs_column = equation / self.order;
        if column < self.solution.len() {
            if column / self.order == rhs_column {
                let operator = entry(&self.operator, row, column % self.order, self.order)?;
                return graph.term(Operator::Mul, &[operator.coordinates.clone(), scale]);
            }
        } else {
            let independent = column
                .checked_sub(self.solution.len())
                .ok_or(SensitivityError::Coordinates)?;
            if independent < self.operator.len() && independent % self.order == row {
                let output = entry(
                    &self.solution,
                    independent / self.order,
                    rhs_column,
                    self.order,
                )?;
                return graph.term(Operator::Mul, &[scale, output.coordinates.clone()]);
            }
            if independent.checked_sub(self.operator.len()) == Some(equation) {
                return graph.term(Operator::Neg, &[scale]);
            }
        }
        graph.constant(Rational::zero())
    }

    /// Formal quotient descriptors attach to the certified differential solve's output cells.
    fn entries(
        &self,
        inputs: &[CheckedExpression],
    ) -> Result<Vec<SensitivityEntry>, SensitivityError> {
        let mut entries = Vec::new();
        for (coordinate, input) in inputs.iter().enumerate() {
            for (output, solution) in self.solution.iter().enumerate() {
                let pairing = PairingWitness::sensitivity(
                    &solution.descriptor().coherent(),
                    input.descriptor(),
                )?;
                entries.push(SensitivityEntry {
                    pairing,
                    kernel: SensitivityKernel::Solve { coordinate, output },
                });
            }
        }
        Ok(entries)
    }
}

/// Resolve one column-ordered scalar entry with checked address arithmetic.
fn entry(
    values: &[CheckedExpression],
    row: usize,
    column: usize,
    rows: usize,
) -> Result<&CheckedExpression, SensitivityError> {
    let position = column
        .checked_mul(rows)
        .and_then(|value| value.checked_add(row))
        .ok_or(SensitivityError::ResourceLimit)?;
    values.get(position).ok_or(SensitivityError::Coordinates)
}
