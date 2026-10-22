// SPDX-License-Identifier: AGPL-3.0-only
//! Positive-root sensitivity from checked polynomial partials and a fixed analytic implication.

use std::collections::BTreeMap;

use nrz_cas::{Handle, Rational, limits::LimitsOverride, source::OriginalForm, syntax::Operator};

use super::{
    CheckedSensitivity, JacobianEvidence, SensitivityEntry, SensitivityError, SensitivityKernel,
    SensitivityObligation, SensitivityRecipe, graph::Graph, implicit, native,
};
use crate::{CheckedDeclarations, CheckedExpression, Context, PairingWitness};

/// Checked roots and original arguments before any implicit differentiation.
struct Roots {
    /// Caller-supplied original square-root expressions.
    originals: Vec<CheckedExpression>,
    /// Independent input symbols in caller order.
    inputs: Vec<CheckedExpression>,
    /// Distinct output placeholder symbols in output order.
    outputs: Vec<CheckedExpression>,
    /// Input column selected by each original root.
    selected: Vec<usize>,
}

impl Context {
    /// Certify native positive-root sensitivities through `t*t-x=0`.
    ///
    /// Each original must be `sqrt` of one independent input. Distinct `roots` are
    /// declared continuous input placeholders with the admitted output kind; they
    /// name exact nominal outputs, not additional numerical inputs or sources.
    /// Both the root and squared-root kind rules must be registered. The graph
    /// proves polynomial partials; the fixed recipe uses `2*t*dt=dx` only after
    /// the numerical owner encloses the positive root and its smooth domain.
    ///
    /// # Errors
    /// Refuses invalid originals, unmatched root descriptors, overlapping keys,
    /// unsupported syntax, incomplete CAS evidence and bounded construction failures.
    pub fn check_root_sensitivity(
        self,
        originals: &[OriginalForm],
        coordinates: &[String],
        roots: &[String],
        limits: &LimitsOverride,
    ) -> Result<CheckedSensitivity, SensitivityError> {
        let checks = self.check_all()?;
        let prepared = Roots::prepare(&self, &checks, originals, coordinates, roots, limits)?;
        let relations = prepared.relations(&self, limits)?;
        let equations = relations
            .iter()
            .map(|original| self.check(original))
            .collect::<Result<Vec<_>, _>>()?;
        let jacobian = prepared.lower(&self, equations, coordinates, roots, limits)?;
        let entries = prepared.entries()?;
        let obligations = prepared
            .selected
            .iter()
            .copied()
            .enumerate()
            .map(|(output, coordinate)| SensitivityObligation::PositiveRoot { coordinate, output })
            .collect();
        Ok(CheckedSensitivity {
            declarations: CheckedDeclarations::new(self, checks),
            recipe: SensitivityRecipe::PositiveRoot,
            coordinates: coordinates.to_vec(),
            inputs: prepared
                .inputs
                .iter()
                .map(|input| input.descriptor().clone())
                .collect(),
            outputs: prepared
                .originals
                .iter()
                .map(|original| original.descriptor().coherent())
                .collect(),
            originals: prepared.originals,
            auxiliary: roots.to_vec(),
            jacobian,
            entries,
            obligations,
        })
    }
}

