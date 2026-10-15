// SPDX-License-Identifier: AGPL-3.0-only
//! Native original admission and coefficient identities; this is not another algebra.

use std::collections::{BTreeMap, BTreeSet};

use nrz_cas::limits::LimitsOverride;
use nrz_cas::source::OriginalForm;
use nrz_cas::syntax::Operator;
use nrz_cas::{Handle, Rational};

use super::graph::Graph;
use super::{
    CheckedSensitivity, JacobianEntry, JacobianEvidence, SensitivityEntry, SensitivityError,
    SensitivityKernel, SensitivityObligation, SensitivityRecipe,
};
use crate::{CheckedDeclarations, CheckedExpression, Context, Declaration, PairingWitness};

impl Context {
    /// Check native square or reciprocal originals and consume their semantic owner.
    ///
    /// Rows follow `originals`; columns follow distinct `coordinates`. Every original
    /// must be a square, negative first power or literal-one quotient of one listed
    /// input. All declarations are checked first, including unused bodies. Input
    /// measures are retained; coherent output derivatives include their exact scales.
    /// Reciprocal premises remain unresolved numerical obligations even if an input
    /// declaration already claims to be nonzero.
    ///
    /// # Errors
    /// Refuses invalid originals, missing/duplicate/discrete coordinates, other recipes,
    /// conditional or unfinished CAS outcomes, mismatched witnesses and finite limits.
    pub fn check_sensitivity(
        self,
        recipe: SensitivityRecipe,
        originals: &[OriginalForm],
        coordinates: &[String],
        limits: &LimitsOverride,
    ) -> Result<CheckedSensitivity, SensitivityError> {
        let checks = self.check_all()?;
        if !matches!(
            recipe,
            SensitivityRecipe::Square | SensitivityRecipe::Reciprocal
        ) {
            return Err(SensitivityError::UnsupportedRecipe);
        }
        let inputs = coordinate_checks(&self, &checks, coordinates)?;
        let checked = originals
            .iter()
            .map(|original| self.check(original))
            .collect::<Result<Vec<_>, _>>()?;
        let (jacobian, entries, obligations) =
            lower(&self, recipe, checked, coordinates, &inputs, limits)?;
        let outputs = jacobian
            .originals
            .iter()
            .map(|original| original.descriptor().coherent())
            .collect();
        Ok(CheckedSensitivity {
            declarations: CheckedDeclarations::new(self, checks),
            recipe,
            coordinates: coordinates.to_vec(),
            originals: jacobian.originals.clone(),
            auxiliary: Vec::new(),
            inputs: inputs
                .iter()
                .map(|input| input.descriptor().clone())
                .collect(),
            outputs,
            jacobian,
            entries,
            obligations,
        })
    }
}

/// Only independent continuous inputs can name differential coordinates.
pub(crate) fn coordinate_checks(
    builder: &Context,
    checks: &BTreeMap<String, CheckedExpression>,
    coordinates: &[String],
) -> Result<Vec<CheckedExpression>, SensitivityError> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for name in coordinates {
        if !seen.insert(name)
            || !matches!(
                builder.declarations.get(name),
                Some(Declaration::Input { .. })
            )
        {
            return Err(SensitivityError::Coordinates);
        }
        let checked = checks.get(name).ok_or(SensitivityError::Coordinates)?;
        PairingWitness::sensitivity(checked.descriptor(), checked.descriptor())?;
        result.push(checked.clone());
    }
    if result.is_empty() {
        return Err(SensitivityError::Coordinates);
    }
    Ok(result)
}

/// Construct and inspect the symbolic Jacobian before selecting any final kernel cells.
fn lower(
    builder: &Context,
    recipe: SensitivityRecipe,
    originals: Vec<CheckedExpression>,
    coordinates: &[String],
    inputs: &[CheckedExpression],
    limits: &LimitsOverride,
) -> Result<
    (
        JacobianEvidence,
        Vec<SensitivityEntry>,
        Vec<SensitivityObligation>,
    ),
    SensitivityError,
> {
    let mut graph = Graph::new(builder, limits)?;
    graph.extent(originals.len(), coordinates.len())?;
    let selected = originals
        .iter()
        .map(|original| select(&mut graph, recipe, original, coordinates))
        .collect::<Result<Vec<_>, _>>()?;
    let obligations = premises(&mut graph, recipe, &selected, inputs)?;
    let witness = graph.jacobian(&originals, coordinates)?;
    let cells = super::replay::jacobian_cells(&mut graph, &witness, &originals, coordinates)?;
    let (derivatives, entries) =
        lower_entries(&mut graph, recipe, &originals, inputs, &selected, &cells)?;
    Ok((
        JacobianEvidence {
            originals,
            coordinates: coordinates.to_vec(),
            witnesses: vec![witness],
            work: graph.work,
            entries: derivatives,
        },
        entries,
        obligations,
    ))
}