impl Roots {
    /// Check all physical originals before identifying the fixed native operation.
    fn prepare(
        builder: &Context,
        checks: &BTreeMap<String, CheckedExpression>,
        originals: &[OriginalForm],
        coordinates: &[String],
        roots: &[String],
        limits: &LimitsOverride,
    ) -> Result<Self, SensitivityError> {
        implicit::distinct(coordinates, roots)?;
        let inputs = native::coordinate_checks(builder, checks, coordinates)?;
        let outputs = native::coordinate_checks(builder, checks, roots)?;
        if originals.len() != outputs.len() {
            return Err(SensitivityError::Coordinates);
        }
        let originals = originals
            .iter()
            .map(|original| builder.check(original))
            .collect::<Result<Vec<_>, _>>()?;
        for (original, output) in originals.iter().zip(&outputs) {
            original
                .descriptor()
                .conversion_to(&output.descriptor().coherent())?;
        }
        let mut graph = Graph::new(builder, limits)?;
        let columns = coordinates
            .len()
            .checked_add(roots.len())
            .ok_or(SensitivityError::ResourceLimit)?;
        graph.extent(originals.len(), columns)?;
        let selected = originals
            .iter()
            .map(|original| select(&mut graph, original, coordinates))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            originals,
            inputs,
            outputs,
            selected,
        })
    }

    /// Build the original scalar defining equations with the existing public CAS graph.
    fn relations(
        &self,
        builder: &Context,
        limits: &LimitsOverride,
    ) -> Result<Vec<OriginalForm>, SensitivityError> {
        let mut graph = Graph::new(builder, limits)?;
        let mut relations = Vec::new();
        for (row, output) in self.outputs.iter().enumerate() {
            let selected = self
                .selected
                .get(row)
                .ok_or(SensitivityError::Coordinates)?;
            let input = self
                .inputs
                .get(*selected)
                .ok_or(SensitivityError::Coordinates)?;
            let square = graph.term(
                Operator::IntegerPower(2),
                std::slice::from_ref(&output.original().syntax),
            )?;
            let relation = graph.term(Operator::Sub, &[square, input.original().syntax.clone()])?;
            relations.push(graph.original(relation, &format!("bindings:positive-root:{row}"))?);
        }
        Ok(relations)
    }

    /// Independent coordinates of the polynomial relation are roots first, then inputs.
    fn lower(
        &self,
        builder: &Context,
        equations: Vec<CheckedExpression>,
        coordinates: &[String],
        roots: &[String],
        limits: &LimitsOverride,
    ) -> Result<JacobianEvidence, SensitivityError> {
        let mut graph = Graph::new(builder, limits)?;
        let keys = roots.iter().chain(coordinates).cloned().collect::<Vec<_>>();
        let inputs = self
            .outputs
            .iter()
            .chain(&self.inputs)
            .cloned()
            .collect::<Vec<_>>();
        graph.extent(equations.len(), keys.len())?;
        let mut expected = Vec::new();
        for (column, input) in inputs.iter().enumerate() {
            for (row, selected) in self.selected.iter().enumerate() {
                expected.push(self.partial(&mut graph, column, row, *selected, input)?);
            }
        }
        implicit::partials(&mut graph, equations, &keys, &inputs, &expected)
    }

    /// Expected polynomial coefficients are `2*t*s_t`, `-s_x` and formal zero.
    fn partial(
        &self,
        graph: &mut Graph<'_>,
        column: usize,
        row: usize,
        selected: usize,
        input: &CheckedExpression,
    ) -> Result<Handle, SensitivityError> {
        if column == row {
            native::coefficient(graph, SensitivityRecipe::Square, input, true)
        } else if column.checked_sub(self.outputs.len()) == Some(selected) {
            let negative = input
                .descriptor()
                .measure()
                .scale()
                .negated(&mut graph.budget)?;
            graph.constant(negative)
        } else {
            graph.constant(Rational::zero())
        }
    }

    /// The positive-root implication yields final quotient descriptors without a scalar kind fiction.
    fn entries(&self) -> Result<Vec<SensitivityEntry>, SensitivityError> {
        let mut entries = Vec::new();
        for (coordinate, input) in self.inputs.iter().enumerate() {
            for (output, original) in self.originals.iter().enumerate() {
                let pairing = PairingWitness::sensitivity(
                    &original.descriptor().coherent(),
                    input.descriptor(),
                )?;
                let kernel = if self.selected.get(output) == Some(&coordinate) {
                    SensitivityKernel::PositiveRoot { coordinate, output }
                } else {
                    SensitivityKernel::Zero
                };
                entries.push(SensitivityEntry { pairing, kernel });
            }
        }
        Ok(entries)
    }
}

/// Named root calls remain outside the CAS derivative grammar; only this native call is admitted.
fn select(
    graph: &mut Graph<'_>,
    original: &CheckedExpression,
    coordinates: &[String],
) -> Result<usize, SensitivityError> {
    let syntax = &original.original().syntax;
    if graph.cas.expression(syntax)?.0 != &Operator::Call("sqrt".into()) {
        return Err(SensitivityError::UnsupportedRecipe);
    }
    let operands = graph.cas.operands(syntax, &mut graph.budget)?;
    let [operand] = operands.as_slice() else {
        return Err(SensitivityError::UnsupportedRecipe);
    };
    let (Operator::Free(key), _) = graph.cas.expression(operand)? else {
        return Err(SensitivityError::UnsupportedRecipe);
    };
    coordinates
        .iter()
        .position(|coordinate| coordinate == key)
        .ok_or(SensitivityError::Coordinates)
}