/// Bind every coefficient identity and formal descriptor to its exact row and column.
fn lower_entries(
    graph: &mut Graph<'_>,
    recipe: SensitivityRecipe,
    originals: &[CheckedExpression],
    inputs: &[CheckedExpression],
    selected: &[usize],
    cells: &[Handle],
) -> Result<(Vec<JacobianEntry>, Vec<SensitivityEntry>), SensitivityError> {
    let mut entries = Vec::new();
    let mut derivatives = Vec::new();
    for (column, input) in inputs.iter().enumerate() {
        for (row, original) in originals.iter().enumerate() {
            let active = selected.get(row) == Some(&column);
            let expected = coefficient(graph, recipe, input, active)?;
            let cell = cells
                .get(derivatives.len())
                .ok_or(SensitivityError::WitnessMismatch)?;
            let identity = graph.equal(cell, &expected)?;
            let pairing =
                PairingWitness::sensitivity(&original.descriptor().coherent(), input.descriptor())?;
            derivatives.push(JacobianEntry {
                expression: cell.clone(),
                witness_index: 0,
                pairing: pairing.clone(),
                identity,
            });
            entries.push(SensitivityEntry {
                pairing,
                kernel: kernel(recipe, column, active)?,
            });
        }
    }
    Ok((derivatives, entries))
}

/// Inspect exactly the original native syntax, never a cancellation or normalized substitute.
fn select(
    graph: &mut Graph<'_>,
    recipe: SensitivityRecipe,
    original: &CheckedExpression,
    coordinates: &[String],
) -> Result<usize, SensitivityError> {
    let syntax = &original.original().syntax;
    let operands = graph.cas.operands(syntax, &mut graph.budget)?;
    let (operator, _) = graph.cas.expression(syntax)?;
    let operand = match (recipe, operator, operands.as_slice()) {
        (SensitivityRecipe::Square, Operator::IntegerPower(2), [input])
        | (SensitivityRecipe::Reciprocal, Operator::IntegerPower(-1), [input]) => input,
        (SensitivityRecipe::Reciprocal, Operator::Div, [numerator, input])
            if graph.cas.expression(numerator)?.0 == &Operator::Rational(Rational::one()) =>
        {
            input
        }
        _ => return Err(SensitivityError::UnsupportedRecipe),
    };
    let (Operator::Free(key), _) = graph.cas.expression(operand)? else {
        return Err(SensitivityError::UnsupportedRecipe);
    };
    coordinates
        .iter()
        .position(|coordinate| coordinate == key)
        .ok_or(SensitivityError::Coordinates)
}

/// Symbolic nonzero hypotheses are independent of caller-supplied physical input claims.
fn premises(
    graph: &mut Graph<'_>,
    recipe: SensitivityRecipe,
    selected: &[usize],
    inputs: &[CheckedExpression],
) -> Result<Vec<SensitivityObligation>, SensitivityError> {
    if recipe == SensitivityRecipe::Square {
        return Ok(Vec::new());
    }
    let unique = selected.iter().copied().collect::<BTreeSet<_>>();
    let mut terms = Vec::new();
    let mut obligations = Vec::new();
    for coordinate in unique {
        let input = inputs
            .get(coordinate)
            .ok_or(SensitivityError::Coordinates)?;
        terms.push((coordinate, input.original().syntax.clone()));
        obligations.push(SensitivityObligation::Nonzero { coordinate });
    }
    graph.nonzero_context(&terms)?;
    Ok(obligations)
}

/// The fixed derivative formula includes the input measure's exact chain-rule factor.
pub(crate) fn coefficient(
    graph: &mut Graph<'_>,
    recipe: SensitivityRecipe,
    input: &CheckedExpression,
    active: bool,
) -> Result<Handle, SensitivityError> {
    if !active {
        return graph.constant(Rational::zero());
    }
    let scale = graph.constant(input.descriptor().measure().scale().clone())?;
    let coherent = input.coordinates.clone();
    match recipe {
        SensitivityRecipe::Square => {
            let two = graph.constant(crate::exact::ratio(2, 1)?)?;
            let doubled = graph.term(Operator::Mul, &[two, coherent])?;
            graph.term(Operator::Mul, &[doubled, scale])
        }
        SensitivityRecipe::PositiveRoot | SensitivityRecipe::Solve { .. } => {
            Err(SensitivityError::UnsupportedRecipe)
        }
        SensitivityRecipe::Reciprocal => {
            let square = graph.term(Operator::Mul, &[coherent.clone(), coherent])?;
            let negative = graph.term(Operator::Neg, &[scale])?;
            graph.term(Operator::Div, &[negative, square])
        }
    }
}

/// Associate the tested formula with its closed numerical instruction.
const fn kernel(
    recipe: SensitivityRecipe,
    coordinate: usize,
    active: bool,
) -> Result<SensitivityKernel, SensitivityError> {
    if !active {
        return Ok(SensitivityKernel::Zero);
    }
    match recipe {
        SensitivityRecipe::Square => Ok(SensitivityKernel::Square { coordinate }),
        SensitivityRecipe::Reciprocal => Ok(SensitivityKernel::Reciprocal { coordinate }),
        SensitivityRecipe::PositiveRoot | SensitivityRecipe::Solve { .. } => {
            Err(SensitivityError::UnsupportedRecipe)
        }
    }
}
